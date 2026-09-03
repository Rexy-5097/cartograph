//! `cartograph trace <symbol>`: follow one relationship across the stack.
//!
//! # Why this is the command that matters
//!
//! The summary tells a developer that 102 HTTP boundary crossings exist. It
//! does not tell them that *this button* posts to *that handler* which writes
//! *that table*. Trace does, and it is the only place the product's thesis —
//! "compute the architecture, prove the relationships" — becomes something a
//! person can check line by line.
//!
//! # This module renders; it does not walk
//!
//! The traversal and the symbol lookup live in [`cartograph_graph::trace`]
//! (M15 Slice 1, ADR-0019). They were here until the MCP server became the
//! third client and a binary crate turned out to be undependable — the same
//! discovery ADR-0016 made at M11. What is left is presentation: the JSON
//! document, the indented text, and the labels.
//!
//! Two things stay here because they are not graph facts. **Refusals** are
//! observations the *resolver* declined, and they live in `Analysis::results`;
//! reaching them from `cartograph-graph` would invert the dependency
//! direction. **How a candidate is described to a person** is a client's
//! choice, so the graph returns [`NodeId`]s and this module formats them.
//!
//! Every hop still reports the confidence, provenance, evidence and location
//! that the edge itself carries. If the rendered path and the graph disagree,
//! the rendering is wrong — there is no second opinion to consult.
//!
//! # Ambiguity is not resolved by choosing
//!
//! Two symbols named `User` in different files are two different things. This
//! command lists both and exits [`ExitCode::Ambiguous`] rather than tracing
//! whichever the graph happened to number first — picking one would make the
//! output depend on iteration order, which is exactly the class of quiet
//! wrongness the project's refusal rules exist to prevent.

use std::fmt::Write as _;

use cartograph_core::{EdgeKind, NodeId, NodeKind, Provenance};
use cartograph_graph::ArchitectureGraph;
use cartograph_graph::trace::{self, Stop, SymbolError, TraceStep};
use cartograph_resolver::MatchStatus;
use serde::Serialize;

use crate::error::{CliError, ErrorCode, ExitCode};
use crate::json::{self, Envelope, JsonEdge, JsonNode, Summary};
use crate::pipeline::{self, Analysis};

/// How deep a trace walks before stopping.
///
/// Re-exported so `--max-depth`'s default and the traversal that honours it
/// cannot drift apart.
pub const DEFAULT_MAX_DEPTH: usize = trace::DEFAULT_MAX_DEPTH;

/// The sentence shown for a stop reason.
///
/// Wording is presentation, so it lives here rather than on the graph's
/// [`Stop`]; `--max-depth` is named because it is the flag the reader would
/// change.
fn describe_stop(stop: Stop) -> &'static str {
    match stop {
        Stop::ChainEnd => "chain ends here — no outgoing edge from this node",
        Stop::MaxDepth => "stopped at --max-depth — the chain may continue",
        Stop::Cycle => "stopped — this node is already on the path",
    }
}

/// One hop, and everything reachable through it.
#[derive(Debug, Serialize)]
struct Step {
    /// The relationship traversed, with its full evidence.
    edge: JsonEdge,
    /// Where it led.
    to: JsonNode,
    /// Further hops from there.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    next: Vec<Step>,
    /// Why the walk stopped, when it stopped here.
    #[serde(skip_serializing_if = "Option::is_none")]
    stop: Option<Stop>,
    /// Refusals recorded in the same file as this node, when the chain broke.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    refusals: Vec<Refusal>,
}

/// An observation the resolver declined to turn into an edge.
///
/// Reported at a break so the output can say "the chain stops here, and here
/// is something nearby that was refused" instead of implying nothing exists.
#[derive(Debug, Clone, Serialize)]
struct Refusal {
    /// Repository-relative path.
    file: String,
    /// One-based line.
    line: u32,
    /// The canonical form of the observation.
    observation: String,
    /// `ambiguous`, `no-match` or `unsupported`.
    status: &'static str,
    /// Why, when the resolver recorded a reason.
    #[serde(skip_serializing_if = "Option::is_none")]
    reason: Option<String>,
}

