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
//! # The path comes out of the graph
//!
//! This module walks edges with [`ArchitectureGraph::edges_from`]. It does not
//! re-derive a chain from the match results, or from the source, or from
//! anything else. If the rendered path and the graph disagree, the rendering
//! is wrong — there is no second opinion to consult. Every hop reports the
//! confidence, provenance, evidence and location that the edge itself carries.
//!
//! # Ambiguity is not resolved by choosing
//!
//! Two symbols named `User` in different files are two different things. This
//! command lists both and exits [`ExitCode::Ambiguous`] rather than tracing
//! whichever the graph happened to number first — picking one would make the
//! output depend on iteration order, which is exactly the class of quiet
//! wrongness the project's refusal rules exist to prevent.

use std::collections::HashSet;
use std::fmt::Write as _;

use cartograph_core::{EdgeKind, NodeId, NodeKind, Provenance};
use cartograph_graph::ArchitectureGraph;
use cartograph_resolver::MatchStatus;
use serde::Serialize;

use crate::error::{CliError, ErrorCode, ExitCode};
use crate::json::{self, Envelope, JsonEdge, JsonNode, Summary};
use crate::pipeline::{self, Analysis};

/// How deep a trace walks before stopping.
///
/// A frontend → client → route → handler → model → table chain is five hops.
/// Ten leaves room for longer real chains while bounding output on a graph
/// with a long import spine.
pub const DEFAULT_MAX_DEPTH: usize = 10;

/// Why a walk stopped.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
enum Stop {
    /// The node has no outgoing edge. The chain genuinely ends here.
    ChainEnd,
    /// `--max-depth` was reached; more of the chain may exist.
    MaxDepth,
    /// The walk returned to a node already on this path.
    Cycle,
}

impl Stop {
    fn describe(self) -> &'static str {
        match self {
            Self::ChainEnd => "chain ends here — no outgoing edge from this node",
            Self::MaxDepth => "stopped at --max-depth — the chain may continue",
            Self::Cycle => "stopped — this node is already on the path",
        }
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

