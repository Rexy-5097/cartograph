//! Provenance: which analysis produced an edge.

use serde::{Deserialize, Serialize};

use crate::confidence::Confidence;

/// The analysis that produced an edge.
///
/// Provenance answers "which resolver fired?" and is the field the interface
/// shows when a user clicks an edge. It is also what decides whether an edge is
/// rendered solid (derived) or dashed (inferred) — that distinction belongs
/// here rather than to a threshold on [`Confidence`], because the *method*
/// determines whether a claim was computed or estimated, not the number.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
#[non_exhaustive]
pub enum Provenance {
    /// A language server resolved the symbol exactly (tsserver, Pyright).
    LspSymbolResolution,
    /// A frontend call site and a backend route agreed on canonical path and
    /// HTTP method.
    RouteMatcher,
    /// An `OpenAPI` document confirmed the route. Ground truth; inference is
    /// skipped when a specification is available.
    ///
    /// Renamed explicitly: `rename_all = "kebab-case"` would split this into
    /// `open-api-spec`, which is not how the specification spells it.
    #[serde(rename = "openapi-spec")]
    OpenApiSpec,
    /// An import statement resolved statically to a file.
    StaticImportResolution,
    /// A URL recovered by propagating module-scope constants.
    ConstantPropagation,
    /// A URL recovered by partially evaluating a template literal. Unknown
    /// segments are preserved as unknown; they are never guessed.
    TemplateEvaluation,
    /// An ORM model resolved to a database table.
    OrmResolution,
    /// Proposed by the calibrated statistical edge classifier fitted at M08.
    ///
    /// This is a logistic regression over structural features (path match,
    /// method match, handler-name similarity, git co-change, and so on). It is
    /// **not** a language model. No language model may propose, construct or
    /// modify graph structure — see `PROJECT_RULES.md` RULE 007 and
    /// `docs/adr/ADR-0007-no-llm-graph-construction.md`.
    ModelInference,
}

impl Provenance {
    /// Whether this analysis *computes* a relationship rather than estimating
    /// one.
    ///
    /// Deterministic edges are rendered solid; inferred edges are rendered
    /// dashed and are the population the calibration work at M08 measures.
    #[must_use]
    pub fn is_deterministic(self) -> bool {
        match self {
            Self::LspSymbolResolution
            | Self::RouteMatcher
            | Self::OpenApiSpec
            | Self::StaticImportResolution => true,
            Self::ConstantPropagation
            | Self::TemplateEvaluation
            | Self::OrmResolution
            | Self::ModelInference => false,
        }
    }

    /// The uncalibrated starting prior for this analysis.
    ///
    /// These are the specification's initial values, not measured frequencies.
    /// A resolver may supply a lower confidence than the prior when it knows
    /// its own result is weak — for example a template evaluation that left
    /// segments unknown. It should not supply a higher one.
    #[must_use]
    pub fn prior_confidence(self) -> Confidence {
        match self {
            Self::LspSymbolResolution => Confidence::LSP_EXACT_SYMBOL,
            Self::RouteMatcher => Confidence::ROUTE_METHOD_EXACT,
            Self::OpenApiSpec => Confidence::OPENAPI_CONFIRMED,
            Self::StaticImportResolution => Confidence::STATIC_IMPORT,
            Self::ConstantPropagation | Self::OrmResolution => Confidence::CONSTANT_PROPAGATION,
            Self::TemplateEvaluation => Confidence::PARTIAL_TEMPLATE_MATCH,
            Self::ModelInference => Confidence::MODEL_INFERENCE,
        }
    }
}

impl std::fmt::Display for Provenance {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let name = match self {
            Self::LspSymbolResolution => "lsp-symbol-resolution",
            Self::RouteMatcher => "route-matcher",
            Self::OpenApiSpec => "openapi-spec",
            Self::StaticImportResolution => "static-import-resolution",
            Self::ConstantPropagation => "constant-propagation",
            Self::TemplateEvaluation => "template-evaluation",
            Self::OrmResolution => "orm-resolution",
            Self::ModelInference => "model-inference",
        };
        f.write_str(name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ALL: [Provenance; 8] = [
        Provenance::LspSymbolResolution,
        Provenance::RouteMatcher,
        Provenance::OpenApiSpec,
        Provenance::StaticImportResolution,
        Provenance::ConstantPropagation,
        Provenance::TemplateEvaluation,
        Provenance::OrmResolution,
        Provenance::ModelInference,
    ];

    #[test]
    fn deterministic_analyses_outrank_inferred_ones() {
        let weakest_deterministic = ALL
            .iter()
            .filter(|p| p.is_deterministic())
            .map(|p| p.prior_confidence().get())
            .fold(f32::INFINITY, f32::min);
        let strongest_inferred = ALL
            .iter()
            .filter(|p| !p.is_deterministic())
            .map(|p| p.prior_confidence().get())
            .fold(0.0_f32, f32::max);

        assert!(
            weakest_deterministic > strongest_inferred,
            "an inferred analysis claims a prior at or above a deterministic one"
        );
    }

    #[test]
    fn model_inference_is_the_least_trusted_analysis() {
        for p in ALL {
            assert!(
                p.prior_confidence().get() >= Provenance::ModelInference.prior_confidence().get(),
                "{p} ranks below model inference"
            );
        }
    }

    #[test]
    fn display_and_serde_agree_on_the_wire_name() {
        for p in ALL {
            let json = serde_json::to_string(&p).unwrap();
            assert_eq!(
                json,
                format!("\"{p}\""),
                "Display and serde disagree for {p:?}; the CLI trace output and \
                 the JSON output would name the same resolver differently"
            );
        }
    }
}
