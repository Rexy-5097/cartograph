//! The `cartograph match` command: join client HTTP calls to backend routes.
//!
//! Reports, for each client observation, the candidates and the decision.
//!
//! # What it shows
//!
//! Accepted matches, and equally importantly the ones that were refused: an
//! ambiguous result lists its rival candidates, and an unsupported one says
//! why it could not be compared. A resolver is only as trustworthy as its
//! refusals, so the refusals are on screen rather than filtered out.
//!
//! # Why the payload shape is frozen
//!
//! This command's `--json` document is the input to the M07 benchmark and the
//! M08 calibration dataset, both of which are accepted, digest-bound
//! artefacts. Its `totals`, `graph`, `matches` and `orm` sections are
//! therefore extended but never reshaped: M09 adds the `schema_version`,
//! `command` and `repository` keys and removes `root`, which used to carry an
//! absolute filesystem path into serialised output (PART 12).

use std::fmt::Write as _;
use std::path::Path;

use cartograph_resolver::{MatchResult, MatchStatus};
use serde::Serialize;

use crate::error::{CliError, ErrorCode, ExitCode};
use crate::json::{self, JsonEdge, JsonNode, SCHEMA_VERSION};
use crate::pipeline::{self, Analysis};

/// Aggregate counts across a run.
///
/// Field names are a contract: `benchmarks/run_benchmark.py` reads them by
/// name, and the M07 results in `benchmarks/results/` were recorded through
/// them. Renaming one silently invalidates a measurement.
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
    /// Routes whose served path was composed from enclosing router prefixes.
    routes_composed: usize,
    /// Calls from a caller into a client wrapper that issues a request.
    client_call_edges: usize,
    /// Dynamic URLs the evaluator fully determined (M05).
    dynamic_resolved: usize,
    /// Dynamic URLs partly determined, with the gaps kept explicit.
    dynamic_partial: usize,
}

impl Totals {
    /// Projects the pipeline's counts onto this command's frozen field names.
    fn from_pipeline(totals: &pipeline::Totals) -> Self {
        Self {
            files: totals.files,
            backend_routes: totals.routes,
            client_calls: totals.http_observations,
            exact: totals.exact,
            strong: totals.strong,
            ambiguous: totals.ambiguous,
            no_match: totals.no_match,
            unsupported: totals.unsupported,
            edges: totals.http_call_edges,
            orm_models: totals.orm_models,
            orm_tables: totals.orm_tables,
            orm_edges: totals.orm_edges,
            orm_ambiguous: totals.orm_ambiguous,
            routes_composed: totals.routes_composed,
            client_call_edges: totals.client_call_edges,
            dynamic_resolved: totals.dynamic_resolved,
            dynamic_partial: totals.dynamic_partial,
        }
    }
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

/// The graph a run produced, as built — not a re-derivation.
///
/// Exported so a reader — or the M07 benchmark — can verify **node identity**
/// rather than infer it. Two edges that name the same handler are only part of
/// one chain if they share a node id, and no summary can show that (ADR-0011).
#[derive(Debug, Serialize)]
struct JsonGraph {
    node_count: usize,
    edge_count: usize,
    nodes: Vec<JsonNode>,
    edges: Vec<JsonEdge>,
}

#[derive(Debug, Serialize)]
struct JsonReport<'a> {
    /// The version of the output contract.
    schema_version: &'static str,
    /// Which command produced the document.
    command: &'static str,
    /// What was analysed. Never the absolute path (PART 12).
    repository: &'a crate::pipeline::Repository,
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
///
/// # Errors
///
/// Whatever the pipeline could not do; see [`pipeline::run`].
pub fn run(path: &Path, json: bool) -> Result<(String, ExitCode), CliError> {
    let analysis = pipeline::run(path, pipeline::Options { quiet: json })?;

    let text = if json {
        render_json(&analysis)?
    } else {
        render_text(&analysis)
    };

    Ok((text, ExitCode::Success))
}

fn render_json(analysis: &Analysis) -> Result<String, CliError> {
    let orm = &analysis.orm;

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
        schema_version: SCHEMA_VERSION,
        command: "match",
        repository: &analysis.repository,
        elapsed_ms: analysis.elapsed.as_millis(),
        totals: Totals::from_pipeline(&analysis.totals),
        graph: JsonGraph {
            node_count: analysis.graph.node_count(),
            edge_count: analysis.graph.edge_count(),
            nodes: json::nodes(&analysis.graph),
            edges: json::edges(&analysis.graph),
        },
        matches: analysis.results.iter().map(json_match).collect(),
        orm: orm_rows,
        orm_ambiguous: orm.ambiguous.clone(),
    };

