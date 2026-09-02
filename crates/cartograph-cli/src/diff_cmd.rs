//! `cartograph diff` — what changed architecturally between two checkouts.
//!
//! # Two paths, not two revisions
//!
//! The command takes two already-checked-out trees rather than two git
//! revisions. Cartograph has no git dependency and gains none here: resolving
//! a revision would mean either linking a git library or spawning `git`, and
//! both are a larger capability than the milestone needs. Two paths keep the
//! analyser doing what it already does — read a directory — and let the caller
//! decide how the trees came to exist (`git worktree add` is the usual answer).
//!
//! # What the document carries
//!
//! The envelope describes the **after** analysis: its repository, summary,
//! nodes and edges. A diff has two graphs and the envelope has room for one,
//! and the after side is the one a reader is looking at.
//!
//! Every diff entry therefore carries its evidence **inline** — confidence,
//! provenance, file, line and the observation itself — on both sides of a
//! change. A removed edge is not in the after graph's `edges` array by
//! definition, so an id alone could not be resolved; inline evidence is what
//! makes every reported edge explain itself rather than only those that
//! survived.
//!
//! # Exit codes
//!
//! `0` when the comparison succeeds, whether or not anything differs — an
//! empty diff is a true answer about two identical trees, not a failure.
//! Unreadable paths surface as `3` through the analyser's own error, exactly
//! as they do for every other command.

use std::path::Path;

use cartograph_core::Provenance;
use serde::Serialize;

use cartograph_graph::diff::{ChangedEdge, EdgeAppearance, GraphDiff, diff};
use cartograph_pipeline::error::{CliError, ExitCode};
use cartograph_pipeline::pipeline::{self, Analysis};

use crate::json::{self, Envelope, Summary};

/// The evidence behind one observation, as reported by the diff.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FactsJson {
    /// An uncalibrated prior selected by evidence class, not a probability.
    pub confidence: f32,
    /// Which analysis produced the edge.
    pub provenance: Provenance,
    /// Repository-relative file the claim was observed in.
    pub file: String,
    /// One-based line. Reported, but a line shift alone is not a change.
    pub line: u32,
    /// The observation supporting the edge.
    pub evidence: String,
}

/// An edge present on one side only.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppearanceJson {
    /// Stable identity of the node the relationship starts at.
    pub source: String,
    /// Stable identity of the node it points to.
    pub target: String,
    /// The relationship kind.
    pub kind: &'static str,
    /// The evidence behind it.
    pub facts: FactsJson,
}

/// An edge both sides assert, with differing evidence.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChangeJson {
    /// Stable identity of the node the relationship starts at.
    pub source: String,
    /// Stable identity of the node it points to.
    pub target: String,
    /// The relationship kind — identical on both sides, by definition.
    pub kind: &'static str,
    /// Which fields differ.
    pub fields: Vec<&'static str>,
    /// What the earlier analysis observed.
    pub before: FactsJson,
    /// What the later analysis observed.
    pub after: FactsJson,
}

/// The `diff` section of the document.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiffJson {
    /// The tree analysed as the earlier state. Never an absolute path.
    pub before: String,
    /// The tree analysed as the later state. Never an absolute path.
    pub after: String,
    /// Relationships the later analysis asserts and the earlier did not.
    pub added: Vec<AppearanceJson>,
    /// Relationships the earlier asserted and the later does not.
    pub removed: Vec<AppearanceJson>,
    /// Relationships both assert, with differing evidence.
    pub changed: Vec<ChangeJson>,
    /// Relationships both assert identically. Counted, not listed.
    pub unchanged: usize,
    /// Correspondence is by stable `(kind, name, file)` identity, never by
    /// `NodeId`. Stated on the wire so a consumer need not infer it.
    pub correspondence: &'static str,
}

fn facts(f: &cartograph_graph::diff::EdgeFacts) -> FactsJson {
    FactsJson {
        confidence: f.confidence,
        provenance: f.provenance,
        file: f.file.clone(),
        line: f.line,
        evidence: f.evidence.clone(),
    }
}

fn payload(before_name: &str, after_name: &str, result: &GraphDiff) -> DiffJson {
    DiffJson {
        before: before_name.to_owned(),
        after: after_name.to_owned(),
        added: result
            .added
            .iter()
            .map(|a| AppearanceJson {
                source: a.identity.source.as_str().to_owned(),
                target: a.identity.target.as_str().to_owned(),
                kind: a.identity.kind,
                facts: facts(&a.facts),
            })
            .collect(),
        removed: result
            .removed
            .iter()
            .map(|a| AppearanceJson {
                source: a.identity.source.as_str().to_owned(),
                target: a.identity.target.as_str().to_owned(),
                kind: a.identity.kind,
                facts: facts(&a.facts),
            })
            .collect(),
        changed: result
            .changed
            .iter()
            .map(|c| ChangeJson {
                source: c.identity.source.as_str().to_owned(),
                target: c.identity.target.as_str().to_owned(),
                kind: c.identity.kind,
                fields: c.fields.iter().map(|f| f.token()).collect(),
                before: facts(&c.before),
                after: facts(&c.after),
            })
            .collect(),
        unchanged: result.unchanged,
        correspondence: "stable-identity",
    }
}

