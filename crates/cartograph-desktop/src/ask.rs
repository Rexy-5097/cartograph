//! ASK with the model switched off.
//!
//! # What this is
//!
//! M16's acceptance sentence has two halves, and this module is the second
//! one: *"with AI disabled the feature degrades to showing the raw evidence"*.
//! It answers "explain this artefact" using nothing but analysis Cartograph
//! has already done — no model, no key, no network, no stored preference.
//!
//! That is not a placeholder for the interesting version. RULE 013 says the
//! product is fully useful with zero API keys and no network, and this is the
//! path that keeps that true. It is built first so the guarantee exists before
//! anything can weaken it.
//!
//! # Raw evidence means the derived claim, not the source
//!
//! "Raw evidence" is `Edge::evidence()` — the sentence the resolver wrote
//! about why it believes a relationship exists. It is not the file, not a
//! fragment of the file, and not the graph. `cartograph_core::Evidence`
//! states the distinction directly: evidence *describes* a claim, it does not
//! quote the file.
//!
//! # Why this reuses `EvidenceRecord` rather than defining its own shape
//!
//! The panel already knows how to show what Cartograph believes about one
//! relationship, and [`EvidenceRecord`] is that contract. Giving ASK a second,
//! nearly-identical record would create two descriptions of the same claim
//! that could drift apart, and the one place a reader would notice is the one
//! place it matters. So an answer is a list of exactly the records the panel
//! already renders, produced by exactly the lookup that already produces them.
//!
//! The bundle's job here is the part `EvidenceRecord` cannot do: decide *which*
//! edges belong to the answer, order them deterministically, refuse evidence
//! that must not leave the machine, and stamp a scope a later citation can be
//! checked against.
//!
//! # Staleness
//!
//! Identical to [`crate::evidence`] and [`crate::blast`], for the identical
//! reason: `NodeId` and `EdgeId` are graph-local and restart at zero for every
//! analysis (ADR-0011), so a selection held across a re-analysis names a
//! handle that almost certainly still exists — as something else. Refusing is
//! the only safe answer.

use cartograph_core::NodeId;
use cartograph_graph::bundle::EvidenceBundle;
use cartograph_graph::trace::{DEFAULT_MAX_DEPTH, trace};
use serde::{Deserialize, Serialize};

use crate::error::{DesktopError, DesktopErrorKind};
use crate::evidence::{AnalysisId, AnalysisSession, EvidenceRecord};

/// Whether a model contributed to an answer.
///
/// Carried in the payload rather than assumed by the interface. The panel must
/// be able to say "this is the evidence, unexplained" from the data, so that
/// when a provider arrives the same surface follows the answer instead of
/// needing to be remembered — the treatment `calibrated` and `routes` already
/// get elsewhere in this crate.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
#[non_exhaustive]
pub enum AiState {
    /// No model was consulted. The answer is the derived evidence alone.
    Disabled,
}

/// An answer about one artefact, assembled from evidence alone.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AskAnswer {
    /// The analysis this belongs to, echoed back so the window can discard a
    /// response that arrived after it moved on.
    pub analysis: AnalysisId,
    /// The artefact asked about.
    pub target: NodeId,
    /// Whether a model was involved. Always [`AiState::Disabled`] today.
    pub ai: AiState,
    /// The bundle these entries came from.
    ///
    /// Opaque, and compared rather than interpreted. It exists so a later
    /// slice can prove a citation belongs to the evidence that was actually
    /// shown, instead of to an edge id that happens to resolve.
    pub scope: u64,
    /// The evidence, in the bundle's deterministic order.
    ///
    /// Empty is an answer, not a failure: an artefact with no outgoing
    /// relationships has nothing derived to explain, and saying so is honest.
    pub entries: Vec<EvidenceRecord>,
}

impl AskAnswer {
    /// How many relationships the answer rests on.
    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether the answer rests on no evidence at all.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

/// Explains one artefact using only already-derived evidence.
///
/// Walks outward from the artefact, bundles the evidence behind every hop, and
/// returns it unchanged. Nothing here computes, summarises, rounds or
/// rephrases a claim, and no model is consulted.
///
/// # Errors
///
/// - [`DesktopErrorKind::StaleSelection`] if `analysis` is not this session's
///   id — the case that would otherwise explain a different artefact
///   confidently and wrongly.
/// - [`DesktopErrorKind::UnknownNode`] if the artefact is not in this graph.
///   Distinct from an empty answer: "nothing to explain" is a result, "this is
///   not here" is the window and the graph disagreeing.
/// - [`DesktopErrorKind::Internal`] if the evidence could not be bundled,
///   which means the graph carries something that must not leave the machine.
pub fn answer(
    session: &AnalysisSession,
    analysis: AnalysisId,
    node: NodeId,
) -> Result<AskAnswer, DesktopError> {
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

    let walk = trace(graph, node, DEFAULT_MAX_DEPTH);
    let bundle = EvidenceBundle::from_trace(graph, &walk).map_err(|_| {
        // The refusal is deliberately not repeated. `BundleError` names the
        // edge and the category and never the text, and an error crossing to
        // the window is a log line like any other (RULE 015).
        DesktopError::internal("This relationship's evidence could not be shown safely.")
    })?;

    // Materialised through the lookup the panel already uses, in the bundle's
    // order. The bundle decides *which* and *in what order*; `evidence` decides
    // *how a claim is described*, once, for every surface.
    let entries = bundle
        .edges()
        .iter()
        .map(|bundled| session.evidence(analysis, bundled.edge))
        .collect::<Result<Vec<EvidenceRecord>, DesktopError>>()?;

    Ok(AskAnswer {
        analysis,
        target: node,
        ai: AiState::Disabled,
        scope: bundle.scope().as_u64(),
        entries,
    })
}
