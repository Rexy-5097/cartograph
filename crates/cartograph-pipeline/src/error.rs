//! Exit codes and the user-facing error model.
//!
//! # Why exit codes are a type
//!
//! A script wrapping Cartograph needs to tell "this repository has nothing to
//! analyse" apart from "that symbol does not exist" apart from "the analyser
//! broke". Returning `1` for all three makes the tool unscriptable, and
//! returning an ad-hoc integer from wherever the failure happened makes the
//! meaning drift. Every exit path in the binary goes through [`ExitCode`], and
//! the codes are documented in `docs/development/cli.md` as a contract.
//!
//! # Why errors carry a machine-readable slug
//!
//! `--json` consumers should not have to pattern-match English. Every
//! [`CliError`] has an [`ErrorCode`] that serialises to a stable kebab-case
//! string, alongside a sentence for a human and, where one genuinely helps, a
//! hint naming the next action.

use std::fmt;

use serde::Serialize;

/// The process exit status.
///
/// `1` is deliberately unused: it is what a panicking or aborting process
/// would produce, and keeping it distinct from every deliberate failure means
/// an unexpected `1` is always a bug report rather than an ambiguous signal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i32)]
pub enum ExitCode {
    /// Analysis completed. Diagnostics may still have been reported.
    Success = 0,
    /// The command line was wrong: unknown flag, missing argument, bad value.
    /// Produced by clap, whose own convention this matches.
    Usage = 2,
    /// The path could not be used: missing, unreadable, or holding no
    /// supported source.
    Input = 3,
    /// The analyser failed. A defect or an environment problem, not user error.
    Analysis = 4,
    /// `trace` was given a symbol the graph does not contain.
    NotFound = 5,
    /// `trace` was given a symbol that matches more than one node. No node is
    /// chosen; the caller must be more specific.
    Ambiguous = 6,
    /// Analysis completed but some files could not be fully parsed, and
    /// `--strict` asked for that to be a failure.
    Partial = 7,
}

impl ExitCode {
    /// The integer handed to the operating system.
    #[must_use]
    pub fn code(self) -> i32 {
        self as i32
    }
}

/// A stable, machine-readable classification of a failure.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ErrorCode {
    /// The path does not exist, or cannot be read.
    InvalidPath,
    /// The path exists but holds no file in a supported language.
    NoSupportedSources,
    /// Extraction or resolution failed.
    AnalysisFailed,
    /// No graph node carries the requested name.
    SymbolNotFound,
    /// More than one graph node carries the requested name.
    AmbiguousSymbol,
    /// Files were parsed only partially, and `--strict` is in force.
    PartialAnalysis,
}

impl ErrorCode {
    /// The exit status this class of failure produces.
    #[must_use]
    pub fn exit(self) -> ExitCode {
        match self {
            Self::InvalidPath | Self::NoSupportedSources => ExitCode::Input,
            Self::AnalysisFailed => ExitCode::Analysis,
            Self::SymbolNotFound => ExitCode::NotFound,
            Self::AmbiguousSymbol => ExitCode::Ambiguous,
            Self::PartialAnalysis => ExitCode::Partial,
        }
    }

    /// The slug as it appears in `--json` output.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::InvalidPath => "invalid-path",
            Self::NoSupportedSources => "no-supported-sources",
            Self::AnalysisFailed => "analysis-failed",
            Self::SymbolNotFound => "symbol-not-found",
            Self::AmbiguousSymbol => "ambiguous-symbol",
            Self::PartialAnalysis => "partial-analysis",
        }
    }
}

/// A failure worth showing a user.
///
/// The message states what went wrong in terms of what the user asked for; the
/// hint, when present, states what to do instead. Internal detail — a
/// `tree-sitter` error string, an IO errno, a backtrace — belongs in a
/// `tracing` event at `-v`, not in the message.
#[derive(Debug, Clone, Serialize)]
pub struct CliError {
    /// Stable classification.
    pub code: ErrorCode,
    /// One sentence, addressed to the person who ran the command.
    pub message: String,
    /// The next action, when there is an obvious one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hint: Option<String>,
    /// Candidates, when the failure is that there were too many or none.
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub candidates: Vec<String>,
}

