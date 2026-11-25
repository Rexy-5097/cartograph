//! One artefact, looked up on demand: where it is and what it touches.
//!
//! **Experimental (M18 browser explorer; proposed ADR-0024).** The scene
//! (ADR-0015) deliberately carries no file or line, and this does not widen it:
//! a window that wants a node's location asks for it, as it asks for an edge's
//! evidence. The relationships listed are the graph's own edges, read with
//! [`cartograph_graph::ArchitectureGraph::edges_from`] and `edges_to`.

use cartograph_core::{EdgeId, EdgeKind, NodeId, NodeKind};
use serde::Serialize;

use crate::error::{DesktopError, DesktopErrorKind};
use crate::evidence::{AnalysisId, AnalysisSession};

/// One relationship of the inspected artefact.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Neighbour {
    /// The relationship.
    pub edge: EdgeId,
    /// Its kind.
    pub kind: EdgeKind,
    /// The artefact at the other end.
    pub node: NodeId,
    /// That artefact's name.
    pub label: String,
    /// That artefact's kind.
    pub node_kind: NodeKind,
}

/// An artefact, with its location and its relationships.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NodeDetail {
    /// The analysis it belongs to.
    pub analysis: AnalysisId,
    /// The artefact.
    pub node: NodeId,
    /// Its name.
    pub label: String,
    /// Its kind.
    pub kind: NodeKind,
    /// The file it is declared in, repository-relative, when it has one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file: Option<String>,
    /// Its line, when it has one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line: Option<u32>,
    /// Relationships from it.
    pub outgoing: Vec<Neighbour>,
    /// Relationships to it.
    pub incoming: Vec<Neighbour>,
}

/// Looks one artefact up.
///
/// # Errors
///
/// [`DesktopErrorKind::StaleSelection`] for another analysis and
/// [`DesktopErrorKind::UnknownNode`] for an artefact not in this one.
pub fn node(
    session: &AnalysisSession,
    analysis: AnalysisId,
    id: NodeId,
) -> Result<NodeDetail, DesktopError> {
    if analysis != session.id() {
        return Err(DesktopError::new(
            DesktopErrorKind::StaleSelection,
            "That selection belongs to an earlier analysis.",
        )
        .with_hint("Select the artefact again on the current map."));
    }
    let graph = session.graph();
    let found = graph.node(id).ok_or_else(|| {
        DesktopError::new(
            DesktopErrorKind::UnknownNode,
            "That artefact is not part of this analysis.",
        )
        .with_hint("Select the artefact again on the current map.")
    })?;
    let neighbour = |edge: EdgeId, kind: EdgeKind, other: NodeId| {
        graph.node(other).map(|n| Neighbour {
            edge,
            kind,
            node: other,
            label: n.name().to_owned(),
            node_kind: n.kind(),
        })
    };
    Ok(NodeDetail {
        analysis,
        node: id,
        label: found.name().to_owned(),
        kind: found.kind(),
        file: found.location().map(|l| l.file().to_owned()),
        line: found.location().map(cartograph_core::SourceLocation::line),
        outgoing: graph
            .edges_from(id)
            .filter_map(|e| neighbour(e.id(), e.kind(), e.target()))
            .collect(),
        incoming: graph
            .edges_to(id)
            .filter_map(|e| neighbour(e.id(), e.kind(), e.source()))
            .collect(),
    })
}
