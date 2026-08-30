//! Finding the source files a command should analyse.
//!
//! Shared by every command so they cannot disagree about which files belong to
//! a project — a divergence there would make two commands silently report on
//! different corpora.
//!
//! # Path handling
//!
//! A path may be `.`, relative, or absolute; all three resolve to the same
//! analysis. What comes *out* is always repository-relative with forward
//! slashes, because a graph is a set of facts about a repository and the
//! absolute location of that repository is a fact about this machine
//! (PART 12).
//!
//! Symbolic links are not followed, for either files or directories. A
//! repository that links to a tree elsewhere on the disk would otherwise pull
//! source from outside the path the user named, and paths outside the analysis
//! root cannot be expressed repository-relative anyway.

use std::path::{Path, PathBuf};

use cartograph_parser::model::SourceLanguage;

use crate::error::{CliError, ErrorCode};

/// Directories never worth descending into.
const SKIP_DIRS: [&str; 11] = [
    "node_modules",
    ".git",
    "dist",
    "build",
    "coverage",
    "target",
    // Python equivalents: vendored or generated trees whose contents are not
    // the project's own source.
    "__pycache__",
    "site-packages",
    "venv",
    ".venv",
    "migrations",
];

/// The file extensions Cartograph reads, for error messages.
const SUPPORTED: &str = ".ts, .tsx, .mts, .cts, .py, .pyi";

/// Finds the analysis root and the repository-relative candidate files.
///
/// # Errors
///
/// [`ErrorCode::InvalidPath`] when the path does not exist, cannot be read, or
/// is a file in a language Cartograph does not parse.
pub fn discover(path: &Path) -> Result<(PathBuf, Vec<String>), CliError> {
    let shown = display(path);

    let metadata = std::fs::metadata(path).map_err(|error| match error.kind() {
        std::io::ErrorKind::NotFound => {
            CliError::new(ErrorCode::InvalidPath, format!("`{shown}` does not exist"))
                .with_hint("give a path to a repository directory, or `.` for the current one")
        }
        std::io::ErrorKind::PermissionDenied => CliError::new(
            ErrorCode::InvalidPath,
            format!("`{shown}` cannot be read: permission denied"),
        ),
        _ => CliError::new(ErrorCode::InvalidPath, format!("`{shown}` cannot be read"))
            .with_hint(format!("underlying cause: {error}")),
    })?;

    if metadata.is_file() {
        let root = path.parent().unwrap_or(Path::new(".")).to_path_buf();
        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .ok_or_else(|| {
                CliError::new(
                    ErrorCode::InvalidPath,
                    "that file's name is not valid UTF-8, so it has no repository-relative form",
                )
            })?
            .to_owned();

        if SourceLanguage::from_path(&name).is_none() {
            return Err(CliError::new(
                ErrorCode::InvalidPath,
                format!("`{name}` is not a file Cartograph can parse"),
            )
            .with_hint(format!("supported extensions are {SUPPORTED}")));
        }
        return Ok((root, vec![name]));
    }

    if !metadata.is_dir() {
        return Err(CliError::new(
            ErrorCode::InvalidPath,
            format!("`{shown}` is neither a file nor a directory"),
        ));
    }

    let mut files = Vec::new();
    walk(path, path, &mut files)?;
    files.sort();
    Ok((path.to_path_buf(), files))
}

/// Whether a path names a location outside the repository, and so must never
/// be reproduced in output.
///
/// `is_absolute()` alone is not this question on Windows. There it requires a
/// drive or UNC prefix, so `\rooted\secret` and `/rooted/secret` — both legal,
/// both resolved against the current drive — report `false` and would be
/// echoed back in full. `has_root()` is `true` for them, and the pair together
/// is the invariant the domain model already enforces by string in
/// `cartograph_core::SourceLocation::new`.
///
/// On Unix the two are equivalent (each means a leading `/`), so this is a
/// no-op there rather than a platform branch. Every relative form stays
/// relative: `..\secret`, `.\secret` and the drive-relative `C:secret` are all
/// `false`, so repository-relative paths are still echoed as typed.
pub(crate) fn is_rooted(path: &Path) -> bool {
    path.is_absolute() || path.has_root()
}

/// How a path is named back to the user.
///
/// A relative path is echoed as typed. A rooted path is reduced to its
/// final component: the user knows which repository they asked about, and the
/// directories above it are not Cartograph's to repeat (PART 12, PART 21).
fn display(path: &Path) -> String {
    if is_rooted(path) {
        path.file_name()
            .and_then(|n| n.to_str())
            .map_or_else(|| "<absolute>".to_owned(), ToOwned::to_owned)
    } else {
        path.to_string_lossy().replace('\\', "/")
    }
}

