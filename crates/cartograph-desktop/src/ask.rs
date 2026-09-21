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

use cartograph_ask::{Answer, GroqProvider, Provider, Question, UreqTransport};
use cartograph_core::NodeId;
use cartograph_graph::bundle::EvidenceBundle;
use cartograph_graph::trace::{DEFAULT_MAX_DEPTH, trace};
use serde::{Deserialize, Serialize};

use crate::credential::{CredentialStore, NoCredentials};
use crate::error::{DesktopError, DesktopErrorKind};
use crate::evidence::{AnalysisId, AnalysisSession, EvidenceRecord};
use crate::keychain::KeychainStore;
use crate::optin::GrantedRepository;

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
    /// No model was consulted, because ASK is not switched on for this
    /// repository. The answer is the derived evidence alone.
    Disabled,
    /// ASK is on, but there is no usable credential — none stored, a locked
    /// keychain, or no keychain at all. **No request was made.**
    Unavailable,
    /// A provider was asked and did not produce a usable answer: unreachable,
    /// refused, or an answer whose citations did not hold against the evidence
    /// it was sent. **Nothing unvalidated is shown.**
    Failed,
    /// A provider answered and every claim cited evidence that was sent.
    Answered,
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
    ///
    /// **Always present**, whatever [`AiState`] says. The degraded answer is
    /// not a fallback that replaces an explanation; it is the floor an
    /// explanation sits on top of, so a reader can always see what the model
    /// was given.
    pub entries: Vec<EvidenceRecord>,
    /// The model's explanation, empty unless [`AiState::Answered`].
    ///
    /// Only ever a projection of a validated [`Answer`]. There is no path by
    /// which an unvalidated claim reaches this field, because the only way to
    /// obtain an `Answer` is `cartograph_ask::validate`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub explanation: Vec<ExplanationRecord>,
}

/// One explanation and the evidence it quoted.
///
/// A presentation projection of `cartograph_ask::ExplanationItem`, which is
/// deliberately not serialisable: what crosses to the window is chosen here,
/// once, rather than by deriving `Serialize` on a validated type and letting
/// every future field cross with it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExplanationRecord {
    /// The explanation, byte for byte as the provider produced it.
    pub text: String,
    /// The evidence behind it. Never empty — an item without one is refused
    /// before it can reach here.
    pub citations: Vec<CitationRecord>,
}

/// One quoted piece of evidence.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CitationRecord {
    /// The edge whose evidence is quoted, as the panel already addresses one.
    pub edge: u64,
    /// The quotation, byte for byte from that edge's evidence.
    pub text: String,
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

    let (bundle, entries) = gather(session, analysis, node)?;

    Ok(AskAnswer {
        analysis,
        target: node,
        ai: AiState::Disabled,
        scope: bundle.scope().as_u64(),
        entries,
        explanation: Vec::new(),
    })
}

/// The evidence behind one artefact: the bundle, and the records for it.
///
/// Both halves come from one walk, so the records the reader sees and the
/// bundle a provider is sent can never describe different sets of edges --
/// which is what makes a citation checkable against what is on screen.
fn gather(
    session: &AnalysisSession,
    analysis: AnalysisId,
    node: NodeId,
) -> Result<(EvidenceBundle, Vec<EvidenceRecord>), DesktopError> {
    let graph = session.graph();
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

    Ok((bundle, entries))
}

