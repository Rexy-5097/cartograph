//! What the analysis decided, beyond the drawing.
//!
//! **Experimental (M18 browser explorer; proposed ADR-0024).** The scene
//! carries what the renderer draws; this carries what an overview and an
//! "unresolved" view need: the pipeline's counts, every request-matching
//! decision including the refusals, and the per-file diagnostics.
//!
//! It is shaping only. Every value is copied from
//! [`cartograph_pipeline::pipeline::Analysis`]; nothing here decides whether a
//! request matches a route or why it did not (RULE 002, ADR-0001). The field
//! names follow `cartograph match --json` so the two can be compared.

use serde::Serialize;

use cartograph_pipeline::pipeline::{Analysis, FileDiagnostic, Languages, Totals};
use cartograph_resolver::{MatchResult, MatchStatus};

use crate::evidence::AnalysisId;

/// One request-matching decision, as `cartograph match --json` states it.
///
/// Paths are repository-relative, as the parser recorded them.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RequestDecision {
    /// The file holding the request.
    pub client_file: String,
    /// Its line.
    pub client_line: u32,
    /// The request as normalised: method and path, unknown parts marked.
    pub client_route: String,
    /// `exact`, `strong`, `ambiguous`, `no-match` or `unsupported`.
    pub status: &'static str,
    /// Why an unsupported request could not be evaluated.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unsupported_reason: Option<String>,
    /// The routes considered.
    pub candidates: Vec<RouteCandidate>,
}

/// A route a request was compared with.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RouteCandidate {
    /// The route, normalised.
    pub route: String,
    /// Where it is declared.
    pub file: String,
    /// Its line.
    pub line: u32,
    /// The handler reference the declaration names, when it names one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub handler: Option<String>,
    /// The match's confidence: an uncalibrated prior, not a probability.
    pub confidence: f32,
    /// How the methods compared.
    pub method_compatibility: String,
    /// How the paths compared.
    pub path_compatibility: String,
    /// The reasons recorded for this comparison.
    pub reasons: Vec<String>,
}

/// Everything an overview or an "unresolved" view needs, for one analysis.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AnalysisReport {
    /// The analysis these figures belong to.
    pub analysis: AnalysisId,
    /// The pipeline's counts, unchanged.
    pub totals: Totals,
    /// Files per language.
    pub languages: Languages,
    /// Every request-matching decision, refusals included, in pipeline order.
    pub decisions: Vec<RequestDecision>,
    /// Model names the ORM analysis could not resolve to one declaration.
    pub orm_ambiguous: Vec<String>,
    /// Parse problems, in file order.
    pub diagnostics: Vec<FileDiagnostic>,
    /// Wall-clock milliseconds the pipeline took.
    pub elapsed_ms: u128,
}

/// Shapes the report from a finished analysis.
#[must_use]
pub fn compose(analysis: AnalysisId, from: &Analysis) -> AnalysisReport {
    AnalysisReport {
        analysis,
        totals: from.totals.clone(),
        languages: from.languages.clone(),
        decisions: from.results.iter().map(decision).collect(),
        orm_ambiguous: from.orm.ambiguous.clone(),
        diagnostics: from.diagnostics.clone(),
        elapsed_ms: from.elapsed.as_millis(),
    }
}

fn decision(result: &MatchResult) -> RequestDecision {
    RequestDecision {
        client_file: result.client.provenance.file.clone(),
        client_line: result.client.provenance.span.start_line,
        client_route: result.client.to_string(),
        status: status_slug(result.status),
        unsupported_reason: result.unsupported_reason.map(|r| format!("{r:?}")),
        candidates: result
            .candidates
            .iter()
            .map(|c| RouteCandidate {
                route: c.route.to_string(),
                file: c.route.provenance.file.clone(),
                line: c.route.provenance.span.start_line,
                handler: c.route.provenance.handler.clone(),
                confidence: c.confidence.get(),
                method_compatibility: format!("{:?}", c.method),
                path_compatibility: format!("{:?}", c.path),
                reasons: c.reasons.iter().map(|r| format!("{r:?}")).collect(),
            })
            .collect(),
    }
}

/// The same slugs `cartograph match --json` uses.
#[allow(clippy::match_wildcard_for_single_variants)]
const fn status_slug(status: MatchStatus) -> &'static str {
    match status {
        MatchStatus::Exact => "exact",
        MatchStatus::Strong => "strong",
        MatchStatus::Ambiguous => "ambiguous",
        MatchStatus::NoMatch => "no-match",
        MatchStatus::Unsupported => "unsupported",
        _ => "other",
    }
}
