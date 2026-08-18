//! The `cartograph parse` command: run the M01 extractor over a file or tree.
//!
//! # Why `parse` exists as its own command
//!
//! Languages are detected from the file extension: TypeScript/TSX (M01) and
//! Python (M02) are recognised, and a mixed repository is walked in one pass.
//!
//! `cartograph analyze` is the M09 deliverable — the full pipeline producing a
//! graph. Shipping it now, wired only to extraction, would misrepresent what
//! the product does (a command that exists but half-works is a documentation
//! defect). `parse` is honest about its scope: it prints **extracted syntactic
//! facts and diagnostics**, exactly what M01 built, and it makes the extractor
//! genuinely testable from the shell. It reports observations, never edges.
//!
//! Output: a human summary by default, the full fact model with `--json`
//! (stdout is pure JSON; logs stay on stderr).

use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use cartograph_parser::Analyzer;
use cartograph_parser::model::{FileAnalysis, ParseStatus, SourceLanguage};
use serde::Serialize;
use tracing::debug;

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

/// Aggregate counts across a run.
#[derive(Debug, Default, Serialize)]
struct Totals {
    files: usize,
    parsed_clean: usize,
    parsed_with_errors: usize,
    failed: usize,
    imports: usize,
    symbols: usize,
    calls: usize,
    strings: usize,
    http_observations: usize,
    route_observations: usize,
    diagnostics: usize,
}

/// The `--json` payload.
#[derive(Debug, Serialize)]
struct JsonReport {
    root: String,
    totals: Totals,
    files: Vec<FileAnalysis>,
}

/// Runs extraction over `path` (a file or a directory tree).
pub fn run(path: &Path, json: bool) -> Result<()> {
    let (root, files) = discover(path)?;
    if files.is_empty() {
        bail!("no TypeScript or TSX files under {}", path.display());
    }

    let mut analyzer = Analyzer::new().context("loading grammars")?;
    let started = std::time::Instant::now();

    let mut results = Vec::with_capacity(files.len());
    for rel in &files {
        // Path and extension were validated by discovery; a per-file failure
        // is a Failed analysis, so the walk itself cannot abort here.
        let file_analysis = analyzer
            .analyze_file(&root, rel)
            .with_context(|| format!("analysing {rel}"))?;
        results.push(file_analysis);
    }
    let elapsed = started.elapsed();

    let totals = tally(&results);
    debug!(files = totals.files, ?elapsed, "extraction finished");

    if json {
        let report = JsonReport {
            root: path.display().to_string(),
            totals,
            files: results,
        };
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else {
        print_summary(&results, &totals, elapsed);
    }
    Ok(())
}

/// Finds the analysis root and the repository-relative candidate files.
fn discover(path: &Path) -> Result<(PathBuf, Vec<String>)> {
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

fn tally(analyses: &[FileAnalysis]) -> Totals {
    let mut totals = Totals::default();
    for analysis in analyses {
        totals.files += 1;
        match analysis.status {
            ParseStatus::Complete => totals.parsed_clean += 1,
            ParseStatus::CompleteWithErrors => totals.parsed_with_errors += 1,
            ParseStatus::Failed => totals.failed += 1,
        }
        totals.imports += analysis.imports.len();
        totals.symbols += analysis.symbols.len();
        totals.calls += analysis.calls.len();
        totals.strings += analysis.strings.len();
        totals.http_observations += analysis.http_calls.len();
        totals.route_observations += analysis.routes.len();
        totals.diagnostics += analysis.diagnostics.len();
    }
    totals
}

fn print_summary(analyses: &[FileAnalysis], totals: &Totals, elapsed: std::time::Duration) {
    for analysis in analyses {
        let status = match analysis.status {
            ParseStatus::Complete => "ok",
            ParseStatus::CompleteWithErrors => "errors",
            ParseStatus::Failed => "FAILED",
        };
        println!(
            "{:<7} {}  symbols {:<4} imports {:<3} calls {:<4} strings {:<4} http {:<3} routes {}",
            status,
            analysis.path,
            analysis.symbols.len(),
            analysis.imports.len(),
            analysis.calls.len(),
            analysis.strings.len(),
            analysis.http_calls.len(),
            analysis.routes.len(),
        );
        for diagnostic in &analysis.diagnostics {
            match diagnostic.span {
                Some(span) => println!(
                    "        - {}:{} {}",
                    analysis.path, span, diagnostic.message
                ),
                None => println!("        - {}: {}", analysis.path, diagnostic.message),
            }
        }
    }
    println!();
    println!(
        "Files          {} ({} clean, {} with errors, {} failed)",
        totals.files, totals.parsed_clean, totals.parsed_with_errors, totals.failed
    );
    println!("Symbols        {}", totals.symbols);
    println!("Imports        {}", totals.imports);
    println!("Call sites     {}", totals.calls);
    println!("Strings        {}", totals.strings);

    println!(
        "HTTP observed  {}   (syntactic observations, not resolved edges)",
        totals.http_observations
    );
    println!(
        "Routes observed {}  (declarations found in source, not verified endpoints)",
        totals.route_observations
    );
    println!("Diagnostics    {}", totals.diagnostics);
    println!("Completed in   {elapsed:.2?}");
}
