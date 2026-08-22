//! The `cartograph normalize` command: canonicalise the routes and HTTP calls
//! observed in a file or tree.
//!
//! # What this command shows, and what it does not
//!
//! It shows each observation's **raw** form beside its **canonical** form —
//! one line per observation, both sides visible. That pairing is the point:
//! canonicalisation is a transformation of a single observation, and seeing
//! the input next to the output makes it checkable.
//!
//! It never pairs a client call with a route declaration. Joining the two
//! sides is M04's work, and printing them as if joined would be exactly the
//! misrepresentation this milestone exists to avoid.

use std::path::Path;

use std::fmt::Write as _;

use cartograph_parser::Analyzer;
use cartograph_parser::model::FileAnalysis;
use cartograph_resolver::{
    CanonicalRoute, NormalizationStatus, ObservationKind, normalize_client_call,
    normalize_route_declaration,
};
use serde::Serialize;

use crate::discovery;
use crate::error::{CliError, ErrorCode, ExitCode};
use crate::json::SCHEMA_VERSION;

/// Aggregate counts across a run.
#[derive(Debug, Default, Serialize)]
struct Totals {
    files: usize,
    route_declarations: usize,
    client_calls: usize,
    exact: usize,
    partial: usize,
    unsupported: usize,
}

/// The `--json` payload.
///
/// `schema_version` and `command` are the envelope every `--json` document
/// carries; the body is consumed unchanged by the frozen M07/M08 benchmark
/// harness, so it is extended rather than reshaped.
#[derive(Debug, Serialize)]
struct JsonReport {
    schema_version: &'static str,
    command: &'static str,
    /// What was analysed. Never the absolute path (PART 12).
    repository: crate::pipeline::Repository,
    totals: Totals,
    /// Every canonicalised observation, each independent of the others.
    canonical_routes: Vec<CanonicalRoute>,
}

/// Canonicalises every observation under `path`.
///
/// # Errors
///
/// [`ErrorCode::InvalidPath`] when the path cannot be analysed, and
/// [`ErrorCode::AnalysisFailed`] when extraction itself breaks.
pub fn run(path: &Path, json: bool) -> Result<(String, ExitCode), CliError> {
    let (root, files) = discovery::discover(path)?;
    let mut analyzer = Analyzer::new().map_err(|error| {
        CliError::new(
            ErrorCode::AnalysisFailed,
            "the TypeScript, TSX and Python grammars could not be loaded",
        )
        .with_hint(format!("underlying cause: {error}"))
    })?;

    let mut canonical = Vec::new();
    let mut totals = Totals::default();

    for rel in &files {
        let analysis: FileAnalysis = analyzer.analyze_file(&root, rel).map_err(|error| {
            CliError::new(
                ErrorCode::AnalysisFailed,
                format!("analysis failed while reading `{rel}`"),
            )
            .with_hint(format!("underlying cause: {error}"))
        })?;
        totals.files += 1;

        for route in &analysis.routes {
            canonical.push(normalize_route_declaration(
                route,
                &analysis.path,
                analysis.language,
            ));
            totals.route_declarations += 1;
        }
        for call in &analysis.http_calls {
            canonical.push(normalize_client_call(
                call,
                &analysis.path,
                analysis.language,
            ));
            totals.client_calls += 1;
        }
    }

    for route in &canonical {
        match route.status {
            NormalizationStatus::Exact => totals.exact += 1,
            NormalizationStatus::Partial => totals.partial += 1,
            NormalizationStatus::Unsupported => totals.unsupported += 1,
        }
    }

    if json {
        let report = JsonReport {
            schema_version: SCHEMA_VERSION,
            command: "normalize",
            repository: crate::pipeline::Repository::describe(path, &root),
            totals,
            canonical_routes: canonical,
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
        Ok((print_summary(&canonical, &totals), ExitCode::Success))
    }
}

fn print_summary(canonical: &[CanonicalRoute], totals: &Totals) -> String {
    let mut out = String::with_capacity(1024);
    for route in canonical {
        let side = match route.provenance.kind {
            ObservationKind::RouteDeclaration => "route ",
            ObservationKind::ClientCall => "call  ",
            // ObservationKind is non-exhaustive: a kind added later renders
            // neutrally rather than failing to compile the CLI.
            _ => "other ",
        };
        let status = match route.status {
            NormalizationStatus::Exact => "exact",
            NormalizationStatus::Partial => "partial",
            NormalizationStatus::Unsupported => "unsupported",
        };
        let _ = writeln!(
            out,
            "{side} {:<11} {}:{}",
            status, route.provenance.file, route.provenance.span
        );
        let _ = writeln!(out, "        raw       {}", route.provenance.raw_path);
        match route.status {
            NormalizationStatus::Unsupported => {
                let _ = writeln!(out, "        canonical (none — not safely normalisable)");
            }
            _ => {
                let _ = writeln!(out, "        canonical {route}");
            }
        }
        for note in &route.notes {
            let _ = writeln!(out, "        note      {note:?}");
        }
    }
    let _ = writeln!(out);
    let _ = writeln!(out, "Files                {}", totals.files);
    let _ = writeln!(out, "Route declarations   {}", totals.route_declarations);
    let _ = writeln!(out, "Client calls         {}", totals.client_calls);
    let _ = writeln!(
        out,
        "Canonicalised        {} exact, {} partial, {} unsupported",
        totals.exact, totals.partial, totals.unsupported
    );
    let _ = writeln!(out);
    let _ = writeln!(
        out,
        "Each observation is canonicalised on its own. Client calls are NOT"
    );
    let _ = writeln!(
        out,
        "matched against route declarations — that is milestone M04."
    );

    out
}