/// The `result` section of a trace document.
#[derive(Debug, Serialize)]
struct TraceResult {
    /// The symbol as requested.
    symbol: String,
    /// The node the walk started from.
    from: JsonNode,
    /// The hops.
    steps: Vec<Step>,
    /// Whether any path reached a database table — the full-stack chain.
    reaches_table: bool,
    /// Why the walk stopped, when it stopped immediately.
    #[serde(skip_serializing_if = "Option::is_none")]
    stop: Option<Stop>,
    /// Refusals in the same file, when the walk stopped immediately.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    refusals: Vec<Refusal>,
    /// The depth limit that was in force.
    max_depth: usize,
}

/// Traces `symbol` through the graph built from `path`.
///
/// # Errors
///
/// [`ErrorCode::SymbolNotFound`] when no node carries the name, and
/// [`ErrorCode::AmbiguousSymbol`] when more than one does.
pub fn run(
    path: &std::path::Path,
    symbol: &str,
    max_depth: usize,
    as_json: bool,
) -> Result<(String, ExitCode), CliError> {
    let analysis = pipeline::run(path, pipeline::Options { quiet: as_json })?;
    let start = resolve_symbol(&analysis.graph, symbol)?;

    let walk = trace::trace(&analysis.graph, start, max_depth);
    let steps = render_steps_of(&analysis, &walk.steps);

    let refusals = if steps.is_empty() {
        refusals_near(&analysis, start)
    } else {
        Vec::new()
    };

    let result = TraceResult {
        symbol: symbol.to_owned(),
        from: node_json(&analysis.graph, start),
        reaches_table: reaches_table(&steps),
        steps,
        stop: walk.stop,
        refusals,
        max_depth,
    };

    let text = if as_json {
        render_json(&analysis, &result)?
    } else {
        render_text(&analysis, &result)
    };

    Ok((text, ExitCode::Success))
}

// ── symbol resolution ───────────────────────────────────────────────

/// Finds the single node a query names, as a CLI error when it cannot.
///
/// The lookup itself is [`trace::resolve_symbol`]; what is here is how a
/// failure is *said*. The graph returns candidate [`NodeId`]s precisely so
/// that this decision stays in the client.
pub(crate) fn resolve_symbol(graph: &ArchitectureGraph, query: &str) -> Result<NodeId, CliError> {
    trace::resolve_symbol(graph, query).map_err(|error| match error {
        SymbolError::NotFound { near } => not_found(graph, query, &near),
        SymbolError::Ambiguous { matches } => CliError::new(
            ErrorCode::AmbiguousSymbol,
            format!("`{query}` matches {} symbols", matches.len()),
        )
        .with_candidates(describe_all(graph, &matches))
        .with_hint(
            "trace one of them by qualifying the name with its file, for example \
             `cartograph trace path/to/file.py:name`",
        ),
    })
}

/// Describes each node, in the order given.
fn describe_all(graph: &ArchitectureGraph, ids: &[NodeId]) -> Vec<String> {
    ids.iter()
        .filter_map(|id| graph.node(*id))
        .map(describe_node)
        .collect()
}

/// The not-found error, with near-misses when there are any.
fn not_found(graph: &ArchitectureGraph, query: &str, near: &[NodeId]) -> CliError {
    // Sorted, de-duplicated and capped on the *rendered* description: two
    // nodes can describe identically, and the list a person reads is what must
    // not repeat itself.
    let mut near: Vec<String> = describe_all(graph, near);
    near.sort();
    near.dedup();
    near.truncate(10);

    let error = CliError::new(
        ErrorCode::SymbolNotFound,
        format!("no symbol named `{query}` is in the graph"),
    );

    if near.is_empty() {
        error.with_hint(
            "the graph contains only symbols that participate in a resolved relationship; \
             run `cartograph .` to see what was found, and note that an observation the \
             resolver refused produces no node",
        )
    } else {
        error
            .with_candidates(near)
            .with_hint("one of the above may be the symbol you meant")
    }
}

