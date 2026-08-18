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

use anyhow::{Context, Result};
use cartograph_parser::Analyzer;
use cartograph_parser::model::FileAnalysis;
use cartograph_resolver::{
    CanonicalRoute, NormalizationStatus, ObservationKind, normalize_client_call,
    normalize_route_declaration,
};
use serde::Serialize;

use crate::discovery;

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
#[derive(Debug, Serialize)]
struct JsonReport {
    root: String,
    totals: Totals,
    /// Every canonicalised observation, each independent of the others.
    canonical_routes: Vec<CanonicalRoute>,
}

/// Canonicalises every observation under `path`.
pub fn run(path: &Path, json: bool) -> Result<()> {
    let (root, files) = discovery::discover(path)?;
    let mut analyzer = Analyzer::new().context("loading grammars")?;

    let mut canonical = Vec::new();
    let mut totals = Totals::default();

    for rel in &files {
        let analysis: FileAnalysis = analyzer
            .analyze_file(&root, rel)
            .with_context(|| format!("analysing {rel}"))?;
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
            root: path.display().to_string(),
            totals,
            canonical_routes: canonical,
        };
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else {
        print_summary(&canonical, &totals);
    }
    Ok(())
}

fn print_summary(canonical: &[CanonicalRoute], totals: &Totals) {
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
        println!(
            "{side} {:<11} {}:{}",
            status, route.provenance.file, route.provenance.span
        );
        println!("        raw       {}", route.provenance.raw_path);
        match route.status {
            NormalizationStatus::Unsupported => {
                println!("        canonical (none — not safely normalisable)");
            }
            _ => println!("        canonical {route}"),
        }
        for note in &route.notes {
            println!("        note      {note:?}");
        }
    }
    println!();
    println!("Files                {}", totals.files);
    println!("Route declarations   {}", totals.route_declarations);
    println!("Client calls         {}", totals.client_calls);
    println!(
        "Canonicalised        {} exact, {} partial, {} unsupported",
        totals.exact, totals.partial, totals.unsupported
    );
    println!();
    println!("Each observation is canonicalised on its own. Client calls are NOT");
    println!("matched against route declarations — that is milestone M04.");
}
