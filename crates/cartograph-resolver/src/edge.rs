//! Turning an accepted match into a graph edge.
//!
//! This is the first place in Cartograph permitted to assert a semantic
//! cross-stack relationship, and the last stage of the resolver pipeline:
//!
//! ```text
//! extraction → canonicalisation → matching → classification → EDGE
//! ```
//!
//! # Edges are built only from accepted matches
//!
//! [`MatchStatus::is_accepted`] gates every edge. An ambiguous result produces
//! no edge no matter how strong its candidates look, because the question of
//! *which* route was reached is unanswered — and an edge is an answer.
//!
//! # Every edge explains itself
//!
//! The domain model already refuses an edge without confidence, provenance,
//! evidence and a source location. This module supplies all four from the
//! match that produced it, so a reader can reconstruct the decision:
//!
//! ```text
//! POST /api/orders → create_order  0.98  route-matcher
//!   client   web/src/CheckoutButton.tsx:34   POST /api/orders
//!   backend  api/orders.py:12                POST /api/orders
//!   because  exact method; every path segment static and equal
//! ```

use std::fmt::Write as _;

use cartograph_core::{CommitId, EdgeKind, Evidence, NodeId, NodeKind, Provenance, SourceLocation};
use cartograph_graph::{ArchitectureGraph, EdgeSpec, GraphError};

use crate::canonical::ObservationKind;
use crate::matching::{Candidate, MatchReason, MatchResult, MethodCompatibility};

/// Adds the edge for an accepted match, creating its endpoint nodes.
///
/// Returns `Ok(None)` when the result was not accepted — that is the ordinary
/// outcome for ambiguous, unmatched and unsupported results, not an error.
///
/// # Node choices
///
/// - **Source**: a [`NodeKind::File`] for the client file. Cartograph does not
///   yet resolve the enclosing function of a call site, so naming one would be
///   a guess. The precise call site is not lost: it is the edge's own
///   `location`, which is exactly how the specification writes a client
///   endpoint (`CheckoutButton.tsx:34`).
/// - **Target**: a [`NodeKind::Function`] named after the route's handler when
///   M02 extracted one — the handler is the decorated function, read
///   syntactically, not inferred. When no handler is available the target is a
///   [`NodeKind::Route`] named by its canonical form, and the limitation is
///   visible in the graph rather than papered over.
///
/// # Errors
///
/// Returns [`GraphError`] if the graph rejects an endpoint, which cannot
/// happen for nodes created here and is propagated rather than ignored.
pub fn add_edge_for_match(
    graph: &mut ArchitectureGraph,
    result: &MatchResult,
    commit: Option<CommitId>,
) -> Result<Option<cartograph_core::EdgeId>, GraphError> {
    let Some(candidate) = result.accepted() else {
        return Ok(None);
    };
    debug_assert_eq!(result.client.provenance.kind, ObservationKind::ClientCall);

    let source = client_node(graph, result)?;
    let target = backend_node(graph, candidate)?;

    let id = graph.add_edge(EdgeSpec {
        source,
        target,
        kind: EdgeKind::HttpCall,
        confidence: candidate.confidence,
        // Deterministic static matching produced this, so the provenance is
        // the route matcher. Not LSP: no language server was consulted. Not
        // model inference: no model was consulted, and none ever will be
        // (RULE 007).
        provenance: Provenance::RouteMatcher,
        evidence: evidence_for(result, candidate),
        // The claim was observed at the client call site.
        location: client_location(result)?,
        commit,
    })?;
    Ok(Some(id))
}

fn client_node(graph: &mut ArchitectureGraph, result: &MatchResult) -> Result<NodeId, GraphError> {
    let provenance = &result.client.provenance;
    let file = &provenance.file;

    // The function the call sits inside, when the extractor found one. A
    // generated client wraps every request in a function that components
    // import and call, and naming it keeps the wrapper on the path instead of
    // attributing the request to a whole file. This is no longer a guess:
    // M01/M02 record the enclosing scope of a call site.
    //
    // A call at module scope still yields a File node, which is what every
    // client observation produced before M07's remediation.
    let (kind, name, line) = provenance.enclosing.as_ref().map_or_else(
        || (NodeKind::File, file.clone(), 1),
        |function| {
            (
                NodeKind::Function,
                function.clone(),
                provenance.span.start_line,
            )
        },
    );
    let location = SourceLocation::new(file.clone(), line).ok();
    graph
        .node_for(kind, name, location)
        .map_err(|_| GraphError::UnknownNode {
            id: NodeId::from_raw(u64::MAX),
        })
}

