//! `cartograph blast` — what depends on one artefact.
//!
//! # This file is an adapter
//!
//! Every semantic decision belongs to
//! [`cartograph_graph::blast::blast_radius`]: which edges count as
//! dependencies, how confidence propagates, how ties break, what order results
//! come back in. There is no traversal here, no edge filtering, and no
//! arithmetic on confidence. If any of those appear in this file, the core has
//! stopped being the oracle and two implementations can disagree.
//!
//! What this file does own is presentation: resolving a symbol a person typed
//! into a node, and rendering the result as text or JSON.
//!
//! # Symbol, never handle
//!
//! The target is named the way `trace` names it — a bare symbol, or
//! `file:name` to disambiguate — and symbol resolution is *shared with*
//! `trace` rather than reimplemented, so the two commands cannot drift about
//! what a name means.
//!
//! `NodeId` is deliberately not a command-line input. It is graph-local and
//! restarts every analysis (ADR-0011), so accepting one would invite callers
//! to persist it and make the CLI quietly depend on a cross-run identity that
//! does not exist (ADR-0014).
//!
//! # Three outcomes that are easy to conflate
//!
//! - the symbol names nothing → [`ExitCode::NotFound`];
//! - the symbol names several things → [`ExitCode::Ambiguous`], listing them,
//!   choosing none;
//! - the symbol names one thing that nothing depends on → **success with an
//!   empty result**. "Nothing depends on this" is a true answer, and reporting
//!   it as a failure would make a correct result look like an error.

use cartograph_core::NodeId;
use cartograph_graph::blast::{BlastRadius, blast_radius};
use cartograph_pipeline::error::{CliError, ErrorCode, ExitCode};
use cartograph_pipeline::pipeline::{self, Analysis};
use serde::Serialize;

use crate::json::{self, Envelope, JsonNode, Summary};
use crate::trace_cmd;

/// One artefact in the blast radius.
#[derive(Debug, Clone, Serialize)]
pub struct ReachedJson {
    /// The dependent artefact.
    pub node: JsonNode,
    /// Hops from this artefact to the target along the reported route.
    pub depth: u32,
    /// Confidence of the best-supported route, per ADR-0018.
    ///
    /// The minimum along that route, maximised over routes. **Not a
    /// probability** — see `calibrated` on the result.
    pub confidence: f32,
    /// The edge this artefact reaches the target through, by id.
    ///
    /// An id rather than a copy: the envelope's `edges` array already carries
    /// that edge's kind, confidence, provenance, evidence and location, and
    /// duplicating them here would be two records of one fact that can drift.
    pub via: u64,
}

/// What depends on the target.
#[derive(Debug, Clone, Serialize)]
pub struct BlastJson {
    /// The artefact the query was about. Never appears in `reached`.
    pub target: JsonNode,
    /// The dependents, in the core's deterministic semantic order.
    pub reached: Vec<ReachedJson>,
    /// Always `false`: confidence is an uncalibrated prior (M08), not a
    /// likelihood. Carried as data so a consumer cannot miss it.
    pub calibrated: bool,
    /// Always `"representative"`.
    ///
    /// **One route is reported per artefact — the best-supported one — not
    /// every route that exists.** Stated in the payload because a consumer
    /// reading only JSON would otherwise be entitled to assume the paths are
    /// exhaustive.
    pub routes: &'static str,
}

/// Runs a blast query.
///
/// # Errors
///
/// [`ErrorCode::SymbolNotFound`] when nothing carries the name and
/// [`ErrorCode::AmbiguousSymbol`] when several things do — both delegated to
/// the resolver `trace` uses. Analysis failures propagate from the pipeline.
pub fn run(
    path: &std::path::Path,
    symbol: &str,
    as_json: bool,
) -> Result<(String, ExitCode), CliError> {
    let analysis = pipeline::run(path, pipeline::Options { quiet: as_json })?;
    let target = trace_cmd::resolve_symbol(&analysis.graph, symbol)?;

    // The one call that matters. Everything below is presentation.
    let radius = blast_radius(&analysis.graph, target);

    let text = if as_json {
        render_json(&analysis, target, &radius)?
    } else {
        render_text(&analysis, symbol, target, &radius)
    };

    // An empty radius is a successful query. See the module documentation.
    Ok((text, ExitCode::Success))
}

