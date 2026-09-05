//! What a provider claims, and what survives being checked.
//!
//! # Two shapes, deliberately
//!
//! [`ClaimedAnswer`] is what a provider produced. Its fields are public and it
//! carries no guarantee at all — that is the point: it is a claim, and naming
//! it that way makes every place one is handled visibly a place where nothing
//! has been proved yet.
//!
//! [`Answer`] is what survived [`validate`]. Its fields are private and it has
//! no public constructor, so the only way to obtain one is to pass the citation
//! contract. "Render an unvalidated answer" is not an expressible program.
//!
//! # Nothing is rewritten on the way through
//!
//! Validation reads; it never repairs. Citation text is not trimmed, its
//! whitespace is not normalised, its punctuation is not adjusted and its bytes
//! are not re-encoded. A quotation that is nearly verbatim is refused, because
//! "nearly verbatim" is a paraphrase and a reader cannot check a paraphrase
//! against the evidence. `Evidence` itself is never touched: a bundle is
//! borrowed, and nothing here can mutate one.
//!
//! # One bad citation rejects the whole answer
//!
//! Not the item — the answer. A partially valid answer is the most dangerous
//! output this feature can produce, because it looks checked. The caller falls
//! back to the raw-evidence path, which is exactly where it would have been
//! with the model switched off.

use cartograph_core::EdgeId;
use cartograph_graph::bundle::{BundleScope, Citation, EvidenceBundle};

use crate::error::ProviderError;

/// One quotation a provider claimed, before anything is checked.
///
/// It carries no scope, because a provider is never told one: the scope is the
/// bundle's, and it is applied here by checking the claim against that bundle.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClaimedCitation {
    /// The edge the provider says it is quoting.
    pub edge: EdgeId,
    /// The text the provider says appears in that edge's evidence.
    pub text: String,
}

/// One explanation a provider claimed, before anything is checked.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClaimedItem {
    /// The sentence the provider produced.
    pub text: String,
    /// The evidence it says supports that sentence.
    pub citations: Vec<ClaimedCitation>,
}

/// A whole answer a provider claimed, before anything is checked.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClaimedAnswer {
    /// The explanation, in the order the provider gave it.
    pub items: Vec<ClaimedItem>,
}

/// One explanation, with the citations that were proved for it.
///
/// Private fields: an `ExplanationItem` exists only inside a validated
/// [`Answer`], and there is no way to assemble one that skipped the check.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExplanationItem {
    text: String,
    citations: Vec<Citation>,
}

impl ExplanationItem {
    /// The explanation, byte for byte as the provider produced it.
    #[must_use]
    pub fn text(&self) -> &str {
        &self.text
    }

    /// The evidence behind it. Never empty — an item without one is refused.
    #[must_use]
    pub fn citations(&self) -> &[Citation] {
        &self.citations
    }
}

/// An answer whose every claim cites evidence that was actually sent.
///
/// There is no public constructor. [`validate`] is the only way to obtain one,
/// which is what makes holding an `Answer` mean something.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Answer {
    scope: BundleScope,
    items: Vec<ExplanationItem>,
}

impl Answer {
    /// The bundle this answer was proved against.
    ///
    /// Carried so a later re-check can refuse an answer that outlived its
    /// analysis rather than resolve its citations against a different graph.
    #[must_use]
    pub const fn scope(&self) -> BundleScope {
        self.scope
    }

    /// The explanation, in the order the provider gave it.
    #[must_use]
    pub fn items(&self) -> &[ExplanationItem] {
        &self.items
    }

    /// How many explanations the answer carries. Never zero.
    #[must_use]
    pub fn len(&self) -> usize {
        self.items.len()
    }

    /// Always `false`: an answer with no items is refused by [`validate`].
    ///
    /// Present because a `len` without an `is_empty` is a trap for a reader,
    /// and answering it honestly is better than omitting it.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }
}

/// Checks a provider's claim against the evidence that was sent to it.
///
/// The four citation properties are decided by
/// [`EvidenceBundle::cite`] — the machinery M16 Slice 1 built — rather than
/// restated here, so there is one definition of "verbatim" in the workspace.
/// What this function adds is the two properties that are about the *answer*
/// rather than about one citation: every item says something, and every item
/// cites something.
///
/// # Errors
///
/// - [`ProviderError::NoItems`] if the provider returned nothing to check.
/// - [`ProviderError::EmptyItem`] for an item with no text.
/// - [`ProviderError::UncitedItem`] for an item with no citations.
/// - [`ProviderError::InvalidCitation`] for a citation that names an edge the
///   bundle does not carry, quotes nothing, or quotes text that is not a
///   byte-for-byte substring of that edge's evidence.
///
/// The first failure refuses the whole answer. Nothing partial is returned.
pub fn validate(bundle: &EvidenceBundle, claimed: ClaimedAnswer) -> Result<Answer, ProviderError> {
    if claimed.items.is_empty() {
        return Err(ProviderError::NoItems);
    }

    let mut items = Vec::with_capacity(claimed.items.len());

    for (index, item) in claimed.items.into_iter().enumerate() {
        // Reads the text to decide whether it says anything; does not rewrite
        // it. The stored text below is the original, untouched.
        if item.text.trim().is_empty() {
            return Err(ProviderError::EmptyItem { item: index });
        }
        if item.citations.is_empty() {
            return Err(ProviderError::UncitedItem { item: index });
        }

        let mut citations = Vec::with_capacity(item.citations.len());
        for (position, claim) in item.citations.iter().enumerate() {
            let citation = bundle.cite(claim.edge, &claim.text).map_err(|cause| {
                ProviderError::InvalidCitation {
                    item: index,
                    citation: position,
                    cause,
                }
            })?;
            citations.push(citation);
        }

        items.push(ExplanationItem {
            text: item.text,
            citations,
        });
    }

    Ok(Answer {
        scope: bundle.scope(),
        items,
    })
}

/// Re-checks an already-validated answer against a bundle.
///
/// Validation proves an answer against the bundle it was produced from. This
/// proves it again at the moment it is used, which is a different question:
/// `EdgeId` is graph-local and restarts at zero for every analysis (ADR-0011),
/// so an answer that outlived its analysis names edges that almost certainly
/// still resolve — as completely different relationships.
///
/// `cartograph_desktop::evidence` already treats that as a correctness hazard
/// rather than untidiness, and refuses. So does this.
///
/// # Errors
///
/// [`ProviderError::ScopeMismatch`] if the answer belongs to another bundle,
/// and [`ProviderError::InvalidCitation`] if a citation no longer holds.
pub fn revalidate(bundle: &EvidenceBundle, answer: &Answer) -> Result<(), ProviderError> {
    if answer.scope != bundle.scope() {
        return Err(ProviderError::ScopeMismatch);
    }

    for (index, item) in answer.items.iter().enumerate() {
        for (position, citation) in item.citations.iter().enumerate() {
            bundle
                .verify(citation)
                .map_err(|cause| ProviderError::InvalidCitation {
                    item: index,
                    citation: position,
                    cause,
                })?;
        }
    }

    Ok(())
}
