//! Blast radius for the desktop: what depends on a selected artefact.
//!
//! # This module decides nothing
//!
//! Every semantic question — which edges count as dependencies, how confidence
//! propagates, how ties break, what order results arrive in — belongs to
//! [`cartograph_graph::blast::blast_radius`], accepted in M12 Slice 1 and
//! governed by
//! [ADR-0018](../../../docs/adr/ADR-0018-blast-radius-confidence.md). This file
//! calls it once and shapes the answer for the wire.
//!
//! The CLI does exactly the same thing over the same core. The two surfaces
//! carry the same field names and the same meanings on purpose: `depth`,
//! `confidence`, `via`, `calibrated` and `routes` mean here precisely what they
//! mean in `cartograph blast --json`, so there is one contract with two
//! renderings rather than two contracts.
//!
//! # Staleness is guarded the same way evidence is
//!
//! A blast query names a node, and `NodeId` is graph-local — it restarts at
//! zero every analysis (ADR-0011). A node id held across a re-analysis almost
//! certainly still *exists* in the new graph as a different artefact, so
//! answering it would highlight a confident, wrong impact set.
//!
//! Rather than invent a second protection, this reuses the [`AnalysisId`] the
//! evidence lookup already carries: a caller must present the analysis it was
//! looking at, and a mismatch is refused as
//! [`DesktopErrorKind::StaleSelection`].

use cartograph_core::{EdgeId, NodeId};
use cartograph_graph::blast::blast_radius;
use serde::Serialize;

use crate::error::{DesktopError, DesktopErrorKind};
use crate::evidence::{AnalysisId, AnalysisSession};

/// One artefact that depends on the target.
///
/// Field names and meanings match `cartograph blast --json` exactly.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImpactedNode {
    /// The dependent artefact, addressing the scene the window is drawing.
    pub node: NodeId,
    /// Hops to the target along the reported route.
    pub depth: u32,
    /// Confidence of the best-supported route: the minimum along it, maximised
    /// over routes (ADR-0018). **Not a probability** — see `calibrated`.
    pub confidence: f32,
    /// The edge this artefact reaches the target through, on that route.
    ///
    /// An id, not a copy: the window already holds the scene's edges, and the
    /// evidence panel already knows how to fetch an edge's provenance and
    /// location. Duplicating them here would be two records of one fact.
    pub via: EdgeId,
}

/// What depends on the selected artefact.
/// Serialise-only, deliberately. This payload travels Rust to the window and
/// never back, so `routes` can stay a `&'static str` naming a constant rather
/// than an owned string that merely happens to hold the right value.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BlastPayload {
    /// The analysis this belongs to, echoed back so the window can discard a
    /// response that arrived after it moved on.
    pub analysis: AnalysisId,
    /// The artefact queried. **Never appears in `reached`** — changing a thing
    /// does not impact the thing.
    pub target: NodeId,
    /// The dependents, in the core's deterministic semantic order.
    pub reached: Vec<ImpactedNode>,
    /// Always `false`. Confidence is an uncalibrated prior (M08), and the
    /// caveat travels with the data so no surface has to remember it.
    pub calibrated: bool,
    /// Always `"representative"`: one route per artefact, the best-supported
    /// one. Others may exist and are not enumerated.
    pub routes: &'static str,
}

impl BlastPayload {
    /// How many artefacts depend on the target.
    #[must_use]
    pub fn len(&self) -> usize {
        self.reached.len()
    }

    /// Whether nothing depends on the target.
    ///
    /// A true answer, not a failure: the caller reports it as an empty result
    /// rather than an error.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.reached.is_empty()
    }
}

/// The blast radius of one artefact in a retained analysis.
///
/// # Errors
///
/// - [`DesktopErrorKind::StaleSelection`] if `analysis` is not the session's
///   id. See the module documentation: this is the case that would otherwise
///   highlight a wrong impact set.
/// - [`DesktopErrorKind::UnknownNode`] if the node is not in this graph. The
///   window only ever asks about a node it is drawing, so reaching this means
///   the two have disagreed and answering "nothing depends on it" would hide
///   that.
pub fn radius(
    session: &AnalysisSession,
    analysis: AnalysisId,
    node: NodeId,
) -> Result<BlastPayload, DesktopError> {
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

    // The one call that matters. Everything else here is shaping.
    let computed = blast_radius(graph, node);

    Ok(BlastPayload {
        analysis,
        target: computed.target,
        reached: computed
            .reached
            .iter()
            .map(|entry| ImpactedNode {
                node: entry.node,
                depth: entry.depth,
                // Copied, never recomputed or rounded.
                confidence: entry.confidence,
                via: entry.via,
            })
            .collect(),
        calibrated: false,
        routes: "representative",
    })
}
