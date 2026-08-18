//! Parser API errors.
//!
//! These are *caller* errors and *environment* errors — an invalid path, an
//! unsupported extension, a grammar that failed to load. Problems inside a
//! source file are never a `ParserError`: they become diagnostics on the
//! [`FileAnalysis`](crate::model::FileAnalysis), because one broken file must
//! not abort a repository walk.

use thiserror::Error;

/// Failure to *begin* analysing, as opposed to problems found while analysing.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum ParserError {
    /// The path failed the domain model's repository-relative rules
    /// (absolute, `..` traversal, or empty).
    #[error("invalid analysis path: {0}")]
    InvalidPath(#[from] cartograph_core::CoreError),

    /// The file extension maps to no supported language.
    #[error("unsupported language for `{path}`")]
    UnsupportedLanguage {
        /// The repository-relative path that was offered.
        path: String,
    },

    /// A grammar failed to load into the parsing runtime — a build/link
    /// problem, not a source problem.
    #[error("failed to load {language} grammar: {detail}")]
    Grammar {
        /// Which grammar.
        language: &'static str,
        /// Runtime-reported detail.
        detail: String,
    },
}