fn describe_node(node: &cartograph_core::Node) -> String {
    match node.location() {
        Some(location) => format!(
            "{}  ({})  {}:{}",
            node.name(),
            label_node_kind(node.kind()),
            location.file(),
            location.line()
        ),
        None => format!("{}  ({})", node.name(), label_node_kind(node.kind())),
    }
}

// ── from graph identifiers to a renderable document ──────────────────

/// Turns the graph's hops into the shape the document reports.
///
/// The walk hands back node and edge identifiers; this attaches the evidence
/// each one stands for, and the refusals recorded near a break. Order is the
/// graph's — nothing is re-sorted here, because branch order is a traversal
/// decision and having two of them is how they disagree.
///
/// An identifier that does not resolve drops its hop rather than filling one
/// in. It cannot happen — the ids come from the graph being read, and
/// `cartograph_graph::trace`'s own test asserts every reported id resolves —
/// but if a graph were ever corrupt, a missing edge is the one thing that must
/// not be invented: an edge without real confidence, provenance and evidence
/// is exactly what RULES 004-006 exist to prevent.
fn render_steps_of(analysis: &Analysis, steps: &[TraceStep]) -> Vec<Step> {
    steps
        .iter()
        .filter_map(|step| {
            let next = render_steps_of(analysis, &step.next);

            // A break is only worth annotating when the chain stopped short of
            // data. Reaching a table is an ending, not a break.
            let refusals = if next.is_empty() && step.stop == Some(Stop::ChainEnd) {
                refusals_near(analysis, step.to)
            } else {
                Vec::new()
            };

            analysis.graph.edge(step.edge).map(|edge| Step {
                edge: json::json_edge(edge),
                to: node_json(&analysis.graph, step.to),
                next,
                stop: step.stop,
                refusals,
            })
        })
        .collect()
}

pub(crate) fn node_json(graph: &ArchitectureGraph, id: NodeId) -> JsonNode {
    graph.node(id).map_or_else(
        || JsonNode {
            id: id.as_u64(),
            kind: NodeKind::Module,
            name: "<unknown>".to_owned(),
            file: None,
            line: None,
        },
        |node| JsonNode {
            id: node.id().as_u64(),
            kind: node.kind(),
            name: node.name().to_owned(),
            file: node.location().map(|l| l.file().to_owned()),
            line: node.location().map(cartograph_core::SourceLocation::line),
        },
    )
}

/// Whether any branch reached a database table.
fn reaches_table(steps: &[Step]) -> bool {
    steps
        .iter()
        .any(|step| matches!(step.to.kind, NodeKind::Table) || reaches_table(&step.next))
}

/// Observations in the same file as `node` that the resolver refused.
///
/// Scoped to the file, and labelled as such. This does not claim the refusal
/// belongs to this symbol — it says something nearby was declined, which is
/// the difference between an honest break and a silent one.
fn refusals_near(analysis: &Analysis, node: NodeId) -> Vec<Refusal> {
    let Some(file) = analysis
        .graph
        .node(node)
        .and_then(cartograph_core::Node::location)
        .map(|l| l.file().to_owned())
    else {
        return Vec::new();
    };

    let mut refusals: Vec<Refusal> = analysis
        .results
        .iter()
        .filter(|result| result.client.provenance.file == file)
        .filter_map(|result| {
            let status = match result.status {
                MatchStatus::Ambiguous => "ambiguous",
                MatchStatus::NoMatch => "no-match",
                MatchStatus::Unsupported => "unsupported",
                _ => return None,
            };
            Some(Refusal {
                file: result.client.provenance.file.clone(),
                line: result.client.provenance.span.start_line,
                observation: result.client.to_string(),
                status,
                reason: result.unsupported_reason.map(|r| format!("{r:?}")),
            })
        })
        .collect();

    refusals.sort_by_key(|refusal| refusal.line);
    refusals.truncate(3);
    refusals
}

