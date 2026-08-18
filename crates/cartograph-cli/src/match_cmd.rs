//! The `cartograph match` command: join client HTTP calls to backend routes.
//!
//! Runs the whole resolver pipeline over a tree — extraction, canonicalisation,
//! matching, classification — and prints, for each client observation, the
//! candidates and the decision.
//!
//! # What it shows
//!
//! Accepted matches, and equally importantly the ones that were refused: an
//! ambiguous result lists its rival candidates, and an unsupported one says
//! why it could not be compared. A resolver is only as trustworthy as its
//! refusals, so the refusals are on screen rather than filtered out.

use std::path::Path;

use anyhow::{Context, Result};
use cartograph_graph::ArchitectureGraph;
use cartograph_parser::Analyzer;
use cartograph_resolver::{
    MatchResult, MatchStatus, RouteIndex, add_edge_for_match, match_client, normalize_client_call,
    normalize_route_declaration,
};
use serde::Serialize;

use crate::discovery;

/// Aggregate counts across a run.
#[derive(Debug, Default, Serialize)]
struct Totals {
    files: usize,
    backend_routes: usize,
    client_calls: usize,
    exact: usize,
    strong: usize,
    ambiguous: usize,
    no_match: usize,
    unsupported: usize,
    edges: usize,
}

/// One decision, in the shape `--json` promises.
#[derive(Debug, Serialize)]
struct JsonMatch {
    client_file: String,
    client_line: u32,
    client_route: String,
    status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    unsupported_reason: Option<String>,
    candidates: Vec<JsonCandidate>,
}

#[derive(Debug, Serialize)]
struct JsonCandidate {
    route: String,
    file: String,
    line: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    handler: Option<String>,
    confidence: f32,
    method_compatibility: String,
    path_compatibility: String,
    reasons: Vec<String>,
}

#[derive(Debug, Serialize)]
struct JsonReport {
    root: String,
    totals: Totals,
    matches: Vec<JsonMatch>,
}

/// Matches every client observation under `path` against the routes found there.
pub fn run(path: &Path, json: bool) -> Result<()> {
    let (root, files) = discovery::discover(path)?;
    let mut analyzer = Analyzer::new().context("loading grammars")?;

    let mut totals = Totals::default();
    let mut backend = Vec::new();
    let mut clients = Vec::new();

    let started = std::time::Instant::now();
    for rel in &files {
        let analysis = analyzer
            .analyze_file(&root, rel)
            .with_context(|| format!("analysing {rel}"))?;
        totals.files += 1;

        for route in &analysis.routes {
            backend.push(normalize_route_declaration(
                route,
                &analysis.path,
                analysis.language,
            ));
        }
        for call in &analysis.http_calls {
            clients.push(normalize_client_call(
                call,
                &analysis.path,
                analysis.language,
            ));
        }
    }
    totals.backend_routes = backend.len();
    totals.client_calls = clients.len();

    let index = RouteIndex::build(backend);
    let results: Vec<MatchResult> = clients.iter().map(|c| match_client(c, &index)).collect();

    // Edges are built only from accepted matches; the graph is the product of
    // the decisions, never an input to them.
    let mut graph = ArchitectureGraph::new();
    for result in &results {
        match result.status {
            MatchStatus::Exact => totals.exact += 1,
            MatchStatus::Strong => totals.strong += 1,
            MatchStatus::Ambiguous => totals.ambiguous += 1,
            MatchStatus::NoMatch => totals.no_match += 1,
            MatchStatus::Unsupported => totals.unsupported += 1,
            _ => {}
        }
        if add_edge_for_match(&mut graph, result, None)?.is_some() {
            totals.edges += 1;
        }
    }
    let elapsed = started.elapsed();

    if json {
        let report = JsonReport {
            root: path.display().to_string(),
            totals,
            matches: results.iter().map(json_match).collect(),
        };
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else {
        print_summary(&results, &totals, elapsed);
    }
    Ok(())
}

fn status_name(status: MatchStatus) -> &'static str {
    match status {
        MatchStatus::Exact => "exact",
        MatchStatus::Strong => "strong",
        MatchStatus::Ambiguous => "ambiguous",
        MatchStatus::NoMatch => "no-match",
        MatchStatus::Unsupported => "unsupported",
        _ => "other",
    }
}

fn json_match(result: &MatchResult) -> JsonMatch {
    JsonMatch {
        client_file: result.client.provenance.file.clone(),
        client_line: result.client.provenance.span.start_line,
        client_route: result.client.to_string(),
        status: status_name(result.status).to_owned(),
        unsupported_reason: result.unsupported_reason.map(|r| format!("{r:?}")),
        candidates: result
            .candidates
            .iter()
            .map(|c| JsonCandidate {
                route: c.route.to_string(),
                file: c.route.provenance.file.clone(),
                line: c.route.provenance.span.start_line,
                handler: c.route.provenance.handler.clone(),
                confidence: c.confidence.get(),
                method_compatibility: format!("{:?}", c.method),
                path_compatibility: format!("{:?}", c.path),
                reasons: c.reasons.iter().map(|r| format!("{r:?}")).collect(),
            })
            .collect(),
    }
}

fn print_summary(results: &[MatchResult], totals: &Totals, elapsed: std::time::Duration) {
    for result in results {
        let client = &result.client.provenance;
        println!(
            "{}:{}  {}",
            client.file, client.span.start_line, result.client
        );

        match result.status {
            MatchStatus::Exact | MatchStatus::Strong => {
                let candidate = result.accepted().expect("accepted status has a candidate");
                let route = &candidate.route.provenance;
                println!(
                    "    ↓ MATCH ({})   confidence {}   provenance route_matcher",
                    status_name(result.status),
                    candidate.confidence
                );
                println!(
                    "      {}:{}  {}{}",
                    route.file,
                    route.span.start_line,
                    candidate.route,
                    route
                        .handler
                        .as_ref()
                        .map(|h| format!("  → {h}"))
                        .unwrap_or_default()
                );
            }
            MatchStatus::Ambiguous => {
                println!(
                    "    ↓ AMBIGUOUS — {} equally plausible routes, no edge produced",
                    result.candidates.len()
                );
                for candidate in &result.candidates {
                    println!(
                        "      candidate {}:{}  {}",
                        candidate.route.provenance.file,
                        candidate.route.provenance.span.start_line,
                        candidate.route
                    );
                }
            }
            MatchStatus::Unsupported => {
                println!(
                    "    ↓ UNSUPPORTED — {}",
                    result
                        .unsupported_reason
                        .map_or_else(|| "not comparable".to_owned(), |r| format!("{r:?}"))
                );
            }
            _ => println!("    ↓ no matching route"),
        }
    }

    println!();
    println!("Files              {}", totals.files);
    println!("Backend routes     {}", totals.backend_routes);
    println!("Client calls       {}", totals.client_calls);
    println!(
        "Decisions          {} exact, {} strong, {} ambiguous, {} no-match, {} unsupported",
        totals.exact, totals.strong, totals.ambiguous, totals.no_match, totals.unsupported
    );
    println!("HttpCall edges     {}", totals.edges);
    println!("Completed in       {elapsed:.2?}");
    println!();
    println!("Edges are produced only for exact and strong matches. Ambiguous results");
    println!("keep their candidates and produce no edge. Confidence values are the");
    println!("specification's uncalibrated priors; calibration is milestone M08.");
}
