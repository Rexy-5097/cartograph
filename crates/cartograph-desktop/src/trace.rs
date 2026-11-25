//! TRACE for a window: the same walk `cartograph trace` performs.
//!
//! **Experimental (M18 browser explorer; proposed ADR-0024).** The walk is
//! [`cartograph_graph::trace::trace`], unchanged. This module checks that the
//! request is about the analysis on screen, as [`crate::blast`] does, and turns
//! the result into identifiers the window can highlight. The edges' evidence
//! stays behind the evidence lookup, so the payload cannot drift from it.

use cartograph_core::{EdgeId, NodeId};
use cartograph_graph::trace::{DEFAULT_MAX_DEPTH, Stop, TraceStep, trace};
use serde::Serialize;

use crate::error::{DesktopError, DesktopErrorKind};
use crate::evidence::{AnalysisId, AnalysisSession};

/// One hop of a trace, and the hops beyond it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TraceHop {
    /// The relationship traversed.
    pub edge: EdgeId,
    /// Where it led.
    pub to: NodeId,
    /// Why the walk stopped here, when it did.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop: Option<Stop>,
    /// Further hops.
    pub next: Vec<TraceHop>,
}

/// A completed trace from one artefact.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TracePayload {
    /// The analysis it belongs to.
    pub analysis: AnalysisId,
    /// Where the walk started.
    pub from: NodeId,
    /// The depth limit in force, as `cartograph trace` uses by default.
    pub max_depth: usize,
    /// Why the walk stopped at once, when it did.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop: Option<Stop>,
    /// The hops.
    pub steps: Vec<TraceHop>,
}

/// Traces outward from `node` in the session's graph.
///
/// # Errors
///
/// [`DesktopErrorKind::StaleSelection`] when the request names another
/// analysis, and [`DesktopErrorKind::UnknownNode`] when the node is not in
/// this one.
pub fn walk(
    session: &AnalysisSession,
    analysis: AnalysisId,
    node: NodeId,
) -> Result<TracePayload, DesktopError> {
    if analysis != session.id() {
        return Err(DesktopError::new(
            DesktopErrorKind::StaleSelection,
            "That selection belongs to an earlier analysis.",
        )
        .with_hint("Select the artefact again on the current map."));
    }
    let graph = session.graph();
    if graph.node(node).is_none() {
        return Err(DesktopError::new(
            DesktopErrorKind::UnknownNode,
            "That artefact is not part of this analysis.",
        )
        .with_hint("Select the artefact again on the current map."));
    }
    let computed = trace(graph, node, DEFAULT_MAX_DEPTH);
    Ok(TracePayload {
        analysis,
        from: computed.from,
        max_depth: computed.max_depth,
        stop: computed.stop,
        steps: computed.steps.iter().map(hop).collect(),
    })
}

fn hop(step: &TraceStep) -> TraceHop {
    TraceHop {
        edge: step.edge,
        to: step.to,
        stop: step.stop,
        next: step.next.iter().map(hop).collect(),
    }
}
