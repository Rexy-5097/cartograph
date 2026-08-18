//! Structured parse diagnostics.
//!
//! A parse problem in one file must not destroy analysis of the rest of the
//! repository: diagnostics are data on the [`FileAnalysis`], never a reason to
//! abort a repository walk.
//!
//! # What a message may contain
//!
//! Structural metadata only: grammar token names, line/column numbers, byte
//! counts. **Never source text**, and never environment values or credentials
//! (RULE 015). The `message` field is designed to be loggable and displayable
//! without redaction.
//!
//! [`FileAnalysis`]: crate::model::FileAnalysis

use serde::{Deserialize, Serialize};

use crate::model::Span;

/// How serious a diagnostic is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Severity {
    /// The file (or a region of it) could not be understood.
    Error,
    /// Something was skipped or approximated; extraction continued.
    Warning,
}

/// What went wrong.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
#[non_exhaustive]
pub enum DiagnosticKind {
    /// The parser found a region it could not parse.
    SyntaxError,
    /// The parser inserted a token the source is missing.
    MissingToken,
    /// The file is not valid UTF-8.
    NotUtf8,
    /// The file could not be read.
    Unreadable,
    /// More diagnostics occurred than the per-file cap; the rest were dropped.
    Truncated,
}

/// One parse problem.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Diagnostic {
    /// How serious this is.
    pub severity: Severity,
    /// What went wrong.
    pub kind: DiagnosticKind,
    /// Human-readable description. Structural metadata only — safe to log and
    /// display, never contains source text.
    pub message: String,
    /// Where, if the problem has a location.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub span: Option<Span>,
}
