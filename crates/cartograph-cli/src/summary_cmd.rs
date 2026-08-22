//! `cartograph <path>`: the first thing a developer runs.
//!
//! # What this command is for
//!
//! Answering "can Cartograph analyse this repository, and what did it find?"
//! in one screen. It is the first impression of the product, so what it must
//! not do is overclaim.
//!
//! # The vocabulary, and why it is chosen
//!
//! The summary never says "verified". It distinguishes:
//!
//! - **observed** — the source says this. A route declaration, an HTTP-looking
//!   call site. A syntactic fact, not a relationship.
//! - **resolved** — the resolver determined this from the observations. A
//!   model that resolved to a table; a call that matched a route.
//! - **refused** — the resolver declined to claim a relationship, because the
//!   candidates were ambiguous, absent, or outside the supported subset.
//!
//! Those three are reported side by side, at the same weight. A tool that
//! prints its successes prominently and its refusals in small type is telling
//! the user something false about its own reliability, and Cartograph's whole
//! argument is that a refusal is a good outcome.

use std::collections::BTreeMap;
use std::fmt::Write as _;

use cartograph_core::{EdgeKind, Provenance};

use crate::error::{CliError, ErrorCode, ExitCode};
use crate::json::{self, Envelope, Summary};
use crate::pipeline::{self, Analysis};
use crate::version::VersionInfo;

/// How many diagnostics are listed before the rest are summarised as a count.
const DIAGNOSTIC_PREVIEW: usize = 5;

/// Runs the pipeline over `path` and renders the summary.
///
/// # Errors
///
/// Whatever the pipeline could not do; see [`pipeline::run`].
pub fn run(
    path: &std::path::Path,
    as_json: bool,
    strict: bool,
) -> Result<(String, ExitCode), CliError> {
    let analysis = pipeline::run(path, pipeline::Options { quiet: as_json })?;

    // A partially-parsed repository is a normal outcome, not a failure: real
    // trees contain files no parser handles. It becomes an error only when the
    // caller asked for that with `--strict` — and even then the analysis is
    // still reported in full, because the results found are still results.
    let partial = strict && analysis.is_partial();
    let complaint = partial.then(|| {
        CliError::new(
            ErrorCode::PartialAnalysis,
            format!(
                "{} file(s) could not be parsed, and --strict treats that as a failure",
                analysis.totals.files_failed
            ),
        )
        .with_hint("run `cartograph parse` on the repository to see which files, or drop --strict")
    });

    let text = if as_json {
        render_json(&analysis, complaint.as_ref())?
    } else {
        render_text(&analysis)
    };

    if let Some(complaint) = &complaint {
        if !as_json {
            // stdout keeps the result; the reason for the exit status goes to
            // stderr so a pipe still receives a clean summary.
            crate::output::emit_error(&complaint.to_string());
        }
        return Ok((text, ExitCode::Partial));
    }

    Ok((text, ExitCode::Success))
}

fn render_json(analysis: &Analysis, complaint: Option<&CliError>) -> Result<String, CliError> {
    let envelope = Envelope::<()> {
        schema_version: json::SCHEMA_VERSION,
        command: "summary",
        repository: &analysis.repository,
        languages: &analysis.languages,
        summary: Summary::from_totals(&analysis.totals, analysis.elapsed),
        nodes: json::nodes(&analysis.graph),
        edges: json::edges(&analysis.graph),
        diagnostics: &analysis.diagnostics,
        // Present but empty on success, so a consumer checks one field
        // unconditionally rather than testing for the key.
        errors: complaint.into_iter().collect(),
        result: None,
    };
    serialise(&envelope)
}

fn serialise<T: serde::Serialize>(value: &T) -> Result<String, CliError> {
    let mut text = serde_json::to_string_pretty(value).map_err(|error| {
        CliError::new(
            ErrorCode::AnalysisFailed,
            "the result could not be serialised as JSON",
        )
        .with_hint(format!("underlying cause: {error}"))
    })?;
    text.push('\n');
    Ok(text)
}

