//! The provider-independent interface, and the exact list of what crosses it.
//!
//! # The request is these three things
//!
//! A provider is given a [`Question`], an [`EvidenceBundle`] and a borrowed
//! [`Secret`]. There is no fourth parameter and no ambient state, so the
//! request model is not a document to be kept in sync with the code — it is the
//! signature.
//!
//! | Crosses the boundary | Cannot cross it, and why |
//! |---|---|
//! | the user's question | source files — a bundle carries `Evidence`, which may not quote a file |
//! | already-derived evidence | the `ArchitectureGraph` — the trait takes a bundle, and a bundle is a copy |
//! | the credential, borrowed for the call | `RepositoryIdentity` — not nameable in this crate |
//! | | absolute paths — `SourceLocation` refuses them at construction |
//! | | environment values — `NodeKind::EnvVar` has no field for one |
//!
//! Repository-**relative** paths do cross, and that is written down rather than
//! glossed: `NodeIdentity` is `kind|name|file`, and evidence text frequently
//! names a file. Evidence must travel byte for byte or a verbatim citation
//! cannot be checked against it. ADR-0021 Amendment 3, decision C4, states the
//! rule at the precision the code can hold: no absolute path and nothing
//! identifying this machine ever leaves it.
//!
//! # Why blocking
//!
//! The desktop and the whole synchronous core path use no async Rust. Tokio
//! exists in exactly one crate, `cartograph-mcp`, because `rmcp` requires it.
//! ASK is a person pressing a button and waiting for a sentence; there is no
//! concurrency to win, and an async interface here would pull a runtime into
//! the synchronous path to serve one request per gesture.
//!
//! # Why the credential is borrowed, and not stored
//!
//! An implementation is constructed without a credential and receives one per
//! call, for the duration of that call. It has nowhere to accumulate keys, and
//! a [`Secret`] cannot become text by accident: no `Display`, no `Serialize`,
//! and a `Debug` that prints a placeholder.
//!
//! # What an implementation must not do
//!
//! Return an [`Answer`] it built itself. It cannot — `Answer` has no public
//! constructor, so the only way to produce one is
//! [`validate`](crate::validate) against the bundle it was given.
//!
//! [`Secret`]: cartograph_core::Secret
//! [`EvidenceBundle`]: cartograph_graph::bundle::EvidenceBundle

use cartograph_core::Secret;
use cartograph_graph::bundle::EvidenceBundle;

use crate::answer::Answer;
use crate::error::ProviderError;
use crate::question::Question;

/// Something that can explain derived evidence, and cite it.
///
/// One implementation ships per provider. The trait exists so that swapping
/// providers is a new implementation of one method rather than a change to
/// anything that depends on ASK.
pub trait Provider {
    /// Explains the bundle in answer to the question.
    ///
    /// The returned answer has already passed the citation contract, because
    /// [`Answer`] cannot be constructed any other way.
    ///
    /// # Errors
    ///
    /// [`ProviderError`] — the provider could not be reached, or what it
    /// produced did not hold against the evidence it was sent.
    fn explain(
        &self,
        bundle: &EvidenceBundle,
        question: &Question,
        credential: &Secret,
    ) -> Result<Answer, ProviderError>;
}
