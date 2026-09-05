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
//! job. This crate does not, and the reason is the workspace's own dependency
//! policy: every entry must answer *"what concrete Cartograph problem does this
//! solve right now?"*, and `thiserror`'s honest answer here is "it saves twenty
//! lines". `serde` earns its place because the wire format cannot be built
//! without it; a derive macro for two `impl` blocks does not.
//! `cartograph_desktop::credential::CredentialError` is hand-written for the
//! same reason and reads the same way.
//!
//! [`EdgeId`]: cartograph_core::EdgeId

use std::fmt;

use cartograph_graph::bundle::CitationError;

use crate::transport::TransportError;

/// What was wrong with a reply, at the coarsest useful grain.
///
/// Deliberately coarse. The obvious richer design wraps the `serde_json` error,
/// and that error quotes the input it choked on — *"invalid type: integer `3`,
/// expected a string at line 1 column 42"*. A response body must never reach a
/// log (ADR-0021 Amendment 3, decision C8), and the surest way to keep one out
/// is to never hold it. The cost is a coarser diagnostic; that is the trade.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum ResponseFault {
    /// The body was not JSON, or not the chat-completions envelope.
    NotJson,
    /// The envelope carried no choice to read.
    NoChoices,
    /// The document was not the schema that was asked for: a missing field, a
    /// field of the wrong type, or a field the schema does not define.
    NotTheSchema,
}

impl fmt::Display for ResponseFault {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::NotJson => "it was not the expected JSON document",
            Self::NoChoices => "it carried no completion",
            Self::NotTheSchema => "it did not match the requested schema",
        })
    }
}

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
    /// The request could not be serialised.
    ///
    /// Not reachable in practice — every field of the wire request is a
    /// string, a number or a fieldless enum — and returned rather than
    /// panicked because a library that panics on a serialiser bug takes its
    /// caller down with it.
    MalformedRequest,
    /// The provider replied with something that could not be read.
    ///
    /// Carries only the classification. Never the body.
    InvalidResponse {
        /// Which of the three ways it failed to be a reply.
        fault: ResponseFault,
    },
    /// The provider could not be reached, or failed, with no more to say.
    ///
    /// For an implementation that has no transport to report on — a mock, or
    /// a future local model. An HTTP provider reports
    /// [`Transport`](Self::Transport) instead, which says which way it failed.
    Unavailable,
    /// The exchange itself failed: no reply, or none that could be read.
    ///
    /// Carries [`TransportError`], which is a category and at most a status
    /// code. It has no field that could hold a body, a header or a URL, so
    /// this variant inherits that guarantee rather than restating it.
    Transport {
        /// How the exchange failed.
        cause: TransportError,
    },
    /// The provider answered, and refused.
    ///
    /// Carries the HTTP status, and the provider's own classification when it
    /// supplied a plausible one — never its message, and never its body.
    Refused {
        /// The HTTP status code.
        status: u16,
        /// The provider's classification, e.g. `invalid_request_error`.
        ///
        /// `None` when the body was not an error document, or when what it
        /// offered did not look like a classification. See
        /// `crate::groq::ApiError::kind` for why that check exists.
        kind: Option<String>,
    },
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
            Self::MalformedRequest => f.write_str("the request could not be prepared"),
            Self::InvalidResponse { fault } => {
                write!(f, "the provider's reply could not be read: {fault}")
            }
            Self::Unavailable => f.write_str("the provider could not be reached"),
            Self::Transport { cause } => write!(f, "{cause}"),
            Self::Refused { status, kind } => match kind {
                Some(kind) => write!(f, "the provider refused the request ({status}, {kind})"),
                None => write!(f, "the provider refused the request ({status})"),
            },
        }
    }
}

impl std::error::Error for ProviderError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::InvalidCitation { cause, .. } => Some(cause),
            Self::Transport { cause } => Some(cause),
            _ => None,
        }
    }
}