fn backend_node(
    graph: &mut ArchitectureGraph,
    candidate: &Candidate,
) -> Result<NodeId, GraphError> {
    let provenance = &candidate.route.provenance;
    // The route declaration's own position. For a decorator this is the
    // decorator line rather than the `def` line — a one-line difference that
    // is recorded honestly rather than adjusted by guesswork.
    let location = SourceLocation::new(provenance.file.clone(), provenance.span.start_line).ok();

    let (kind, name) = match &provenance.handler {
        Some(handler) => (NodeKind::Function, handler.clone()),
        None => (NodeKind::Route, candidate.route.to_string()),
    };
    graph
        .node_for(kind, name, location)
        .map_err(|_| GraphError::UnknownNode {
            id: NodeId::from_raw(u64::MAX),
        })
}

fn client_location(result: &MatchResult) -> Result<SourceLocation, GraphError> {
    let provenance = &result.client.provenance;
    SourceLocation::new(provenance.file.clone(), provenance.span.start_line)
        .or_else(|_| SourceLocation::new(provenance.file.clone(), 1))
        .map_err(|_| GraphError::UnknownNode {
            id: NodeId::from_raw(u64::MAX),
        })
}

/// Builds the human-readable case for an edge.
///
/// Written to be read by someone asking why two things are connected, so it
/// names both sides, both canonical forms, and the rules that fired. It never
/// contains source text — only paths, positions and canonical routes
/// (RULE 015).
///
/// # Panics
///
/// Cannot panic in practice: the fallback literal is non-empty, which is the
/// only condition [`Evidence::new`] rejects.
#[must_use]
pub fn evidence_for(result: &MatchResult, candidate: &Candidate) -> Evidence {
    let client = &result.client;
    let backend = &candidate.route;

    let mut summary = format!(
        "{} matched {} at {}:{}",
        client, backend, backend.provenance.file, backend.provenance.span.start_line
    );
    if let Some(handler) = &backend.provenance.handler {
        let _ = write!(summary, " ({handler})");
    }
    summary.push_str("; ");
    summary.push_str(&describe(&candidate.reasons, candidate.method));

    // Evidence rejects an empty summary, and the format above cannot be empty.
    Evidence::new(summary)
        .unwrap_or_else(|_| Evidence::new("route matched").expect("non-empty literal"))
}

/// Renders match reasons as a sentence.
fn describe(reasons: &[MatchReason], method: MethodCompatibility) -> String {
    let mut parts: Vec<String> = Vec::new();
    let mut static_positions = 0;
    let mut bound = Vec::new();
    let mut unknown = Vec::new();
    let mut unverified = Vec::new();

    for reason in reasons {
        match reason {
            MatchReason::MethodExact => parts.push("exact HTTP method".into()),
            MatchReason::MethodUndeclared(_) => parts.push(match method {
                MethodCompatibility::ClientUnknown => "client declared no method".into(),
                MethodCompatibility::ServerUnknown => "route declared no method".into(),
                _ => "neither side declared a method".into(),
            }),
            MatchReason::StaticPathExact => {
                parts.push("every path segment static and equal".into());
            }
            MatchReason::StaticSegmentsAgree(_) => static_positions += 1,
            MatchReason::ParameterBoundToLiteral(i) => bound.push(*i),
            MatchReason::ClientValueUnknown(i) => unknown.push(*i),
            MatchReason::UnknownValueAgainstLiteral(i) => unverified.push(*i),
            MatchReason::WildcardConsumedSuffix(n) => {
                parts.push(format!("route wildcard consumed {n} trailing segment(s)"));
            }
            MatchReason::TrailingSlashDiffers => parts.push("trailing slash differs".into()),
            MatchReason::NoDiscriminatingStaticSegment => {
                parts.push("no shared static segment".into());
            }
        }
    }

    if static_positions > 0 {
        parts.push(format!("{static_positions} static segment(s) equal"));
    }
    if !bound.is_empty() {
        parts.push(format!(
            "declared parameter(s) at position(s) {} bound to concrete values",
            join(&bound)
        ));
    }
    if !unknown.is_empty() {
        parts.push(format!(
            "client value(s) at position(s) {} not determined",
            join(&unknown)
        ));
    }
    if !unverified.is_empty() {
        parts.push(format!(
            "client value(s) at position(s) {} compared against literal segment(s), unverified",
            join(&unverified)
        ));
    }
    parts.join("; ")
}

fn join(positions: &[usize]) -> String {
    positions
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join(", ")
}