#[allow(clippy::too_many_lines)] // One report; splitting it would hide its shape.
fn render_text(analysis: &Analysis) -> String {
    let totals = &analysis.totals;
    let mut out = String::with_capacity(2048);

    let version = VersionInfo::current();
    let _ = writeln!(
        out,
        "Cartograph {} — {}",
        version.version, analysis.repository.name
    );
    let _ = writeln!(out);

    let languages = analysis.languages.present();
    let languages = if languages.is_empty() {
        "none".to_owned()
    } else {
        languages.join(", ")
    };
    let _ = writeln!(out, "  path            {}", analysis.repository.requested);
    let _ = writeln!(out, "  languages       {languages}");
    let _ = writeln!(out, "  files analysed  {}", totals.files);
    let _ = writeln!(out, "  symbols         {}", totals.symbols);
    let _ = writeln!(out, "  duration        {:.2?}", analysis.elapsed);
    let _ = writeln!(out);

    // ── Observed ────────────────────────────────────────────────────
    let _ = writeln!(out, "OBSERVED — what the source says");
    let _ = writeln!(
        out,
        "  routes                {:>7}   route declarations found in source",
        totals.routes
    );
    let _ = writeln!(
        out,
        "  HTTP call sites       {:>7}   syntactically HTTP-looking calls",
        totals.http_observations
    );
    let _ = writeln!(
        out,
        "  ORM models            {:>7}   classes recognised as ORM models",
        totals.orm_models
    );
    let _ = writeln!(out);

    // ── Resolved ────────────────────────────────────────────────────
    let _ = writeln!(out, "RESOLVED — what the resolver determined");
    let _ = writeln!(
        out,
        "  database tables       {:>7}   models that resolved to a table name",
        totals.orm_tables
    );
    let _ = writeln!(
        out,
        "  graph                 {:>7}   nodes, {} edges",
        totals.nodes, totals.edges
    );
    let _ = writeln!(
        out,
        "  cross-language        {:>7}   edges whose endpoints are different languages",
        totals.cross_language_edges
    );

    let by_kind = count_by_kind(analysis);
    if !by_kind.is_empty() {
        let _ = writeln!(out);
        let _ = writeln!(out, "  by relationship");
        for (kind, count) in &by_kind {
            let _ = writeln!(out, "    {:<20} {count:>7}", label_kind(*kind));
        }
    }

    let by_provenance = count_by_provenance(analysis);
    if !by_provenance.is_empty() {
        let _ = writeln!(out);
        let _ = writeln!(out, "  by provenance");
        for (provenance, count) in &by_provenance {
            let _ = writeln!(
                out,
                "    {:<20} {count:>7}   {}",
                label_provenance(*provenance),
                if provenance.is_deterministic() {
                    "computed"
                } else {
                    "estimated"
                }
            );
        }
    }

    let by_confidence = count_by_confidence(analysis);
    if !by_confidence.is_empty() {
        let _ = writeln!(out);
        let _ = writeln!(out, "  by confidence (uncalibrated priors)");
        for (value, count) in by_confidence.iter().rev() {
            let _ = writeln!(out, "    {:<20} {count:>7}", format_confidence(*value));
        }
    }
    let _ = writeln!(out);

    // ── Refused ─────────────────────────────────────────────────────
    let _ = writeln!(out, "REFUSED — what the resolver declined to claim");
    let _ = writeln!(
        out,
        "  ambiguous             {:>7}   rival candidates, no edge produced",
        totals.ambiguous
    );
    let _ = writeln!(
        out,
        "  no match              {:>7}   no candidate route",
        totals.no_match
    );
    let _ = writeln!(
        out,
        "  unsupported           {:>7}   outside the supported subset",
        totals.unsupported
    );
    let _ = writeln!(out);

    // ── Diagnostics ─────────────────────────────────────────────────
    let _ = writeln!(out, "DIAGNOSTICS");
    if totals.diagnostics == 0 {
        let _ = writeln!(out, "  none — every file parsed cleanly");
    } else {
        let _ = writeln!(
            out,
            "  {} diagnostic(s) across {} file(s); {} file(s) could not be parsed",
            totals.diagnostics, totals.files_with_diagnostics, totals.files_failed
        );
        for diagnostic in analysis.diagnostics.iter().take(DIAGNOSTIC_PREVIEW) {
            match diagnostic.line {
                Some(line) => {
                    let _ = writeln!(
                        out,
                        "    {}:{}  {}",
                        diagnostic.file, line, diagnostic.message
                    );
                }
                None => {
                    let _ = writeln!(out, "    {}  {}", diagnostic.file, diagnostic.message);
                }
            }
        }
        if analysis.diagnostics.len() > DIAGNOSTIC_PREVIEW {
            let _ = writeln!(
                out,
                "    … and {} more (see `cartograph parse` for the full list)",
                analysis.diagnostics.len() - DIAGNOSTIC_PREVIEW
            );
        }
    }
    let _ = writeln!(out);

    // ── The caveat, stated rather than implied ──────────────────────
    let _ = writeln!(
        out,
        "Nothing above is claimed to be verified. Confidence values are the"
    );
    let _ = writeln!(
        out,
        "specification's uncalibrated priors, not measured probabilities;"
    );
    let _ = writeln!(
        out,
        "see docs/benchmarks/m08-confidence-policy.md. Refusals are by design."
    );
    let _ = writeln!(out);
    let _ = writeln!(out, "NEXT");
    if totals.edges > 0 {
        let _ = writeln!(
            out,
            "  cartograph trace <symbol>     follow one relationship, hop by hop"
        );
    }
    // The question a first run actually provokes is "why was so much refused?",
    // and until M09 the summary reported the count without saying where the
    // answer lives. `match` prints the reason for every single one.
    if totals.refused() > 0 {
        let _ = writeln!(
            out,
            "  cartograph match <path>       why each of the {} refusals happened",
            totals.refused()
        );
    }
    let _ = writeln!(
        out,
        "  --json                        the machine-readable graph"
    );

    out
}

