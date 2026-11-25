//! A few lines of source around an artefact, for a window to show.
//!
//! **Experimental (M18 browser explorer; proposed ADR-0024).** This is the
//! one place a window can ask for file contents, so it is narrow on purpose:
//!
//! - **Only files the analysis cites.** The requested path must be exactly the
//!   file of a node or an edge in the session's graph. A path the graph does
//!   not mention is refused before any filesystem call, so the lookup cannot
//!   be used to read arbitrary files.
//! - **Only inside the repository.** The joined path is canonicalised and must
//!   remain under the canonical repository root, so `..` and links cannot
//!   leave it.
//! - **Only a window.** At most [`MAX_LINES`] lines around the requested
//!   line, from a file no larger than [`MAX_BYTES`].
//! - **No absolute paths out.** The excerpt names the file as the graph does,
//!   repository-relative.

use std::path::Path;

use serde::Serialize;

use crate::error::{DesktopError, DesktopErrorKind};
use crate::evidence::{AnalysisId, AnalysisSession};
use crate::repository::ValidatedRepository;

/// The most lines one excerpt returns.
pub const MAX_LINES: u32 = 61;
/// The largest file an excerpt is read from.
pub const MAX_BYTES: u64 = 2 * 1024 * 1024;

/// Lines of one file around one line.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceExcerpt {
    /// The analysis whose graph cites the file.
    pub analysis: AnalysisId,
    /// The file, repository-relative, as the graph names it.
    pub file: String,
    /// The 1-based number of the first line returned.
    pub first_line: u32,
    /// The line that was asked for, when one was.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub highlight: Option<u32>,
    /// The lines, without their line endings.
    pub lines: Vec<String>,
    /// How many lines the file has.
    pub total_lines: u32,
}

/// Reads an excerpt of `file` around `line`.
///
/// # Errors
///
/// [`DesktopErrorKind::StaleSelection`] for another analysis;
/// [`DesktopErrorKind::NotFound`] for a file the graph does not cite, one
/// that resolves outside the repository, or one that cannot be read;
/// [`DesktopErrorKind::AnalysisFailed`] for one too large to excerpt.
pub fn excerpt(
    repository: &ValidatedRepository,
    session: &AnalysisSession,
    analysis: AnalysisId,
    file: &str,
    line: Option<u32>,
) -> Result<SourceExcerpt, DesktopError> {
    if analysis != session.id() {
        return Err(DesktopError::new(
            DesktopErrorKind::StaleSelection,
            "That selection belongs to an earlier analysis.",
        )
        .with_hint("Select it again on the current map."));
    }
    if !cited(session, file) {
        return Err(not_available());
    }
    let root = repository
        .path()
        .canonicalize()
        .map_err(|_| not_available())?;
    let full = root
        .join(Path::new(file))
        .canonicalize()
        .map_err(|_| not_available())?;
    if !full.starts_with(&root) || !full.is_file() {
        return Err(not_available());
    }
    let size = std::fs::metadata(&full).map_err(|_| not_available())?.len();
    if size > MAX_BYTES {
        return Err(DesktopError::new(
            DesktopErrorKind::AnalysisFailed,
            "That file is too large to preview.",
        ));
    }
    let bytes = std::fs::read(&full).map_err(|_| not_available())?;
    let text = String::from_utf8_lossy(&bytes);
    let all: Vec<&str> = text.lines().collect();
    let total = u32::try_from(all.len()).unwrap_or(u32::MAX);
    let centre = line.unwrap_or(1).clamp(1, total.max(1));
    let half = MAX_LINES / 2;
    let first = centre.saturating_sub(half).max(1);
    let last = (first + MAX_LINES - 1).min(total);
    let lines = if total == 0 {
        Vec::new()
    } else {
        all[(first - 1) as usize..last as usize]
            .iter()
            .map(|l| (*l).to_owned())
            .collect()
    };
    Ok(SourceExcerpt {
        analysis,
        file: file.to_owned(),
        first_line: first,
        highlight: line.filter(|l| *l >= 1 && *l <= total),
        lines,
        total_lines: total,
    })
}

/// Whether the graph names `file` as the file of a node or an edge.
fn cited(session: &AnalysisSession, file: &str) -> bool {
    let graph = session.graph();
    graph
        .nodes()
        .any(|n| n.location().is_some_and(|l| l.file() == file))
        || graph.edges().any(|e| e.location().file() == file)
}

fn not_available() -> DesktopError {
    DesktopError::new(
        DesktopErrorKind::NotFound,
        "That file is not part of this analysis.",
    )
    .with_hint("Only files the analysis cites can be previewed.")
}