// ── rendering ───────────────────────────────────────────────────────

fn render_json(analysis: &Analysis, result: &TraceResult) -> Result<String, CliError> {
    let envelope = Envelope {
        schema_version: json::SCHEMA_VERSION,
        command: "trace",
        repository: &analysis.repository,
        languages: &analysis.languages,
        summary: Summary::from_totals(&analysis.totals, analysis.elapsed),
        nodes: json::nodes(&analysis.graph),
        edges: json::edges(&analysis.graph),
        diagnostics: &analysis.diagnostics,
        errors: Vec::new(),
        result: Some(result),
    };

    let mut text = serde_json::to_string_pretty(&envelope).map_err(|error| {
        CliError::new(
            ErrorCode::AnalysisFailed,
            "the trace could not be serialised as JSON",
        )
        .with_hint(format!("underlying cause: {error}"))
    })?;
    text.push('\n');
    Ok(text)
}

fn render_text(analysis: &Analysis, result: &TraceResult) -> String {
    let mut out = String::with_capacity(1024);

    let _ = writeln!(out, "{}", render_node(&result.from));
    render_steps(&mut out, &result.steps, "");

    if let Some(stop) = result.stop {
        let _ = writeln!(out, "  ⊘ {}", describe_stop(stop));
        render_refusals(&mut out, &result.refusals, "  ");
    }

    let _ = writeln!(out);
    if result.reaches_table {
        let _ = writeln!(
            out,
            "This path crosses the HTTP boundary and reaches a database table."
        );
    } else {
        let _ = writeln!(
            out,
            "This path does not reach a database table. Every hop shown is an"
        );
        let _ = writeln!(
            out,
            "edge the resolver produced; a missing hop was refused, not hidden."
        );
    }
    let _ = writeln!(
        out,
        "Confidence values are uncalibrated priors, not measured probabilities."
    );
    let _ = writeln!(out);
    let _ = writeln!(
        out,
        "Repository {} — {} nodes, {} edges, analysed in {:.2?}.",
        analysis.repository.name, analysis.totals.nodes, analysis.totals.edges, analysis.elapsed
    );

    out
}

/// Renders the hops below a node.
///
/// # Why branches are labelled
///
/// A node with two outgoing edges is not a chain, and printing its two
/// successors one after another would read as one — `A → B → C` when the graph
/// says `A → B` and `A → C`. That misreading is the whole failure mode this
/// command exists to avoid, so a fork announces itself and its branches are
/// indented under it.
///
/// `pad` is the column the *node* on this level starts at; the edge that leads
/// to it is drawn two columns further in, so the arrow always sits under the
/// node it leaves.
fn render_steps(out: &mut String, steps: &[Step], pad: &str) {
    let branching = steps.len() > 1;

    for (index, step) in steps.iter().enumerate() {
        let inner = if branching {
            let _ = writeln!(out, "{pad}├─ branch {} of {}", index + 1, steps.len());
            format!("{pad}   ")
        } else {
            pad.to_owned()
        };

        // The relationship, with the four attributes that make it checkable.
        let _ = writeln!(out, "{inner}  │");
        let _ = writeln!(
            out,
            "{inner}  ↓ {}   confidence {:.2}   provenance {}",
            label_edge_kind(step.edge.kind).to_uppercase(),
            step.edge.confidence,
            label_provenance(step.edge.provenance)
        );
        let _ = writeln!(out, "{inner}  │   evidence   {}", step.edge.evidence);
        let _ = writeln!(
            out,
            "{inner}  │   observed   {}:{}",
            step.edge.file, step.edge.line
        );
        let _ = writeln!(out, "{inner}  │");
        let _ = writeln!(out, "{inner}{}", render_node(&step.to));

        if let Some(stop) = step.stop {
            // A chain that ends at a table has arrived, not stalled.
            if !(stop == Stop::ChainEnd && matches!(step.to.kind, NodeKind::Table)) {
                let _ = writeln!(out, "{inner}  ⊘ {}", describe_stop(stop));
            }
            render_refusals(out, &step.refusals, &format!("{inner}  "));
        }

        render_steps(out, &step.next, &inner);
    }
}