/// Shapes the core result for the wire, adding nothing to it.
fn payload(analysis: &Analysis, target: NodeId, radius: &BlastRadius) -> BlastJson {
    BlastJson {
        target: trace_cmd::node_json(&analysis.graph, target),
        reached: radius
            .reached
            .iter()
            .map(|entry| ReachedJson {
                node: trace_cmd::node_json(&analysis.graph, entry.node),
                depth: entry.depth,
                // Copied, never recomputed or rounded.
                confidence: entry.confidence,
                via: entry.via.as_u64(),
            })
            .collect(),
        calibrated: false,
        routes: "representative",
    }
}

fn render_json(
    analysis: &Analysis,
    target: NodeId,
    radius: &BlastRadius,
) -> Result<String, CliError> {
    let result = payload(analysis, target, radius);
    let envelope = Envelope {
        schema_version: json::SCHEMA_VERSION,
        command: "blast",
        repository: &analysis.repository,
        languages: &analysis.languages,
        summary: Summary::from_totals(&analysis.totals, analysis.elapsed),
        nodes: json::nodes(&analysis.graph),
        edges: json::edges(&analysis.graph),
        diagnostics: &analysis.diagnostics,
        errors: Vec::new(),
        result: Some(&result),
    };

    let mut text = serde_json::to_string_pretty(&envelope).map_err(|error| {
        CliError::new(
            ErrorCode::AnalysisFailed,
            "the blast radius could not be serialised as JSON",
        )
        .with_hint(format!("underlying cause: {error}"))
    })?;
    text.push('\n');
    Ok(text)
}

/// The human form.
///
/// Ordered by the core, not re-sorted here: a reader comparing the text and
/// the JSON must see the same sequence.
fn render_text(analysis: &Analysis, query: &str, target: NodeId, radius: &BlastRadius) -> String {
    use std::fmt::Write as _;

    let graph = &analysis.graph;
    let mut out = String::new();
    let described = graph.node(target).map_or_else(
        || query.to_owned(),
        |node| {
            let location = node
                .location()
                .map_or_else(String::new, |l| format!(" {}:{}", l.file(), l.line()));
            format!("{:?} {}{}", node.kind(), node.name(), location)
        },
    );

    let _ = writeln!(out, "blast radius of {described}");

    if radius.is_empty() {
        let _ = writeln!(out, "\n  nothing depends on it");
        let _ = writeln!(
            out,
            "\n  that is an answer, not a failure: no relationship in this analysis\n  \
             reaches it."
        );
        return out;
    }

    let _ = writeln!(
        out,
        "\n  {} artefact{} depend{} on it, up to {} hop{} away",
        radius.len(),
        if radius.len() == 1 { "" } else { "s" },
        if radius.len() == 1 { "s" } else { "" },
        radius.max_depth(),
        if radius.max_depth() == 1 { "" } else { "s" }
    );
    let _ = writeln!(
        out,
        "\n  {:>5}  {:>5}  {:<10} {:<34} reached through",
        "depth", "conf", "kind", "artefact"
    );

    for entry in &radius.reached {
        let node = graph.node(entry.node);
        let (kind, name) = node.map_or_else(
            || ("?".to_owned(), "<unknown>".to_owned()),
            |n| (format!("{:?}", n.kind()), n.name().to_owned()),
        );
        let via = graph.edge(entry.via).map_or_else(
            || "-".to_owned(),
            |e| {
                format!(
                    "{:?} {}:{}",
                    e.kind(),
                    e.location().file(),
                    e.location().line()
                )
            },
        );
        let _ = writeln!(
            out,
            "  {:>5}  {:>5.3}  {:<10} {:<34} {}",
            entry.depth,
            entry.confidence,
            kind,
            truncate(&name, 34),
            via
        );
    }

    // Both caveats are stated because both are ways a reader could over-read
    // this table.
    let _ = writeln!(
        out,
        "\n  confidence is an uncalibrated prior, not a probability, and is the\n  \
         weakest step of the route shown"
    );
    let _ = writeln!(
        out,
        "  one representative route is shown per artefact; others may exist"
    );
    out
}

/// Trims a name to fit the column without pretending it was shorter.
fn truncate(value: &str, width: usize) -> String {
    if value.chars().count() <= width {
        return value.to_owned();
    }
    let kept: String = value.chars().take(width.saturating_sub(1)).collect();
    format!("{kept}…")
}
