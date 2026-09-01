//! The machine-readable output contract.
//!
//! # Why this is versioned
//!
//! `--json` is the integration boundary. Anything that consumes a Cartograph
//! graph without parsing terminal prose — an editor plugin, CI, a future
//! desktop client — reads this shape, and those consumers cannot be updated in
//! lockstep with the analyser. So the contract carries [`SCHEMA_VERSION`], and
//! changing a field's meaning means changing that number.
//!
//! # Why the edge shape is what it is
//!
//! `A -> B` is not a serialisable edge in this project. Every edge here
//! carries the four attributes the domain model requires — confidence,
//! provenance, evidence, location — because an edge that cannot explain itself
//! is not a fact, and a consumer that receives only the endpoints has been
//! given a claim without the case for it.
//!
//! Node and edge *identity* is exposed as `id`, and `source`/`target` are
//! those ids. That is deliberate: two edges naming the same handler are part
//! of one chain only if they share a node id, and no name-based encoding can
//! express that (ADR-0011).
//!
//! # Versions
//!
//! **1.0** — the shape M09 published. **1.1** — M12 adds the `blast` command,
//! whose payload occupies the same `result` slot `trace` already used. The
//! change is purely additive: `result` became a `oneOf` discriminated by
//! `command`, and the `trace` branch is byte-identical to 1.0, so a consumer
//! written against 1.0 keeps validating every `trace` document unchanged.
//!
//! # Compatibility
//!
//! The `parse`, `normalize` and `match` payload bodies predate this module and
//! are consumed by the frozen M07/M08 benchmark harness. This module supplies
//! exactly the node and edge shape those payloads already used, so adding the
//! envelope around them is additive: existing readers keep working.

use cartograph_core::{EdgeKind, NodeKind, Provenance};
use cartograph_graph::ArchitectureGraph;
use serde::Serialize;

use crate::error::CliError;
use crate::pipeline::{FileDiagnostic, Languages, Repository, Totals};

/// The version of this output contract.
///
/// Bump the minor part for additive changes, the major part for anything a
/// consumer could be broken by. Documented in `docs/architecture/json-schema.md`.
pub const SCHEMA_VERSION: &str = "1.1";

/// One graph node.
#[derive(Debug, Clone, Serialize)]
pub struct JsonNode {
    /// Stable within one run. Referenced by [`JsonEdge::source`] and
    /// [`JsonEdge::target`].
    pub id: u64,
    /// What kind of artefact this is.
    pub kind: NodeKind,
    /// The symbol, route, model or table name.
    pub name: String,
    /// Repository-relative path, when the node has a source location.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file: Option<String>,
    /// One-based line, when the node has a source location.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line: Option<u32>,
}

/// One graph edge, with the case for believing it.
#[derive(Debug, Clone, Serialize)]
pub struct JsonEdge {
    /// Stable within one run.
    pub id: u64,
    /// [`JsonNode::id`] of the origin.
    pub source: u64,
    /// [`JsonNode::id`] of the destination.
    pub target: u64,
    /// The relationship.
    pub kind: EdgeKind,
    /// The specification's prior for this evidence class. Uncalibrated — see
    /// `docs/benchmarks/m08-confidence-policy.md`.
    pub confidence: f32,
    /// Which analysis produced it.
    pub provenance: Provenance,
    /// A structural description of the observation. Never source text
    /// (RULE 015).
    pub evidence: String,
    /// Repository-relative path where the claim was observed.
    pub file: String,
    /// One-based line where the claim was observed.
    pub line: u32,
}

/// Serialises every node, in id order.
///
/// Id order rather than discovery order so two runs over the same tree produce
/// byte-identical output, which is what makes a golden fixture and a benchmark
/// comparison meaningful.
pub fn nodes(graph: &ArchitectureGraph) -> Vec<JsonNode> {
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
    nodes
}

/// Serialises every edge, in id order.
pub fn edges(graph: &ArchitectureGraph) -> Vec<JsonEdge> {
    let mut edges: Vec<JsonEdge> = graph.edges().map(json_edge).collect();
    edges.sort_by_key(|e| e.id);
    edges
}

/// Serialises one edge.
pub fn json_edge(edge: &cartograph_core::Edge) -> JsonEdge {
    JsonEdge {
        id: edge.id().as_u64(),
        source: edge.source().as_u64(),
        target: edge.target().as_u64(),
        kind: edge.kind(),
        confidence: edge.confidence().get(),
        provenance: edge.provenance(),
        evidence: edge.evidence().as_str().to_owned(),
        file: edge.location().file().to_owned(),
        line: edge.location().line(),
    }
}

