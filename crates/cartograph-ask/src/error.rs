//! Why an answer was refused, or a provider could not answer.
//!
//! # What these errors may carry
//!
//! Positions and reasons. **Never** a credential, a request body, a response
//! body, an absolute path, or the text a provider produced. An error message is
//! a log line, and RULE 015 applies to it exactly as it applies to `tracing`.
//!
//! That is structural rather than a convention here: no variant has a field
//! that could hold one. The most a variant carries is an index, and — through
//! [`CitationError`] — an [`EdgeId`], which is a graph-local integer.
//!
//! # Why this is hand-written rather than derived
//!
//! `cartograph-core` and `cartograph-graph` use `thiserror` for exactly this
//! job, and this crate would too if it had any dependency at all. It has none:
//! at this step `cartograph-ask` depends on nothing outside the workspace, so
//! that the boundary it exists to establish can be reviewed without also
//! reviewing what came with it. `cartograph_desktop::credential::CredentialError`
//! is hand-written for the same reason and reads the same way.
//!
//! [`EdgeId`]: cartograph_core::EdgeId

use std::fmt;

use cartograph_graph::bundle::CitationError;

/// Why an answer was refused.
///
/// Every variant that names a position names it by index, so a reader of a log
/// can find the offending item without the log carrying its text.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum ProviderError {
    /// The provider returned nothing to check.
    ///
    /// An answer with no items is not an empty answer, it is a failure to
    /// answer: a bundle with no evidence must degrade to the raw-evidence path
    /// rather than reach a provider at all.
    NoItems,
    /// An explanation item said nothing.
    EmptyItem {
        /// Which item, counting from zero.
        item: usize,
    },
    /// An explanation item cited nothing.
    ///
    /// This is the failure RULE 007 exists to prevent: an uncited sentence is
    /// interpretation wearing the appearance of a derived fact.
    UncitedItem {
        /// Which item, counting from zero.
        item: usize,
    },
    /// A citation did not hold against the bundle it was checked against.
    ///
    /// The cause distinguishes a foreign bundle, an edge the bundle does not
    /// carry, an empty quotation and a quotation that is not verbatim.
    InvalidCitation {
        /// Which item, counting from zero.
        item: usize,
        /// Which citation within that item, counting from zero.
        citation: usize,
        /// What was wrong with it.
        cause: CitationError,
    },
    /// The answer was proved against a different bundle than the one it is
    /// now being checked against.
    ///
    /// The staleness defence ADR-0011 requires: an `EdgeId` restarts at zero
    /// for every analysis, so a citation that outlives its bundle almost
    /// certainly resolves — as a completely different relationship.
    ScopeMismatch,
    /// The provider could not be reached, or failed.
    ///
    /// Deliberately shapeless at this step. There is no transport yet, and an
    /// error type that anticipated one would be guessing at its failure modes;
    /// this enum is `#[non_exhaustive]` so a later step can distinguish them
    /// without breaking a caller. What will never be added is a field carrying
    /// a body, a header or a URL.
    Unavailable,
}

impl fmt::Display for ProviderError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoItems => f.write_str("the provider returned no explanation"),
            Self::EmptyItem { item } => write!(f, "explanation item {item} says nothing"),
            Self::UncitedItem { item } => {
                write!(f, "explanation item {item} cites no evidence")
            }
            Self::InvalidCitation {
                item,
                citation,
                cause,
            } => write!(
                f,
                "citation {citation} of explanation item {item} was refused: {cause}"
            ),
            Self::ScopeMismatch => f.write_str("that answer belongs to a different analysis"),
            Self::Unavailable => f.write_str("the provider could not be reached"),
        }
    }
}

impl std::error::Error for ProviderError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::InvalidCitation { cause, .. } => Some(cause),
            _ => None,
        }
    }
}