impl CliError {
    /// A failure with no further advice.
    pub fn new(code: ErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            hint: None,
            candidates: Vec::new(),
        }
    }

    /// Adds the next action a user should take.
    #[must_use]
    pub fn with_hint(mut self, hint: impl Into<String>) -> Self {
        self.hint = Some(hint.into());
        self
    }

    /// Adds the alternatives that made the request unanswerable.
    #[must_use]
    pub fn with_candidates(mut self, candidates: Vec<String>) -> Self {
        self.candidates = candidates;
        self
    }

    /// The exit status this failure produces.
    #[must_use]
    pub fn exit(&self) -> ExitCode {
        self.code.exit()
    }
}

impl fmt::Display for CliError {
    /// The human rendering, used for stderr.
    ///
    /// The slug is included the way `rustc` includes an error number: a person
    /// reading it can ignore the bracket, and a script grepping stderr has
    /// something stable to match that is not an English sentence.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "error[{}]: {}", self.code.as_str(), self.message)?;
        for candidate in &self.candidates {
            write!(f, "\n  {candidate}")?;
        }
        if let Some(hint) = &self.hint {
            write!(f, "\nhint: {hint}")?;
        }
        Ok(())
    }
}

impl std::error::Error for CliError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exit_codes_are_the_documented_integers() {
        // These are a public contract: scripts branch on them. Changing one is
        // a breaking change, and this test is where that decision surfaces.
        assert_eq!(ExitCode::Success.code(), 0);
        assert_eq!(ExitCode::Usage.code(), 2);
        assert_eq!(ExitCode::Input.code(), 3);
        assert_eq!(ExitCode::Analysis.code(), 4);
        assert_eq!(ExitCode::NotFound.code(), 5);
        assert_eq!(ExitCode::Ambiguous.code(), 6);
        assert_eq!(ExitCode::Partial.code(), 7);
    }

    #[test]
    fn one_is_never_a_deliberate_exit_code() {
        // Reserved for panics, so an unexpected 1 always means a defect.
        for code in [
            ExitCode::Success,
            ExitCode::Usage,
            ExitCode::Input,
            ExitCode::Analysis,
            ExitCode::NotFound,
            ExitCode::Ambiguous,
            ExitCode::Partial,
        ] {
            assert_ne!(code.code(), 1);
        }
    }

    #[test]
    fn every_error_code_maps_to_a_non_success_exit() {
        for code in [
            ErrorCode::InvalidPath,
            ErrorCode::NoSupportedSources,
            ErrorCode::AnalysisFailed,
            ErrorCode::SymbolNotFound,
            ErrorCode::AmbiguousSymbol,
            ErrorCode::PartialAnalysis,
        ] {
            assert_ne!(
                code.exit(),
                ExitCode::Success,
                "{code:?} exits successfully"
            );
        }
    }

    #[test]
    fn error_slugs_are_stable_kebab_case() {
        assert_eq!(ErrorCode::InvalidPath.as_str(), "invalid-path");
        assert_eq!(
            ErrorCode::NoSupportedSources.as_str(),
            "no-supported-sources"
        );
        assert_eq!(ErrorCode::SymbolNotFound.as_str(), "symbol-not-found");
        assert_eq!(ErrorCode::AmbiguousSymbol.as_str(), "ambiguous-symbol");
        assert_eq!(ErrorCode::AnalysisFailed.as_str(), "analysis-failed");
        assert_eq!(ErrorCode::PartialAnalysis.as_str(), "partial-analysis");
    }

    #[test]
    fn the_json_slug_and_the_serde_name_agree() {
        // Two spellings of the same thing is how a contract quietly breaks.
        for code in [
            ErrorCode::InvalidPath,
            ErrorCode::NoSupportedSources,
            ErrorCode::AnalysisFailed,
            ErrorCode::SymbolNotFound,
            ErrorCode::AmbiguousSymbol,
            ErrorCode::PartialAnalysis,
        ] {
            let serialised = serde_json::to_string(&code).expect("serialises");
            assert_eq!(serialised, format!("\"{}\"", code.as_str()));
        }
    }

    #[test]
    fn the_human_rendering_carries_candidates_and_hint() {
        let error = CliError::new(ErrorCode::AmbiguousSymbol, "`User` matches 2 symbols")
            .with_candidates(vec!["a.py:1".to_owned(), "b.py:2".to_owned()])
            .with_hint("qualify with file:name");
        let rendered = error.to_string();
        assert!(
            rendered.starts_with("error[ambiguous-symbol]: `User` matches 2 symbols"),
            "{rendered}"
        );
        assert!(rendered.contains("\n  a.py:1"));
        assert!(rendered.contains("\nhint: qualify with file:name"));
    }
}
