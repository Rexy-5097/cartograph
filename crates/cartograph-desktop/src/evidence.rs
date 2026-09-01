//! Looking up the evidence behind one edge.
//!
//! # Why this is a lookup and not part of the scene
//!
//! [ADR-0017](../../../docs/adr/ADR-0017-render-scene.md) deliberately kept
//! confidence, provenance, evidence and location out of the render payload:
//! ten thousand nodes should not pay transport for data nothing draws. So the
//! evidence record is fetched for the one edge a person actually clicked, and
//! this module is that fetch.
//!
//! Everything returned is read straight off the [`Edge`] the Rust analysis
//! produced. Nothing here computes, estimates, rounds or reformats a claim.
//!
//! # The stale-selection problem, and why a generation token exists
//!
//! `EdgeId` is **graph-local and restarts at zero for every analysis**
//! (ADR-0011). That makes a stale selection genuinely dangerous rather than
//! merely untidy: if the window still holds edge `42` from the previous
//! repository and asks for its evidence after a new analysis has replaced the
//! graph, edge `42` almost certainly *exists* in the new graph — as a
//! completely different relationship. The lookup would succeed and the panel
//! would show confident, well-formed, wrong evidence.
//!
//! Nothing about the id itself can detect that. So every analysis is stamped
//! with an [`AnalysisId`], the payload carries it, and a lookup must present
//! the id it was looking at. A mismatch is refused as
//! [`DesktopErrorKind::StaleSelection`] rather than answered.
//!
//! This is defence at the boundary, not in the interface. The frontend also
//! clears its selection when the graph changes — but a correctness guarantee
//! that depends on the client remembering to do something is not a guarantee.

use cartograph_core::{Edge, EdgeId, EdgeKind, Node, NodeId, NodeKind};
use cartograph_graph::ArchitectureGraph;
use serde::{Deserialize, Serialize};

use crate::error::{DesktopError, DesktopErrorKind};

/// Identifies one analysis, so a selection cannot outlive its graph.
///
/// Monotonic within a process. It is not persisted and means nothing across
/// runs — it exists only to make "is this selection still about the graph you
/// are looking at?" answerable.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct AnalysisId(u64);

impl AnalysisId {
    /// The first id.
    #[must_use]
    pub const fn first() -> Self {
        Self(1)
    }

    /// The next id in sequence.
    #[must_use]
    pub const fn next(self) -> Self {
        Self(self.0 + 1)
    }

    /// The raw value.
    #[must_use]
    pub const fn as_u64(self) -> u64 {
        self.0
    }
}

/// One endpoint of a relationship, named well enough to show.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Endpoint {
    /// Graph-local handle, so the interface can highlight the node.
    pub id: NodeId,
    /// The artefact's name.
    pub label: String,
    /// What the artefact is.
    pub kind: NodeKind,
}

/// Where a claim was observed.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Location {
    /// **Repository-relative**, never absolute. Enforced at construction by
    /// `SourceLocation`, which rejects absolute paths outright.
    pub file: String,
    /// One-based.
    pub line: u32,
    /// One-based, when the analysis recorded one.
    pub column: Option<u32>,
}

/// Everything Cartograph knows about one edge.
///
/// Every field is read from the edge. There is no field here that the analysis
/// did not produce, and none that this module derives.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EvidenceRecord {
    /// The analysis this belongs to; echoed back so the caller can check.
    pub analysis: AnalysisId,
    /// The edge itself.
    pub edge: EdgeId,
    /// What kind of relationship.
    pub kind: EdgeKind,
    /// Where it starts.
    pub source: Endpoint,
    /// Where it points.
    pub target: Endpoint,
    /// The raw confidence value.
    ///
    /// **This is an uncalibrated prior selected by evidence class, not a
    /// probability.** M08 measured it: ECE 0.18, and the low bands have no
    /// verified observations at all. The interface must not present it as a
    /// likelihood, and `calibrated` below exists so that requirement travels
    /// with the data instead of relying on whoever writes the next client.
    pub confidence: f32,
    /// Always `false` today. Carried so the frontend states the caveat from
    /// the data rather than hard-coding an assumption that may change.
    pub calibrated: bool,
    /// Which analysis produced the edge.
    pub provenance: cartograph_core::Provenance,
    /// Whether that analysis computes rather than estimates.
    ///
    /// The product's central distinction: a derived edge was resolved, an
    /// inferred one was guessed at with a prior. Read from the edge, not
    /// decided here.
    pub deterministic: bool,
    /// The observation supporting the claim.
    pub evidence: String,
    /// Where the claim was observed.
    pub location: Location,
}

