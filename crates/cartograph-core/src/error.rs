//! Errors produced when constructing domain values.
//!
//! Every variant here represents a rejected value, not a recoverable runtime
//! fault. They exist so invalid domain state is unrepresentable rather than
//! merely discouraged.
//!
//! Error messages never embed source-file *contents*. Repository-relative
//! paths and symbol names are permitted; file text, environment variables and
//! credentials are not (`PROJECT_RULES.md` RULE 015).

use thiserror::Error;

/// Failure to construct a value in the Cartograph domain model.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum CoreError {
    /// A confidence outside the closed interval `[0.0, 1.0]`, or not finite.
    ///
    /// Confidence is a probability that an edge is correct. Values outside the
    /// unit interval cannot be calibrated and would silently corrupt the
    /// reliability diagrams published at M08.
    #[error("confidence must be a finite value in [0.0, 1.0], got {value}")]
    ConfidenceOutOfRange {
        /// The rejected value, rendered for diagnostics.
        value: String,
    },

    /// An absolute path was offered as a source location.
    ///
    /// Locations are repository-relative so that a graph is portable between
    /// machines and so that no local filesystem layout is ever committed or
    /// transmitted.
    #[error("source location must be repository-relative, got absolute path `{path}`")]
    AbsolutePath {
        /// The rejected path.
        path: String,
    },

    /// A path escaping the repository root via `..`.
    #[error("source location must stay inside the repository, got `{path}`")]
    PathEscapesRepository {
        /// The rejected path.
        path: String,
    },

    /// An empty path, name or evidence string.
    #[error("{field} must not be empty")]
    Empty {
        /// Which field was empty.
        field: &'static str,
    },

    /// A commit identifier that is not a hexadecimal object id.
    ///
    /// Accepts 7 to 64 hex characters, covering abbreviated SHA-1 through full
    /// SHA-256 object ids.
    #[error("commit id must be 7-64 hexadecimal characters, got `{value}`")]
    InvalidCommitId {
        /// The rejected value.
        value: String,
    },

    /// A line or column number of zero.
    ///
    /// Editors, language servers and the CLI all count from one. Accepting a
    /// zero here would produce off-by-one evidence, which is worse than no
    /// evidence.
    #[error("{field} is 1-based and must not be zero")]
    ZeroIndexed {
        /// Which field was zero.
        field: &'static str,
    },
}