fn count_by_kind(analysis: &Analysis) -> Vec<(EdgeKind, usize)> {
    let mut counts: BTreeMap<String, (EdgeKind, usize)> = BTreeMap::new();
    for edge in analysis.graph.edges() {
        let entry = counts
            .entry(label_kind(edge.kind()).to_owned())
            .or_insert((edge.kind(), 0));
        entry.1 += 1;
    }
    counts.into_values().collect()
}

fn count_by_provenance(analysis: &Analysis) -> Vec<(Provenance, usize)> {
    let mut counts: BTreeMap<String, (Provenance, usize)> = BTreeMap::new();
    for edge in analysis.graph.edges() {
        let entry = counts
            .entry(label_provenance(edge.provenance()).to_owned())
            .or_insert((edge.provenance(), 0));
        entry.1 += 1;
    }
    counts.into_values().collect()
}

/// Confidence values, keyed by their exact hundredths so a float never becomes
/// a map key.
fn count_by_confidence(analysis: &Analysis) -> Vec<(u32, usize)> {
    let mut counts: BTreeMap<u32, usize> = BTreeMap::new();
    for edge in analysis.graph.edges() {
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let key = (edge.confidence().get() * 100.0).round() as u32;
        *counts.entry(key).or_insert(0) += 1;
    }
    counts.into_iter().collect()
}

fn format_confidence(hundredths: u32) -> String {
    format!("{}.{:02}", hundredths / 100, hundredths % 100)
}

fn label_kind(kind: EdgeKind) -> &'static str {
    match kind {
        EdgeKind::Import => "import",
        EdgeKind::Call => "call",
        EdgeKind::Inherits => "inherits",
        EdgeKind::Implements => "implements",
        EdgeKind::References => "references",
        EdgeKind::HttpCall => "http-call",
        EdgeKind::OrmAccess => "orm-access",
        EdgeKind::Queries => "queries",
        // EdgeKind is non-exhaustive: a kind added later renders neutrally
        // rather than failing to compile the CLI.
        _ => "other",
    }
}

fn label_provenance(provenance: Provenance) -> &'static str {
    match provenance {
        Provenance::LspSymbolResolution => "lsp-symbol",
        Provenance::RouteMatcher => "route-matcher",
        Provenance::OpenApiSpec => "openapi-spec",
        Provenance::StaticImportResolution => "static-import",
        Provenance::ConstantPropagation => "constant-propagation",
        Provenance::TemplateEvaluation => "template-evaluation",
        Provenance::OrmResolution => "orm-resolution",
        Provenance::ModelInference => "model-inference",
        _ => "other",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn confidence_renders_as_two_decimal_places() {
        assert_eq!(format_confidence(100), "1.00");
        assert_eq!(format_confidence(98), "0.98");
        assert_eq!(format_confidence(45), "0.45");
        assert_eq!(format_confidence(80), "0.80");
    }

    #[test]
    fn every_edge_kind_has_a_label() {
        for kind in [
            EdgeKind::Import,
            EdgeKind::Call,
            EdgeKind::Inherits,
            EdgeKind::Implements,
            EdgeKind::References,
            EdgeKind::HttpCall,
            EdgeKind::OrmAccess,
            EdgeKind::Queries,
        ] {
            assert_ne!(label_kind(kind), "other", "{kind:?} has no label");
        }
    }

    #[test]
    fn every_provenance_has_a_label() {
        for provenance in [
            Provenance::LspSymbolResolution,
            Provenance::RouteMatcher,
            Provenance::OpenApiSpec,
            Provenance::StaticImportResolution,
            Provenance::ConstantPropagation,
            Provenance::TemplateEvaluation,
            Provenance::OrmResolution,
            Provenance::ModelInference,
        ] {
            assert_ne!(
                label_provenance(provenance),
                "other",
                "{provenance:?} has no label"
            );
        }
    }
}