    let mut visited = HashSet::new();
    visited.insert(start);
    let (steps, stop) = walk(&analysis, start, max_depth, &mut visited);

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
        stop,
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

/// Finds the single node a query names.
///
/// A bare name matches on the node's name. A query containing `:` is read as
/// `file:name` — `orders.py:create_order` — which is how a user disambiguates
/// without needing to know node ids.
fn resolve_symbol(graph: &ArchitectureGraph, query: &str) -> Result<NodeId, CliError> {
    let (file_hint, name) = match query.rsplit_once(':') {
        Some((file, name)) if !file.is_empty() && !name.is_empty() => (Some(file), name),
        _ => (None, query),
    };

    let mut matches: Vec<&cartograph_core::Node> = graph
        .nodes()
        .filter(|node| node.name() == name)
        .filter(|node| match file_hint {
            None => true,
            Some(hint) => node
                .location()
                .is_some_and(|location| path_matches(location.file(), hint)),
        })
        .collect();

    // Deterministic ordering: the candidate list a user sees must not depend
    // on graph iteration order.
    matches.sort_by(|a, b| {
        let key = |n: &cartograph_core::Node| {
            (
                n.location()
                    .map(|l| l.file().to_owned())
                    .unwrap_or_default(),
                n.location()
                    .map_or(0, cartograph_core::SourceLocation::line),
                n.id().as_u64(),
            )
        };
        key(a).cmp(&key(b))
    });

    match matches.as_slice() {
        [] => Err(not_found(graph, query, name)),
        [only] => Ok(only.id()),
        many => Err(CliError::new(
            ErrorCode::AmbiguousSymbol,
            format!("`{query}` matches {} symbols", many.len()),
        )
        .with_candidates(many.iter().map(|n| describe_node(n)).collect())
        .with_hint(
            "trace one of them by qualifying the name with its file, for example \
             `cartograph trace path/to/file.py:name`",
        )),
    }
}

/// Whether a node's repository-relative path satisfies a user's file hint.
///
/// Suffix matching on path segments: `orders.py` matches `app/api/orders.py`,
/// and `api/orders.py` matches it too, but `ders.py` does not.
fn path_matches(file: &str, hint: &str) -> bool {
    let hint = hint.trim_start_matches("./").replace('\\', "/");
    if file == hint {
        return true;
    }
    file.strip_suffix(hint.as_str())
        .is_some_and(|prefix| prefix.is_empty() || prefix.ends_with('/'))
}

/// The not-found error, with near-misses when there are any.
fn not_found(graph: &ArchitectureGraph, query: &str, name: &str) -> CliError {
    // Case-insensitive equality first, then substring: the two mistakes a
    // person actually makes. Not a general edit-distance search — suggesting
    // `Order` for `Odrer` is guessing, and this command does not guess.
    let mut near: Vec<String> = graph
        .nodes()
        .filter(|node| {
            node.name().eq_ignore_ascii_case(name)
                || (name.len() >= 3 && node.name().contains(name))
        })
        .map(describe_node)
        .collect();
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

// ── traversal ───────────────────────────────────────────────────────

/// Walks outward from `from`, depth-first, following graph edges only.
fn walk(
    analysis: &Analysis,
    from: NodeId,
    remaining: usize,
    visited: &mut HashSet<NodeId>,
) -> (Vec<Step>, Option<Stop>) {
    let graph = &analysis.graph;

    let mut outgoing: Vec<&cartograph_core::Edge> = graph.edges_from(from).collect();
    if outgoing.is_empty() {
        return (Vec::new(), Some(Stop::ChainEnd));
    }
    if remaining == 0 {
        return (Vec::new(), Some(Stop::MaxDepth));
    }

    // Deterministic branch order, independent of insertion order.
    outgoing.sort_by_key(|edge| {
        (
            edge.kind() as u8,
            edge.target().as_u64(),
            edge.id().as_u64(),
        )
    });

    let mut steps = Vec::with_capacity(outgoing.len());
    for edge in outgoing {
        let target = edge.target();
        let step = if visited.contains(&target) {
            Step {
                edge: json::json_edge(edge),
                to: node_json(graph, target),
                next: Vec::new(),
                stop: Some(Stop::Cycle),
                refusals: Vec::new(),
            }
        } else {
            visited.insert(target);
            let (next, stop) = walk(analysis, target, remaining - 1, visited);
            visited.remove(&target);

            // A break is only worth annotating when the chain stopped short of
            // data. Reaching a table is an ending, not a break.
            let refusals = if next.is_empty() && stop == Some(Stop::ChainEnd) {
                refusals_near(analysis, target)
            } else {
                Vec::new()
            };

            Step {
                edge: json::json_edge(edge),
                to: node_json(graph, target),
                next,
                stop,
                refusals,
            }
        };
        steps.push(step);
    }

    (steps, None)
}

fn node_json(graph: &ArchitectureGraph, id: NodeId) -> JsonNode {
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
        let _ = writeln!(out, "  ⊘ {}", stop.describe());
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
                let _ = writeln!(out, "{inner}  ⊘ {}", stop.describe());
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

    #[test]
    fn a_file_hint_matches_on_whole_path_segments() {
        assert!(path_matches("app/api/orders.py", "orders.py"));
        assert!(path_matches("app/api/orders.py", "api/orders.py"));
        assert!(path_matches("app/api/orders.py", "app/api/orders.py"));
        assert!(path_matches("orders.py", "orders.py"));
        assert!(path_matches("app/api/orders.py", "./orders.py"));
    }

    #[test]
    fn a_file_hint_does_not_match_a_partial_segment() {
        // `ders.py` must not match `orders.py`: a hint that matched half a
        // filename would silently pick the wrong symbol.
        assert!(!path_matches("app/api/orders.py", "ders.py"));
        assert!(!path_matches("app/api/orders.py", "reorders.py"));
        assert!(!path_matches("app/api/orders.py", "other.py"));
    }

    #[test]
    fn every_stop_reason_explains_itself() {
        for stop in [Stop::ChainEnd, Stop::MaxDepth, Stop::Cycle] {
            assert!(!stop.describe().is_empty());
        }
        assert!(Stop::MaxDepth.describe().contains("may continue"));
    }

    #[test]
    fn stop_reasons_serialise_as_stable_slugs() {
        assert_eq!(
            serde_json::to_string(&Stop::ChainEnd).unwrap(),
            "\"chain-end\""
        );
        assert_eq!(
            serde_json::to_string(&Stop::MaxDepth).unwrap(),
            "\"max-depth\""
        );
        assert_eq!(serde_json::to_string(&Stop::Cycle).unwrap(), "\"cycle\"");
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
