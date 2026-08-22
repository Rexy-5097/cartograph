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

use std::fmt::Write as _;

use cartograph_parser::Analyzer;
use cartograph_parser::model::{FileAnalysis, ParseStatus};

use crate::discovery;
use crate::error::{CliError, ErrorCode, ExitCode};
use crate::json::SCHEMA_VERSION;
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
///
/// `schema_version` and `command` are the envelope every `--json` document
/// carries (see `json`); the body predates it and is consumed unchanged by the
/// frozen M07/M08 benchmark harness, so it is extended rather than reshaped.
#[derive(Debug, Serialize)]
struct JsonReport {
    schema_version: &'static str,
    command: &'static str,
    /// What was analysed. Never the absolute path (PART 12).
    repository: crate::pipeline::Repository,
    totals: Totals,
    files: Vec<FileAnalysis>,
}

/// Runs extraction over `path` (a file or a directory tree).
///
/// # Errors
///
/// [`ErrorCode::InvalidPath`] or [`ErrorCode::NoSupportedSources`] when the
/// path cannot be analysed, [`ErrorCode::AnalysisFailed`] when extraction
/// itself breaks.
pub fn run(path: &Path, json: bool) -> Result<(String, ExitCode), CliError> {
    let (root, files) = discovery::discover(path)?;
    if files.is_empty() {
        return Err(CliError::new(
            ErrorCode::NoSupportedSources,
            "no TypeScript, TSX or Python source files were found under that path",
        )
        .with_hint("supported extensions are .ts, .tsx, .mts, .cts, .py, .pyi"));
    }

    let mut analyzer = Analyzer::new().map_err(|error| {
        CliError::new(
            ErrorCode::AnalysisFailed,
            "the TypeScript, TSX and Python grammars could not be loaded",
        )
        .with_hint(format!("underlying cause: {error}"))
    })?;
    let started = std::time::Instant::now();

    let mut results = Vec::with_capacity(files.len());
    for rel in &files {
        // Path and extension were validated by discovery; a per-file failure
        // is a Failed analysis, so the walk itself cannot abort here.
        let file_analysis = analyzer.analyze_file(&root, rel).map_err(|error| {
            CliError::new(
                ErrorCode::AnalysisFailed,
                format!("analysis failed while reading `{rel}`"),
            )
            .with_hint(format!("underlying cause: {error}"))
        })?;
        results.push(file_analysis);
    }
    let elapsed = started.elapsed();

    let totals = tally(&results);
    debug!(files = totals.files, ?elapsed, "extraction finished");

    if json {
        let report = JsonReport {
            schema_version: SCHEMA_VERSION,
            command: "parse",
            repository: crate::pipeline::Repository::describe(path, &root),
            totals,
            files: results,
        };
        let mut text = serde_json::to_string_pretty(&report).map_err(|error| {
            CliError::new(
                ErrorCode::AnalysisFailed,
                "the result could not be serialised as JSON",
            )
            .with_hint(format!("underlying cause: {error}"))
        })?;
        text.push('\n');
        Ok((text, ExitCode::Success))
    } else {
        Ok((print_summary(&results, &totals, elapsed), ExitCode::Success))
    }
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

fn print_summary(
    analyses: &[FileAnalysis],
    totals: &Totals,
    elapsed: std::time::Duration,
) -> String {
    let mut out = String::with_capacity(1024);
    for analysis in analyses {
        let status = match analysis.status {
            ParseStatus::Complete => "ok",
            ParseStatus::CompleteWithErrors => "errors",
            ParseStatus::Failed => "FAILED",
        };
        let _ = writeln!(
            out,
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
                Some(span) => {
                    let _ = writeln!(
                        out,
                        "        - {}:{} {}",
                        analysis.path, span, diagnostic.message
                    );
                }
                None => {
                    let _ = writeln!(out, "        - {}: {}", analysis.path, diagnostic.message);
                }
            }
        }
    }
    let _ = writeln!(out);
    let _ = writeln!(
        out,
        "Files          {} ({} clean, {} with errors, {} failed)",
        totals.files, totals.parsed_clean, totals.parsed_with_errors, totals.failed
    );
    let _ = writeln!(out, "Symbols        {}", totals.symbols);
    let _ = writeln!(out, "Imports        {}", totals.imports);
    let _ = writeln!(out, "Call sites     {}", totals.calls);
    let _ = writeln!(out, "Strings        {}", totals.strings);

    let _ = writeln!(
        out,
        "HTTP observed  {}   (syntactic observations, not resolved edges)",
        totals.http_observations
    );
    let _ = writeln!(
        out,
        "Routes observed {}  (declarations found in source, not verified endpoints)",
        totals.route_observations
    );
    let _ = writeln!(out, "Diagnostics    {}", totals.diagnostics);
    let _ = writeln!(out, "Completed in   {elapsed:.2?}");

    out
}