/// The counts a machine consumer reads, in the vocabulary the summary uses.
#[derive(Debug, Clone, Serialize)]
pub struct Summary {
    /// Files opened and parsed.
    pub files: usize,
    /// Declared symbols across every parsed file.
    pub symbols: usize,
    /// Route declarations observed in source.
    pub routes_observed: usize,
    /// Syntactically HTTP-looking call sites observed in source.
    pub http_observed: usize,
    /// ORM models discovered.
    pub orm_models: usize,
    /// ORM models that resolved to a table.
    pub tables_resolved: usize,
    /// Nodes in the produced graph.
    pub nodes: usize,
    /// Edges in the produced graph.
    pub edges: usize,
    /// Edges whose endpoints were extracted by different grammars.
    pub cross_language_edges: usize,
    /// Client observations that produced an `HttpCall` edge.
    pub http_call_edges: usize,
    /// `Call` edges into a function that issues a request.
    pub client_call_edges: usize,
    /// `OrmAccess` and `Queries` edges.
    pub orm_edges: usize,
    /// Observations with rival candidates, which produce no edge.
    pub ambiguous: usize,
    /// Observations with no candidate route at all.
    pub no_match: usize,
    /// Observations outside the supported subset.
    pub unsupported: usize,
    /// Observations the resolver declined to turn into an edge, in total.
    pub refused: usize,
    /// Parse diagnostics across every file.
    pub diagnostics: usize,
    /// Files that could not be parsed at all.
    pub files_failed: usize,
    /// Wall-clock milliseconds for extraction, resolution and construction.
    pub duration_ms: u128,
}

impl Summary {
    /// Projects the pipeline's totals onto the published contract.
    #[must_use]
    pub fn from_totals(totals: &Totals, elapsed: std::time::Duration) -> Self {
        Self {
            files: totals.files,
            symbols: totals.symbols,
            routes_observed: totals.routes,
            http_observed: totals.http_observations,
            orm_models: totals.orm_models,
            tables_resolved: totals.orm_tables,
            nodes: totals.nodes,
            edges: totals.edges,
            cross_language_edges: totals.cross_language_edges,
            http_call_edges: totals.http_call_edges,
            client_call_edges: totals.client_call_edges,
            orm_edges: totals.orm_edges,
            ambiguous: totals.ambiguous,
            no_match: totals.no_match,
            unsupported: totals.unsupported,
            refused: totals.refused(),
            diagnostics: totals.diagnostics,
            files_failed: totals.files_failed,
            duration_ms: elapsed.as_millis(),
        }
    }
}

/// The top-level `--json` document.
///
/// Command-specific sections are omitted rather than emitted empty, so a
/// consumer can tell "this command does not produce traces" from "this trace
/// found nothing".
#[derive(Debug, Serialize)]
pub struct Envelope<'a, T: Serialize> {
    /// The version of this contract.
    pub schema_version: &'static str,
    /// Which command produced the document.
    pub command: &'static str,
    /// What was analysed. Never an absolute path.
    pub repository: &'a Repository,
    /// Files per language.
    pub languages: &'a Languages,
    /// The counts.
    pub summary: Summary,
    /// Every node in the produced graph.
    pub nodes: Vec<JsonNode>,
    /// Every edge in the produced graph, each with its own evidence.
    pub edges: Vec<JsonEdge>,
    /// Parse problems. Recoverable unless `files_failed` is non-zero.
    pub diagnostics: &'a [FileDiagnostic],
    /// Failures. Empty on success; a successful document always has this key
    /// so a consumer can check one field unconditionally.
    pub errors: Vec<&'a CliError>,
    /// The command-specific section, when the command has one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<T>,
}

/// The `--json` document for a command that failed before producing a graph.
///
/// Emitted on stdout so a consumer parsing `--json` output always receives
/// JSON, whether the run succeeded or not.
#[derive(Debug, Serialize)]
pub struct ErrorEnvelope<'a> {
    /// The version of this contract.
    pub schema_version: &'static str,
    /// Which command was attempted.
    pub command: &'static str,
    /// The failures. Never empty in this document.
    pub errors: Vec<&'a CliError>,
}

impl<'a> ErrorEnvelope<'a> {
    /// Wraps one failure.
    #[must_use]
    pub fn new(command: &'static str, error: &'a CliError) -> Self {
        Self {
            schema_version: SCHEMA_VERSION,
            command,
            errors: vec![error],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::ErrorCode;

    #[test]
    fn the_schema_version_is_declared() {
        assert_eq!(SCHEMA_VERSION, "1.1");
    }

    #[test]
    fn an_error_document_is_valid_json_with_the_schema_version() {
        let error = CliError::new(ErrorCode::SymbolNotFound, "no such symbol");
        let envelope = ErrorEnvelope::new("trace", &error);
        let text = serde_json::to_string(&envelope).expect("serialises");
        let value: serde_json::Value = serde_json::from_str(&text).expect("round-trips");

        assert_eq!(value["schema_version"], "1.1");
        assert_eq!(value["command"], "trace");
        assert_eq!(value["errors"][0]["code"], "symbol-not-found");
        assert_eq!(value["errors"][0]["message"], "no such symbol");
    }

    #[test]
    fn an_edge_never_serialises_as_only_its_endpoints() {
        // The rule this project is built on: no edge without its evidence.
        let edge = JsonEdge {
            id: 1,
            source: 2,
            target: 3,
            kind: EdgeKind::HttpCall,
            confidence: 0.98,
            provenance: Provenance::RouteMatcher,
            evidence: "POST /api/orders".to_owned(),
            file: "src/api.ts".to_owned(),
            line: 30,
        };
        let value = serde_json::to_value(&edge).expect("serialises");

        for required in [
            "source",
            "target",
            "kind",
            "confidence",
            "provenance",
            "evidence",
            "file",
            "line",
        ] {
            assert!(
                value.get(required).is_some(),
                "edge lost its `{required}` field"
            );
        }
        assert_eq!(value["provenance"], "route-matcher");
        assert_eq!(value["kind"], "http-call");
    }
}
