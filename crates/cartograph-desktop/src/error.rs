//! The failure contract the desktop frontend consumes.
//!
//! # Why this is not a string
//!
//! A frontend that decides what went wrong by matching on message text breaks
//! the first time someone improves the wording, and it breaks silently — the
//! branch simply stops being taken. So every failure crossing the boundary
//! carries a [`DesktopErrorKind`], and the frontend switches on that and never
//! on [`DesktopError::message`].
//!
//! The message exists to be *shown*, not parsed.
//!
//! # Why not reuse `ErrorCode` directly
//!
//! `cartograph_pipeline::error::ErrorCode` is the CLI's classification and
//! carries cases a desktop window has no use for — `SymbolNotFound` and
//! `AmbiguousSymbol` belong to `cartograph trace`. It also lacks the two the
//! desktop genuinely needs and the CLI never encounters: a path that is a file
//! rather than a directory, and a directory the process may not read.
//!
//! Rather than widen a stable contract for a second consumer, this maps into a
//! kind set the desktop actually has states for. The mapping is total, and the
//! test suite asserts that every `ErrorCode` has a destination — a new variant
//! upstream fails the build here rather than silently becoming `Internal`.

use serde::{Deserialize, Serialize};

use cartograph_pipeline::error::{CliError, ErrorCode};

/// What kind of failure occurred.
///
/// Serialised in `camelCase` so the TypeScript side reads naturally. These are
/// the only values the frontend may branch on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum DesktopErrorKind {
    /// The path does not exist.
    NotFound,
    /// The path exists but is a file, not a directory.
    NotADirectory,
    /// The path exists and is a directory, but could not be read.
    PermissionDenied,
    /// The directory holds no file in a supported language.
    NoSupportedSources,
    /// Discovery, parsing or resolution failed.
    AnalysisFailed,
    /// The user abandoned the operation before it completed.
    ///
    /// See [`crate::session`] for what this does and does not mean: the
    /// frontend stops waiting, but analysis already in flight runs to
    /// completion.
    Cancelled,
    /// The selection belongs to an analysis that has been replaced.
    ///
    /// Not cosmetic. `EdgeId` is graph-local and restarts at zero per
    /// analysis, so a stale id usually *exists* in the new graph as a
    /// different relationship — answering it would show confident, wrong
    /// evidence. Refusing is the only safe response.
    StaleSelection,
    /// The edge is not part of the current analysis.
    UnknownEdge,
    /// The artefact is not part of the current analysis.
    ///
    /// Distinct from [`Self::UnknownEdge`] so a surface can say which kind of
    /// handle went stale, and distinct from an empty result: "nothing depends
    /// on this" is an answer, "this is not here" is a disagreement between the
    /// window and the graph.
    UnknownNode,
    /// Evidence was requested before anything had been analysed.
    NoAnalysis,
    /// A defect. Reaching this means Cartograph is wrong, not the user.
    Internal,
}

impl DesktopErrorKind {
    /// Whether the user can plausibly fix this by choosing a different
    /// repository.
    ///
    /// Drives whether the frontend offers "choose another folder" or "report
    /// this", which is a decision about wording and should not be made by
    /// guessing from the kind at three different call sites.
    #[must_use]
    pub const fn is_user_correctable(self) -> bool {
        matches!(
            self,
            Self::NotFound
                | Self::NotADirectory
                | Self::PermissionDenied
                | Self::NoSupportedSources
        )
    }
}

/// A failure, ready to be shown and ready to be branched on.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DesktopError {
    /// Stable classification. Branch on this.
    pub kind: DesktopErrorKind,
    /// One sentence addressed to the person using the application.
    ///
    /// Never contains file contents, and never contains an absolute path — see
    /// the module tests, which assert both.
    pub message: String,
    /// The next action, when there is an obvious one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hint: Option<String>,
}

impl DesktopError {
    /// A failure with no further advice.
    #[must_use]
    pub fn new(kind: DesktopErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
            hint: None,
        }
    }

    /// Adds the next action.
    #[must_use]
    pub fn with_hint(mut self, hint: impl Into<String>) -> Self {
        self.hint = Some(hint.into());
        self
    }

    /// The defect case, kept short so it is obvious at a call site that this is
    /// not a user error.
    #[must_use]
    pub fn internal(message: impl Into<String>) -> Self {
        Self::new(DesktopErrorKind::Internal, message)
            .with_hint("This is a defect in Cartograph. Please report it.")
    }
}

impl std::fmt::Display for DesktopError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for DesktopError {}

impl From<CliError> for DesktopError {
    /// Total by construction: every [`ErrorCode`] has a destination, and the
    /// match is exhaustive so a new variant upstream is a compile error here
    /// rather than a silent `Internal`.
    fn from(error: CliError) -> Self {
        let kind = match error.code {
            ErrorCode::InvalidPath => DesktopErrorKind::NotFound,
            ErrorCode::NoSupportedSources => DesktopErrorKind::NoSupportedSources,
            ErrorCode::AnalysisFailed | ErrorCode::PartialAnalysis => {
                DesktopErrorKind::AnalysisFailed
            }
            // `trace`-only classifications. A desktop session never issues a
            // symbol query, so reaching these means the caller did something
            // this crate does not model.
            ErrorCode::SymbolNotFound | ErrorCode::AmbiguousSymbol => DesktopErrorKind::Internal,
        };
        Self {
            kind,
            message: error.message,
            hint: error.hint,
        }
    }
}