fn walk(root: &Path, dir: &Path, out: &mut Vec<String>) -> Result<(), CliError> {
    let entries = std::fs::read_dir(dir).map_err(|error| {
        CliError::new(
            ErrorCode::InvalidPath,
            format!("`{}` cannot be listed", display(dir)),
        )
        .with_hint(format!("underlying cause: {error}"))
    })?;

    for entry in entries {
        // A single unreadable entry does not abort the walk: the rest of the
        // repository is still analysable, and refusing all of it because of
        // one bad directory entry would be the wrong trade.
        let Ok(entry) = entry else { continue };
        let path = entry.path();
        let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
            continue; // non-UTF-8 names cannot become repository-relative paths
        };
        let Ok(file_type) = entry.file_type() else {
            continue;
        };

        // `file_type` describes the entry itself, so a symlink is never
        // mistaken for the directory or file it points at.
        if file_type.is_symlink() {
            continue;
        }

        if file_type.is_dir() {
            if name.starts_with('.') || SKIP_DIRS.contains(&name) {
                continue;
            }
            walk(root, &path, out)?;
        } else if SourceLanguage::from_path(name).is_some() {
            if let Ok(rel) = path.strip_prefix(root) {
                // Forward slashes: these are repository-relative fact paths.
                out.push(rel.to_string_lossy().replace('\\', "/"));
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_missing_path_is_an_input_error_naming_the_path() {
        let error = discover(Path::new("definitely/not/here")).expect_err("must fail");
        assert_eq!(error.code, ErrorCode::InvalidPath);
        assert!(error.message.contains("definitely/not/here"), "{error}");
        assert!(error.hint.is_some(), "an actionable hint is expected");
    }

    #[test]
    fn an_absolute_missing_path_is_not_echoed_in_full() {
        let error = discover(Path::new("/nonexistent-root-xyz/secret-project")).expect_err("fails");
        assert!(
            !error.message.contains("/nonexistent-root-xyz"),
            "the absolute path leaked into the message: {error}"
        );
        assert!(error.message.contains("secret-project"), "{error}");
    }

    /// The classification `display` depends on, pinned per form.
    ///
    /// `\rooted\secret` and `/rooted/secret` are the regression: on Windows
    /// neither is `is_absolute()`, so both were echoed back in full. The
    /// relative rows guard the other direction — a fix that redacted
    /// everything would be no fix at all.
    #[test]
    fn every_rooted_form_is_rooted_and_no_relative_form_is() {
        for rooted in [
            r"C:\workspace\secret-project",
            "C:/secret-project",
            r"\rooted\secret-project",
            "/rooted/secret-project",
            concat!(r"\", r"\", r"server\share\secret-project"),
        ] {
            assert!(
                is_rooted(Path::new(rooted)),
                "{rooted} must be treated as rooted"
            );
        }

        for relative in [
            "relative/path",
            r"relative\path",
            "./relative/path",
            r".\secret-project",
            r"..\secret-project",
            "../sibling",
            "C:secret-project", // drive-relative, not rooted
            ".",
        ] {
            assert!(
                !is_rooted(Path::new(relative)),
                "{relative} must stay relative and be echoed as typed"
            );
        }
    }

    /// Windows accepts a path rooted on the current drive, with no letter.
    /// `is_absolute()` is false for it, which is exactly how it leaked.
    #[test]
    fn a_rooted_drive_less_missing_path_is_not_echoed_in_full() {
        for rooted in [
            r"\nonexistent-root-xyz\secret-project",
            "/nonexistent-root-xyz/secret-project",
        ] {
            let error = discover(Path::new(rooted)).expect_err("fails");
            assert!(
                !error.message.contains("nonexistent-root-xyz"),
                "the rooted path leaked into the message for {rooted}: {error}"
            );
            assert!(
                error.message.contains("secret-project"),
                "the repository name is still owed to the user: {error}"
            );
        }
    }

    /// A relative path is the user's own words and is echoed unchanged, so the
    /// redaction cannot be implemented by redacting everything.
    #[test]
    fn a_relative_path_is_echoed_as_typed() {
        assert_eq!(display(Path::new("crates/x")), "crates/x");
        assert_eq!(display(Path::new(r"crates\x")), "crates/x");
        assert_eq!(display(Path::new("../sibling")), "../sibling");
    }

    #[test]
    fn an_unsupported_file_names_the_supported_extensions() {
        let manifest = Path::new(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml");
        let error = discover(&manifest).expect_err("Cargo.toml is not parseable source");
        assert_eq!(error.code, ErrorCode::InvalidPath);
        assert!(error.hint.unwrap_or_default().contains(".tsx"));
    }

    #[test]
    fn a_supported_file_analyses_just_that_file() {
        let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .join("cartograph-parser/tests/fixtures/http.ts");
        let (_, files) = discover(&fixture).expect("a .ts file is analysable");
        assert_eq!(files, vec!["http.ts".to_owned()]);
    }

    #[test]
    fn discovered_paths_are_relative_and_use_forward_slashes() {
        let fixtures = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .join("cartograph-parser/tests/fixtures");
        let (_, files) = discover(&fixtures).expect("fixtures exist");
        assert!(!files.is_empty());
        for file in &files {
            assert!(!file.contains('\\'), "backslash in {file}");
            assert!(!Path::new(file).is_absolute(), "absolute path {file}");
            assert!(!file.contains(".."), "traversal in {file}");
        }
        assert!(files.iter().any(|f| f.starts_with("python/")));
    }

    #[test]
    fn python_files_are_discovered_alongside_typescript() {
        let fixtures = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .join("cartograph-parser/tests/fixtures");
        let (_, files) = discover(&fixtures).expect("fixtures exist");
        let language = |name: &str| SourceLanguage::from_path(name);
        assert!(
            files
                .iter()
                .any(|f| language(f) == Some(SourceLanguage::Python)),
            "no Python found"
        );
        assert!(
            files
                .iter()
                .any(|f| language(f) == Some(SourceLanguage::TypeScript)),
            "no TypeScript found"
        );
    }
}