/// An analysed repository, retained so its evidence can be queried.
///
/// Holds the graph because the evidence lives on it. This is the only state
/// the desktop keeps between commands.
#[derive(Debug)]
pub struct AnalysisSession {
    id: AnalysisId,
    graph: ArchitectureGraph,
}

impl AnalysisSession {
    /// Retains a graph under an id.
    #[must_use]
    pub const fn new(id: AnalysisId, graph: ArchitectureGraph) -> Self {
        Self { id, graph }
    }

    /// Which analysis this is.
    #[must_use]
    pub const fn id(&self) -> AnalysisId {
        self.id
    }

    /// The graph, for callers that need to read it.
    #[must_use]
    pub const fn graph(&self) -> &ArchitectureGraph {
        &self.graph
    }

    /// The evidence behind one edge.
    ///
    /// # Errors
    ///
    /// - [`DesktopErrorKind::StaleSelection`] if `analysis` is not this
    ///   session's id. See the module documentation: this is the case that
    ///   would otherwise return confident, wrong evidence.
    /// - [`DesktopErrorKind::UnknownEdge`] if the edge is not in this graph.
    /// - [`DesktopErrorKind::Internal`] if an endpoint is missing, which would
    ///   mean the graph is corrupt.
    pub fn evidence(
        &self,
        analysis: AnalysisId,
        edge: EdgeId,
    ) -> Result<EvidenceRecord, DesktopError> {
        if analysis != self.id {
            return Err(DesktopError::new(
                DesktopErrorKind::StaleSelection,
                "That selection belongs to an earlier analysis.",
            )
            .with_hint("Select the relationship again on the current map."));
        }

        let found = self.graph.edge(edge).ok_or_else(|| {
            DesktopError::new(
                DesktopErrorKind::UnknownEdge,
                "That relationship is not part of this analysis.",
            )
            .with_hint("Select the relationship again on the current map.")
        })?;

        record(self.id, &self.graph, found)
    }
}

/// Reads an edge into a record, resolving its endpoints.
fn record(
    analysis: AnalysisId,
    graph: &ArchitectureGraph,
    edge: &Edge,
) -> Result<EvidenceRecord, DesktopError> {
    let source = endpoint(graph, edge.source())?;
    let target = endpoint(graph, edge.target())?;
    let location = edge.location();

    Ok(EvidenceRecord {
        analysis,
        edge: edge.id(),
        kind: edge.kind(),
        source,
        target,
        confidence: edge.confidence().get(),
        // Nothing in Cartograph is calibrated yet; M08 measured that and it has
        // not changed. Stated as data so the interface cannot forget it.
        calibrated: false,
        provenance: edge.provenance(),
        deterministic: edge.is_deterministic(),
        evidence: edge.evidence().as_str().to_owned(),
        location: Location {
            file: location.file().to_owned(),
            line: location.line(),
            column: location.column(),
        },
    })
}

/// Resolves a node handle into a showable endpoint.
fn endpoint(graph: &ArchitectureGraph, id: NodeId) -> Result<Endpoint, DesktopError> {
    let node: &Node = graph.node(id).ok_or_else(|| {
        DesktopError::new(
            DesktopErrorKind::Internal,
            "A relationship pointed at an artefact that is not in the graph.",
        )
    })?;
    Ok(Endpoint {
        id,
        label: node.name().to_owned(),
        kind: node.kind(),
    })
}
