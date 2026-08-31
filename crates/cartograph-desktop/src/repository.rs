//! Validating a path the user chose.
//!
//! # A repository path is untrusted input
//!
//! It arrives from a file dialog, or from whatever the frontend put in the
//! field, and it is a string until proven otherwise. Every check here answers
//! a question the analyser would otherwise answer by failing later and less
//! clearly: does it exist, is it a directory, can it be read.
//!
//! Validation is deliberately **read-only and non-executing**. Nothing here
//! runs a file, reads a file's contents, follows a configuration, or asks the
//! repository what it would like to happen. A hostile repository is a
//! directory full of bytes, and it stays that way.
//!
//! # Paths are not echoed
//!
//! A message naming `C:\workspace\clients\acme-secret` leaks two things:
//! the user's identity and the name of a project they may not have announced.
//! M09 found exactly this defect in the CLI and fixed it with
//! `discovery::is_rooted`; the same rule applies here, and
//! [`display_name`] is the only thing that reaches a message.

use std::path::{Path, PathBuf};

use cartograph_pipeline::discovery;

use crate::error::{DesktopError, DesktopErrorKind};

/// A path that has been checked and may be analysed.
///
/// Constructing one is the *only* way to reach [`crate::session::analyze`], so
/// an unvalidated path cannot reach the analyser by mistake.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedRepository {
    path: PathBuf,
    display: String,
}

impl ValidatedRepository {
    /// The path on disk. Never sent to the frontend.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// The name safe to show: the final component, or `<absolute>` for a
    /// rooted path with no usable component.
    #[must_use]
    pub fn display_name(&self) -> &str {
        &self.display
    }
}

/// The safe-to-show form of a path.
///
/// A relative path is shown as typed — it reveals nothing the user did not
/// already type, and hiding it would make the application confusing for no
/// gain. A rooted path is reduced to its final component, because that is the
/// part the user recognises and the rest is the part that identifies them.
///
/// This mirrors `discovery::is_rooted`, which is the predicate M09's Windows
/// privacy fix introduced: `Path::is_absolute` alone is false for a
/// drive-less rooted path such as `\projects\secret`, and that gap is how the
/// original leak happened.
#[must_use]
pub fn display_name(path: &Path) -> String {
    if discovery::is_rooted(path) {
        path.file_name().map_or_else(
            || "<absolute>".to_owned(),
            |n| n.to_string_lossy().into_owned(),
        )
    } else {
        path.to_string_lossy().into_owned()
    }
}

/// Checks that `path` is an existing, readable directory.
///
/// # Errors
///
/// - [`DesktopErrorKind::NotFound`] if nothing is there;
/// - [`DesktopErrorKind::NotADirectory`] if it is a file;
/// - [`DesktopErrorKind::PermissionDenied`] if it cannot be read.
///
/// The three are distinguished because the user's next action differs for
/// each, and a frontend that shows one message for all three cannot help.
pub fn validate(path: &Path) -> Result<ValidatedRepository, DesktopError> {
    let shown = display_name(path);

    if path.as_os_str().is_empty() {
        return Err(
            DesktopError::new(DesktopErrorKind::NotFound, "No folder was chosen.")
                .with_hint("Choose a repository folder to analyse."),
        );
    }

    // `symlink_metadata` would refuse a symlinked repository, which is a
    // normal way to keep a checkout elsewhere, so the link is followed
    // deliberately. Following it cannot execute anything.
    let metadata = match std::fs::metadata(path) {
        Ok(metadata) => metadata,
        Err(error) => {
            return Err(match error.kind() {
                std::io::ErrorKind::NotFound => DesktopError::new(
                    DesktopErrorKind::NotFound,
                    format!("`{shown}` does not exist."),
                )
                .with_hint("Check the folder is still there, then choose it again."),
                std::io::ErrorKind::PermissionDenied => DesktopError::new(
                    DesktopErrorKind::PermissionDenied,
                    format!("`{shown}` cannot be read."),
                )
                .with_hint("Check the folder's permissions, or choose another."),
                // Anything else is the operating system telling us something
                // this code does not model. Reporting it as "not found" would
                // be a guess, so it is reported as what it is.
                _ => DesktopError::new(
                    DesktopErrorKind::NotFound,
                    format!("`{shown}` could not be opened."),
                )
                .with_hint("Choose another folder."),
            });
        }
    };

    if !metadata.is_dir() {
        return Err(DesktopError::new(
            DesktopErrorKind::NotADirectory,
            format!("`{shown}` is a file, not a folder."),
        )
        .with_hint("Choose the repository's folder instead."));
    }

    // Existence and type are not readability. A directory can be stat-able and
    // still refuse enumeration, which is the case that would otherwise surface
    // much later as a confusing empty analysis.
    if let Err(error) = std::fs::read_dir(path) {
        let kind = if error.kind() == std::io::ErrorKind::PermissionDenied {
            DesktopErrorKind::PermissionDenied
        } else {
            DesktopErrorKind::NotFound
        };
        return Err(
            DesktopError::new(kind, format!("`{shown}` cannot be listed."))
                .with_hint("Check the folder's permissions, or choose another."),
        );
    }

    Ok(ValidatedRepository {
        path: path.to_path_buf(),
        display: shown,
    })
}
