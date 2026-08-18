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

use std::path::Path;

use anyhow::{Context, Result, bail};
use cartograph_parser::Analyzer;
use cartograph_parser::model::{FileAnalysis, ParseStatus};

use crate::discovery;
use serde::Serialize;
use tracing::debug;

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
    let (root, files) = discovery::discover(path)?;
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