    let mut text = serde_json::to_string_pretty(&report).map_err(|error| {
        CliError::new(
            ErrorCode::AnalysisFailed,
            "the result could not be serialised as JSON",
        )
        .with_hint(format!("underlying cause: {error}"))
    })?;
    text.push('\n');
    Ok(text)
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

fn render_text(analysis: &Analysis) -> String {
    let totals = Totals::from_pipeline(&analysis.totals);
    let mut out = String::with_capacity(4096);
    render_decisions(&mut out, analysis);
    render_totals(&mut out, &totals, analysis.elapsed);
    out
}

/// One block per client observation: what it is, and what was decided.
fn render_decisions(out: &mut String, analysis: &Analysis) {
    for result in &analysis.results {
        let client = &result.client.provenance;
        let _ = writeln!(
            out,
            "{}:{}  {}",
            client.file, client.span.start_line, result.client
        );

        match result.status {
            MatchStatus::Exact | MatchStatus::Strong => {
                let Some(candidate) = result.accepted() else {
                    // An accepted status without a candidate would be a
                    // resolver defect; say so rather than panic in a renderer.
                    let _ = writeln!(out, "    ↓ MATCH — candidate missing (defect)");
                    continue;
                };
                let route = &candidate.route.provenance;
                let _ = writeln!(
                    out,
                    "    ↓ MATCH ({})   confidence {}   provenance route_matcher",
                    status_name(result.status),
                    candidate.confidence
                );
                let _ = writeln!(
                    out,
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
                let _ = writeln!(
                    out,
                    "    ↓ AMBIGUOUS — {} equally plausible routes, no edge produced",
                    result.candidates.len()
                );
                for candidate in &result.candidates {
                    let _ = writeln!(
                        out,
                        "      candidate {}:{}  {}",
                        candidate.route.provenance.file,
                        candidate.route.provenance.span.start_line,
                        candidate.route
                    );
                }
            }
            MatchStatus::Unsupported => {
                let _ = writeln!(
                    out,
                    "    ↓ UNSUPPORTED — {}",
                    result
                        .unsupported_reason
                        .map_or_else(|| "not comparable".to_owned(), |r| format!("{r:?}"))
                );
            }
            _ => {
                let _ = writeln!(out, "    ↓ no matching route");
            }
        }
    }
}

/// The run's aggregate counts.
fn render_totals(out: &mut String, totals: &Totals, elapsed: std::time::Duration) {
    let _ = writeln!(out);
    let _ = writeln!(out, "Files              {}", totals.files);
    let _ = writeln!(out, "Backend routes     {}", totals.backend_routes);
    let _ = writeln!(out, "Client calls       {}", totals.client_calls);
    let _ = writeln!(
        out,
        "Decisions          {} exact, {} strong, {} ambiguous, {} no-match, {} unsupported",
        totals.exact, totals.strong, totals.ambiguous, totals.no_match, totals.unsupported
    );
    let _ = writeln!(
        out,
        "Dynamic URLs       {} fully resolved, {} partially",
        totals.dynamic_resolved, totals.dynamic_partial
    );
    let _ = writeln!(
        out,
        "Routers            {} route(s) given a composed served path",
        totals.routes_composed
    );
    let _ = writeln!(
        out,
        "Client wrappers    {} call edge(s) into a function that issues a request",
        totals.client_call_edges
    );
    let _ = writeln!(out, "HttpCall edges     {}", totals.edges);
    let _ = writeln!(
        out,
        "ORM                {} models ({} with a resolved table), {} edges",
        totals.orm_models, totals.orm_tables, totals.orm_edges
    );
    if totals.orm_ambiguous > 0 {
        let _ = writeln!(
            out,
            "                   {} model name(s) claimed by more than one class — no edge",
            totals.orm_ambiguous
        );
    }
    let _ = writeln!(out, "Completed in       {elapsed:.2?}");
    let _ = writeln!(out);
    let _ = writeln!(
        out,
        "Edges are produced only for exact and strong matches. Ambiguous results"
    );
    let _ = writeln!(
        out,
        "keep their candidates and produce no edge. Confidence values are the"
    );
    let _ = writeln!(
        out,
        "specification's uncalibrated priors; see docs/benchmarks/m08-confidence-policy.md."
    );
}
