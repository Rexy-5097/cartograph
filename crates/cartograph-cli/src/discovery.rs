//! Finding the source files a command should analyse.
//!
//! Shared by `parse` and `normalize` so the two cannot disagree about which
//! files belong to a project — a divergence there would make the two commands
//! silently report on different corpora.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use cartograph_parser::model::SourceLanguage;

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

/// Finds the analysis root and the repository-relative candidate files.
pub fn discover(path: &Path) -> Result<(PathBuf, Vec<String>)> {
    let metadata =
        std::fs::metadata(path).with_context(|| format!("cannot access {}", path.display()))?;

    if metadata.is_file() {
        let root = path.parent().unwrap_or(Path::new(".")).to_path_buf();
        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .context("file name is not valid UTF-8")?
            .to_owned();
        if SourceLanguage::from_path(&name).is_none() {
            bail!("`{name}` is not a TypeScript or TSX file");
        }
        return Ok((root, vec![name]));
    }

    let mut files = Vec::new();
    walk(path, path, &mut files)?;
    files.sort();
    Ok((path.to_path_buf(), files))
}

fn walk(root: &Path, dir: &Path, out: &mut Vec<String>) -> Result<()> {
    for entry in std::fs::read_dir(dir).with_context(|| format!("reading {}", dir.display()))? {
        let entry = entry?;
        let path = entry.path();
        let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
            continue; // non-UTF-8 names cannot become repository-relative paths
        };
        if entry.file_type()?.is_dir() {
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