fn render_refusals(out: &mut String, refusals: &[Refusal], pad: &str) {
    if refusals.is_empty() {
        return;
    }
    let _ = writeln!(
        out,
        "{pad}refused in the same file (not necessarily from this symbol):"
    );
    for refusal in refusals {
        let reason = refusal
            .reason
            .as_ref()
            .map(|r| format!(" — {r}"))
            .unwrap_or_default();
        let _ = writeln!(
            out,
            "{pad}  {}:{}  {}  [{}{}]",
            refusal.file, refusal.line, refusal.observation, refusal.status, reason
        );
    }
}

fn render_node(node: &JsonNode) -> String {
    match (&node.file, node.line) {
        (Some(file), Some(line)) => format!(
            "{}  ({})  {file}:{line}",
            node.name,
            label_node_kind(node.kind)
        ),
        _ => format!("{}  ({})", node.name, label_node_kind(node.kind)),
    }
}

fn label_node_kind(kind: NodeKind) -> &'static str {
    match kind {
        NodeKind::Repository => "repository",
        NodeKind::Package => "package",
        NodeKind::Directory => "directory",
        NodeKind::File => "file",
        NodeKind::Module => "module",
        NodeKind::Class => "class",
        NodeKind::Function => "function",
        NodeKind::Method => "method",
        NodeKind::Variable => "variable",
        NodeKind::Route => "route",
        NodeKind::Table => "table",
        NodeKind::Column => "column",
        NodeKind::ExternalService => "external service",
        NodeKind::EnvVar => "env var",
        _ => "node",
    }
}

fn label_edge_kind(kind: EdgeKind) -> &'static str {
    match kind {
        EdgeKind::Import => "import",
        EdgeKind::Call => "call",
        EdgeKind::Inherits => "inherits",
        EdgeKind::Implements => "implements",
        EdgeKind::References => "references",
        EdgeKind::HttpCall => "http-call",
        EdgeKind::OrmAccess => "orm-access",
        EdgeKind::Queries => "queries",
        _ => "edge",
    }
}

fn label_provenance(provenance: Provenance) -> &'static str {
    match provenance {
        Provenance::LspSymbolResolution => "lsp-symbol",
        Provenance::RouteMatcher => "route-matcher",
        Provenance::OpenApiSpec => "openapi-spec",
        Provenance::StaticImportResolution => "static-import",
        Provenance::ConstantPropagation => "constant-propagation",
        Provenance::TemplateEvaluation => "template-evaluation",
        Provenance::OrmResolution => "orm-resolution",
        Provenance::ModelInference => "model-inference",
        _ => "other",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Path-hint matching, the traversal and the serialised stop slugs are
    // tested in `cartograph_graph::trace`, which now owns them.

    #[test]
    fn every_stop_reason_explains_itself() {
        for stop in [Stop::ChainEnd, Stop::MaxDepth, Stop::Cycle] {
            assert!(!describe_stop(stop).is_empty());
        }
        assert!(describe_stop(Stop::MaxDepth).contains("may continue"));
    }

    /// The flag's default and the traversal's default are the same number.
    #[test]
    fn the_depth_default_is_the_graph_layer_default() {
        assert_eq!(DEFAULT_MAX_DEPTH, trace::DEFAULT_MAX_DEPTH);
    }

    #[test]
    fn every_node_kind_has_a_label() {
        for kind in [
            NodeKind::Repository,
            NodeKind::Package,
            NodeKind::Directory,
            NodeKind::File,
            NodeKind::Module,
            NodeKind::Class,
            NodeKind::Function,
            NodeKind::Method,
            NodeKind::Variable,
            NodeKind::Route,
            NodeKind::Table,
            NodeKind::Column,
            NodeKind::ExternalService,
            NodeKind::EnvVar,
        ] {
            assert_ne!(label_node_kind(kind), "node", "{kind:?} has no label");
        }
    }
}