/// Runs the comparison.
///
/// # Errors
///
/// Propagates the analyser's error for either path — an unreadable tree is an
/// input problem and exits `3`, the same as for every other command.
pub fn run(before: &Path, after: &Path, as_json: bool) -> Result<(String, ExitCode), CliError> {
    let left = pipeline::run(before, pipeline::Options { quiet: as_json })?;
    let right = pipeline::run(after, pipeline::Options { quiet: as_json })?;

    let result = diff(&left.graph, &right.graph);

    let text = if as_json {
        render_json(&left, &right, &result)?
    } else {
        render_text(&left, &right, &result)
    };
    // An empty diff is a true answer about two identical trees, so the exit
    // code says success either way. A script asking "did anything change?"
    // reads the counts, not the status.
    Ok((text, ExitCode::Success))
}

fn render_json(left: &Analysis, right: &Analysis, result: &GraphDiff) -> Result<String, CliError> {
    let section = payload(&left.repository.name, &right.repository.name, result);
    let envelope = Envelope {
        schema_version: json::SCHEMA_VERSION,
        command: "diff",
        repository: &right.repository,
        languages: &right.languages,
        summary: Summary::from_totals(&right.totals, right.elapsed),
        nodes: json::nodes(&right.graph),
        edges: json::edges(&right.graph),
        diagnostics: &right.diagnostics,
        errors: Vec::new(),
        result: Some(&section),
    };

    let mut text = serde_json::to_string_pretty(&envelope).map_err(|error| {
        CliError::new(
            cartograph_pipeline::error::ErrorCode::AnalysisFailed,
            "the diff could not be serialised as JSON",
        )
        .with_hint(format!("underlying cause: {error}"))
    })?;
    text.push('\n');
    Ok(text)
}

/// The human form.
///
/// Ordered by the diff engine, not re-sorted here: a reader comparing the text
/// and the JSON must see the same sequence.
fn render_text(left: &Analysis, right: &Analysis, result: &GraphDiff) -> String {
    use std::fmt::Write as _;

    let mut out = String::new();
    let _ = writeln!(out, "{} -> {}", left.repository.name, right.repository.name);

    if result.is_empty() {
        let _ = writeln!(
            out,
            "\nNo architectural difference: the two trees assert the same relationships."
        );
        let _ = writeln!(out, "{} relationships unchanged.", result.unchanged);
        return out;
    }

    let _ = writeln!(
        out,
        "\n{} added, {} removed, {} changed; {} unchanged.",
        result.added.len(),
        result.removed.len(),
        result.changed.len(),
        result.unchanged
    );

    section(&mut out, "ADDED", '+', &result.added);
    section(&mut out, "REMOVED", '-', &result.removed);
    changes(&mut out, &result.changed);

    let _ = writeln!(
        out,
        "\nConfidence is an uncalibrated prior selected by evidence class, not a probability."
    );
    out
}

/// One side-only section of the text form.
///
/// Added and removed differ only in their heading and marker, so they share a
/// renderer rather than two copies that could drift apart.
fn section(out: &mut String, heading: &str, marker: char, entries: &[EdgeAppearance]) {
    use std::fmt::Write as _;

    if entries.is_empty() {
        return;
    }
    let _ = writeln!(out, "\n{heading}");
    for entry in entries {
        let _ = writeln!(
            out,
            "  {marker} {} -> {} [{}]  {:.2} {}  {}:{}",
            short(&entry.identity.source),
            short(&entry.identity.target),
            entry.identity.kind,
            entry.facts.confidence,
            provenance_token(entry.facts.provenance),
            entry.facts.file,
            entry.facts.line
        );
    }
}

/// The changed section, which shows both observations so a reader can see what
/// moved rather than being told only that something did.
fn changes(out: &mut String, entries: &[ChangedEdge]) {
    use std::fmt::Write as _;

    if entries.is_empty() {
        return;
    }
    let _ = writeln!(out, "\nCHANGED");
    for c in entries {
        let fields: Vec<&str> = c.fields.iter().map(|f| f.token()).collect();
        let _ = writeln!(
            out,
            "  ~ {} -> {} [{}]  {}",
            short(&c.identity.source),
            short(&c.identity.target),
            c.identity.kind,
            fields.join(", ")
        );
        for (label, facts) in [("before", &c.before), ("after ", &c.after)] {
            let _ = writeln!(
                out,
                "      {label} {:.2} {}  {}:{}",
                facts.confidence,
                provenance_token(facts.provenance),
                facts.file,
                facts.line
            );
        }
    }
}

/// The readable part of an identity, for the text form.
///
/// The canonical encoding is exact but noisy to read; the name and file are
/// what a person scanning a diff is looking for. The JSON keeps the full
/// identity, so nothing is lost.
fn short(identity: &cartograph_core::NodeIdentity) -> String {
    let raw = identity.as_str();
    let parts: Vec<&str> = raw.split('|').collect();
    match parts.as_slice() {
        [kind, name, "1", file] => format!("{name} ({kind} in {file})"),
        [kind, name, ..] => format!("{name} ({kind})"),
        _ => raw.to_owned(),
    }
}

fn provenance_token(provenance: Provenance) -> &'static str {
    match provenance {
        Provenance::LspSymbolResolution => "lsp",
        Provenance::RouteMatcher => "route-matcher",
        Provenance::OpenApiSpec => "openapi",
        Provenance::StaticImportResolution => "static-import",
        Provenance::ConstantPropagation => "constant-propagation",
        Provenance::TemplateEvaluation => "template-evaluation",
        Provenance::OrmResolution => "orm",
        Provenance::ModelInference => "model-inference",
        // `Provenance` is non-exhaustive, so a new variant must not stop the
        // diff rendering. Naming it unknown is honest; guessing would not be.
        _ => "unknown",
    }
}
