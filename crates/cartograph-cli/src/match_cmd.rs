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
use cartograph_core::{EdgeKind, NodeKind, Provenance};
use cartograph_graph::ArchitectureGraph;
use cartograph_parser::Analyzer;
use cartograph_resolver::{
    MatchResult, MatchStatus, OrmAnalysis, RouteIndex, add_edge_for_match, add_orm_edges,
    collect_exported_constants, match_client, normalize_client_call, normalize_route_declaration,
    resolve_url, scope_for_file, with_resolved_url,
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
    /// ORM models discovered, and how many resolved a table (M06).
    orm_models: usize,
    orm_tables: usize,
    orm_edges: usize,
    orm_ambiguous: usize,
    /// Dynamic URLs the evaluator fully determined (M05).
    dynamic_resolved: usize,
    /// Dynamic URLs partly determined, with the gaps kept explicit.
    dynamic_partial: usize,
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

/// One ORM relationship, in the shape `--json` promises.
#[derive(Debug, Serialize)]
struct JsonOrm {
    model: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    table: Option<String>,
    table_evidence: String,
    flavor: String,
    base: String,
    file: String,
    line: u32,
    /// Handlers that access this model, with the call that evidenced it.
    accessed_by: Vec<String>,
}

/// One graph node, in the shape `--json` promises.
///
/// Exported so a reader — or the M07 benchmark — can verify **node identity**
/// rather than infer it. Two edges that name the same handler are only part of
/// one chain if they share a node id, and no summary can show that.
#[derive(Debug, Serialize)]
struct JsonNode {
    id: u64,
    kind: NodeKind,
    name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    file: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    line: Option<u32>,
}

/// One graph edge, with the four attributes the domain model requires.
#[derive(Debug, Serialize)]
struct JsonEdge {
    id: u64,
    source: u64,
    target: u64,
    kind: EdgeKind,
    confidence: f32,
    provenance: Provenance,
    evidence: String,
    file: String,
    line: u32,
}

/// The graph a run produced, as built — not a re-derivation.
#[derive(Debug, Serialize)]
struct JsonGraph {
    node_count: usize,
    edge_count: usize,
    nodes: Vec<JsonNode>,
    edges: Vec<JsonEdge>,
}

#[derive(Debug, Serialize)]
struct JsonReport {
    root: String,
    /// Wall-clock time for extraction, resolution and graph construction.
    elapsed_ms: u128,
    totals: Totals,
    /// The graph itself, so traversal can be checked rather than assumed.
    graph: JsonGraph,
    matches: Vec<JsonMatch>,
    orm: Vec<JsonOrm>,
    /// Model names claimed by more than one class, which resolve to nothing.
    orm_ambiguous: Vec<String>,
}

/// Matches every client observation under `path` against the routes found there.
pub fn run(path: &Path, json: bool) -> Result<()> {
    let outcome = analyse(path)?;
    report(path, outcome, json)
}

/// Everything one run produced.
struct Outcome {
    totals: Totals,
    results: Vec<MatchResult>,
    orm: OrmAnalysis,
    /// Kept rather than dropped: the graph is the deliverable, and the
    /// decisions above are only the reasoning that produced it.
    graph: ArchitectureGraph,
    elapsed: std::time::Duration,
}

fn analyse(path: &Path) -> Result<Outcome> {
    let (root, files) = discovery::discover(path)?;
    let mut analyzer = Analyzer::new().context("loading grammars")?;

    let mut totals = Totals::default();
    let mut backend = Vec::new();
    let mut clients = Vec::new();

    let started = std::time::Instant::now();

    // Every file is analysed before any URL is resolved: dynamic resolution
    // (M05) needs the whole project's exported constants to follow an import.
    let mut analyses = Vec::with_capacity(files.len());
    for rel in &files {
        analyses.push(
            analyzer
                .analyze_file(&root, rel)
                .with_context(|| format!("analysing {rel}"))?,
        );
    }
    let exported = collect_exported_constants(&analyses);

    for analysis in &analyses {
        totals.files += 1;
        let scope = scope_for_file(analysis, &exported);

        for route in &analysis.routes {
            backend.push(normalize_route_declaration(
                route,
                &analysis.path,
                analysis.language,
            ));
        }
        for call in &analysis.http_calls {
            // M05 resolves a dynamic URL as far as the source determines it.
            // A literal URL is declined and takes the unchanged M04 path, so
            // previously-correct behaviour cannot shift.
            let resolved = resolve_url(call, analysis, &scope);
            if let Some(r) = &resolved {
                if r.is_fully_known() {
                    totals.dynamic_resolved += 1;
                } else {
                    totals.dynamic_partial += 1;
                }
            }
            let effective = resolved
                .as_ref()
                .map_or_else(|| call.clone(), |r| with_resolved_url(call, r));
            clients.push(normalize_client_call(
                &effective,
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
    // M06: handler → model → table, over the same graph so the cross-stack
    // chain stays connected (ADR-0011).
    let orm = OrmAnalysis::build(&analyses);
    totals.orm_models = orm.models.len();
    totals.orm_tables = orm
        .models
        .values()
        .filter(|m| m.table.name().is_some())
        .count();
    totals.orm_ambiguous = orm.ambiguous.len();
    totals.orm_edges = add_orm_edges(&mut graph, &orm, None)?;

    let elapsed = started.elapsed();

    Ok(Outcome {
        totals,
        results,
        orm,
        graph,
        elapsed,
    })
}

fn report(path: &Path, outcome: Outcome, json: bool) -> Result<()> {
    let Outcome {
        totals,
        results,
        orm,
        graph,
        elapsed,
    } = outcome;

    if json {
        let mut orm_rows: Vec<JsonOrm> = orm
            .models
            .values()
            .map(|m| JsonOrm {
                model: m.name.clone(),
                table: m.table.name().map(ToOwned::to_owned),
                table_evidence: m.table.explain(),
                flavor: format!("{:?}", m.flavor),
                base: m.base.clone(),
                file: m.file.clone(),
                line: m.span.start_line,
                accessed_by: orm
                    .accesses
                    .iter()
                    .filter(|a| a.model == m.name)
                    .filter_map(|a| {
                        a.handler.as_ref().map(|h| {
                            format!(
                                "{h} via {}() at {}:{}",
                                a.expression, a.file, a.span.start_line
                            )
                        })
                    })
                    .collect(),
            })
            .collect();
        orm_rows.sort_by(|a, b| a.model.cmp(&b.model));

        let report = JsonReport {
            root: path.display().to_string(),
            elapsed_ms: elapsed.as_millis(),
            totals,
            graph: json_graph(&graph),
            matches: results.iter().map(json_match).collect(),
            orm: orm_rows,
            orm_ambiguous: orm.ambiguous.clone(),
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

/// Serialises the graph exactly as it was built.
///
/// Nodes and edges are emitted in id order so two runs over the same tree
/// produce byte-identical output, which is what makes a benchmark result
/// comparable between passes.
fn json_graph(graph: &ArchitectureGraph) -> JsonGraph {
    let mut nodes: Vec<JsonNode> = graph
        .nodes()
        .map(|n| JsonNode {
            id: n.id().as_u64(),
            kind: n.kind(),
            name: n.name().to_owned(),
            file: n.location().map(|l| l.file().to_owned()),
            line: n.location().map(cartograph_core::SourceLocation::line),
        })
        .collect();
    nodes.sort_by_key(|n| n.id);

    let mut edges: Vec<JsonEdge> = graph
        .edges()
        .map(|e| JsonEdge {
            id: e.id().as_u64(),
            source: e.source().as_u64(),
            target: e.target().as_u64(),
            kind: e.kind(),
            confidence: e.confidence().get(),
            provenance: e.provenance(),
            evidence: e.evidence().as_str().to_owned(),
            file: e.location().file().to_owned(),
            line: e.location().line(),
        })
        .collect();
    edges.sort_by_key(|e| e.id);

    JsonGraph {
        node_count: graph.node_count(),
        edge_count: graph.edge_count(),
        nodes,
        edges,
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
    println!(
        "Dynamic URLs       {} fully resolved, {} partially",
        totals.dynamic_resolved, totals.dynamic_partial
    );
    println!("HttpCall edges     {}", totals.edges);
    println!(
        "ORM                {} models ({} with a resolved table), {} edges",
        totals.orm_models, totals.orm_tables, totals.orm_edges
    );
    if totals.orm_ambiguous > 0 {
        println!(
            "                   {} model name(s) claimed by more than one class — no edge",
            totals.orm_ambiguous
        );
    }
    println!("Completed in       {elapsed:.2?}");
    println!();
    println!("Edges are produced only for exact and strong matches. Ambiguous results");
    println!("keep their candidates and produce no edge. Confidence values are the");
    println!("specification's uncalibrated priors; calibration is milestone M08.");
}