/// Explains one artefact, consulting a model only when every gate allows it.
///
/// # The order of the gates is the security property
///
/// 1. **Selection.** A stale analysis or an unknown artefact is refused before
///    anything else happens, exactly as [`answer`] refuses it.
/// 2. **Opt-in.** No grant, or ASK switched off, ends here with
///    [`AiState::Disabled`]. **The credential is not read** -- a store that is
///    never consulted cannot prompt, fail or leak.
/// 3. **Credential.** Absent, refused or unavailable ends here with
///    [`AiState::Unavailable`], and **no request is made**.
/// 4. **Provider.** Only now, and only for this one call, is the credential
///    borrowed.
///
/// Every exit above returns the derived evidence, so RULE 013 -- *"fully
/// useful with zero API keys and no network"* -- holds on every path rather
/// than on the one nobody took.
///
/// # What a failure does not do
///
/// A provider that cannot be reached, refuses, or produces an answer whose
/// citations do not hold degrades to [`AiState::Failed`] with the evidence
/// intact. It never surfaces a partially valid answer: `cartograph_ask`
/// rejects the whole answer if any citation fails, and an answer that looks
/// checked but is not is the most dangerous thing this feature could show.
///
/// # Errors
///
/// The same three as [`answer`], plus
/// [`DesktopErrorKind::InvalidQuestion`] when the question is empty or longer
/// than the provider boundary accepts. A bad question is the caller's mistake
/// and is reported as one rather than degraded: quietly returning evidence for
/// a question that was never sent would call something an answer that is not.
pub fn explain<P: Provider, C: CredentialStore>(
    session: &AnalysisSession,
    analysis: AnalysisId,
    node: NodeId,
    question: &str,
    grant: Option<&GrantedRepository>,
    credentials: &C,
    provider: &P,
) -> Result<AskAnswer, DesktopError> {
    if analysis != session.id() {
        return Err(DesktopError::new(
            DesktopErrorKind::StaleSelection,
            "That selection belongs to an earlier analysis.",
        )
        .with_hint("Select the artefact again on the current map."));
    }
    if session.graph().node(node).is_none() {
        return Err(DesktopError::new(
            DesktopErrorKind::UnknownNode,
            "That artefact is not part of this analysis.",
        )
        .with_hint("Select the artefact again on the current map."));
    }

    let question = Question::new(question).map_err(|_| {
        DesktopError::new(
            DesktopErrorKind::InvalidQuestion,
            "That question cannot be sent as it is.",
        )
        .with_hint("Ask something shorter than two thousand characters.")
    })?;

    let (bundle, entries) = gather(session, analysis, node)?;
    let degraded = |ai: AiState| AskAnswer {
        analysis,
        target: node,
        ai,
        scope: bundle.scope().as_u64(),
        entries: entries.clone(),
        explanation: Vec::new(),
    };

    // Gate 2. An enabled grant is the only thing that permits gate 3.
    let Some(identity) = grant
        .filter(|granted| granted.ask_enabled())
        .map(GrantedRepository::identity)
    else {
        return Ok(degraded(AiState::Disabled));
    };

    // Gate 3. Absence and failure land on one outcome *here* on purpose: the
    // panel shows the evidence either way, and the distinction the store keeps
    // is for the surface that offers to fix it, not for this decision.
    let Ok(Some(credential)) = credentials.get(identity) else {
        return Ok(degraded(AiState::Unavailable));
    };

    // Gate 4. The credential is borrowed for this call and nothing else. It is
    // not stored on the provider, on the answer, or on anything returned here.
    match provider.explain(&bundle, &question, &credential) {
        Ok(validated) => Ok(AskAnswer {
            ai: AiState::Answered,
            explanation: project(&validated),
            ..degraded(AiState::Answered)
        }),
        // `ProviderError` is dropped rather than reported. Its variants carry
        // status codes and fixed strings, but a provider failure crossing to
        // the window is a message from a third party about a request the
        // window never made, and it is not needed to show what is already here.
        Err(_) => Ok(degraded(AiState::Failed)),
    }
}

/// [`explain`] over the stack this product actually ships.
///
/// The OS keychain (ADR-0021 Amendment 2) and Groq over `ureq` (Amendment 3),
/// assembled here rather than in the Tauri shell. ADR-0016 puts everything
/// that can be wrong on this side of the workspace boundary, where the gates
/// run -- *"if a command there grows a branch, that branch is in the wrong
/// file"* -- and choosing a credential store is a branch.
///
/// A machine whose keychain cannot be opened falls back to [`NoCredentials`],
/// which reads as absent, so ASK degrades for the same reason and by the same
/// path as a repository with no key stored. That is the behaviour §13 requires
/// and it is reached without a special case.
///
/// # Errors
///
/// Exactly [`explain`]'s.
pub fn explain_with_os_keychain(
    session: &AnalysisSession,
    analysis: AnalysisId,
    node: NodeId,
    question: &str,
    grant: Option<&GrantedRepository>,
) -> Result<AskAnswer, DesktopError> {
    // Constructed per call and holding nothing: the transport keeps a
    // connection agent, the provider keeps the transport, and neither ever
    // holds a credential.
    let provider = GroqProvider::new(UreqTransport::new());

    match KeychainStore::open() {
        Ok(keychain) => explain(
            session, analysis, node, question, grant, &keychain, &provider,
        ),
        Err(_) => explain(
            session,
            analysis,
            node,
            question,
            grant,
            &NoCredentials,
            &provider,
        ),
    }
}

/// Turns a validated answer into what the window may see.
fn project(answer: &Answer) -> Vec<ExplanationRecord> {
    answer
        .items()
        .iter()
        .map(|item| ExplanationRecord {
            text: item.text().to_owned(),
            citations: item
                .citations()
                .iter()
                .map(|citation| CitationRecord {
                    edge: citation.edge().as_u64(),
                    text: citation.text().to_owned(),
                })
                .collect(),
        })
        .collect()
}
