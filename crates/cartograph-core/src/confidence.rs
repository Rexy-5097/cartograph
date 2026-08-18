//! Confidence: how much an edge should be trusted.

use serde::{Deserialize, Serialize};

use crate::error::CoreError;

/// A probability, in `[0.0, 1.0]`, that an edge is correct.
///
/// # Calibration status
///
/// The associated constants below are the **starting priors** from the frozen
/// specification. They are not calibrated. An edge marked `0.8` is not yet
/// known to be correct 80% of the time — establishing that is the entire point
/// of milestone M08, which fits these values against a labelled benchmark and
/// publishes a reliability diagram and expected calibration error.
///
/// Until M08 lands, no accuracy claim may be made on the basis of these
/// numbers. See `docs/benchmarks/README.md`.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(try_from = "f32", into = "f32")]
pub struct Confidence(f32);

impl Confidence {
    /// A symbol resolved exactly by a language server. `1.00`.
    pub const LSP_EXACT_SYMBOL: Self = Self(1.00);
    /// Canonical path and HTTP method both matched exactly. `0.98`.
    pub const ROUTE_METHOD_EXACT: Self = Self(0.98);
    /// Confirmed against an `OpenAPI` specification, which is ground truth. `0.98`.
    pub const OPENAPI_CONFIRMED: Self = Self(0.98);
    /// An import statically resolved to a file. `0.90`.
    pub const STATIC_IMPORT: Self = Self(0.90);
    /// A URL recovered by propagating module-scope constants. `0.80`.
    pub const CONSTANT_PROPAGATION: Self = Self(0.80);
    /// A template literal only partially evaluated; some segments unknown. `0.65`.
    pub const PARTIAL_TEMPLATE_MATCH: Self = Self(0.65);
    /// Proposed by the calibrated statistical model. `0.45`.
    ///
    /// "Model" here means the logistic-regression edge classifier fitted at
    /// M08 — never a language model. See [`crate::edge::Edge`] and RULE 007.
    pub const MODEL_INFERENCE: Self = Self(0.45);

    /// Constructs a confidence, rejecting values outside `[0.0, 1.0]` and
    /// non-finite values.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::ConfidenceOutOfRange`] if `value` is `NaN`,
    /// infinite, negative, or greater than one.
    pub fn new(value: f32) -> Result<Self, CoreError> {
        if !value.is_finite() || !(0.0..=1.0).contains(&value) {
            return Err(CoreError::ConfidenceOutOfRange {
                value: value.to_string(),
            });
        }
        Ok(Self(value))
    }

    /// The underlying probability.
    #[must_use]
    pub fn get(self) -> f32 {
        self.0
    }
}

impl TryFrom<f32> for Confidence {
    type Error = CoreError;

    fn try_from(value: f32) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl From<Confidence> for f32 {
    fn from(value: Confidence) -> Self {
        value.0
    }
}

impl std::fmt::Display for Confidence {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:.2}", self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Confidences are compared within a tolerance rather than exactly:
    /// they arrive from calibration arithmetic, and bit-exact equality is not
    /// a property this type promises.
    fn assert_close(actual: Confidence, expected: f32) {
        assert!(
            (actual.get() - expected).abs() < f32::EPSILON,
            "expected {expected}, got {actual}"
        );
    }

    #[test]
    fn accepts_the_unit_interval() {
        assert_close(Confidence::new(0.0).unwrap(), 0.0);
        assert_close(Confidence::new(1.0).unwrap(), 1.0);
        assert_close(Confidence::new(0.94).unwrap(), 0.94);
    }

    #[test]
    fn rejects_values_outside_the_unit_interval() {
        assert!(Confidence::new(-0.01).is_err());
        assert!(Confidence::new(1.01).is_err());
    }

    #[test]
    fn rejects_non_finite_values() {
        assert!(Confidence::new(f32::NAN).is_err());
        assert!(Confidence::new(f32::INFINITY).is_err());
        assert!(Confidence::new(f32::NEG_INFINITY).is_err());
    }

    #[test]
    fn priors_are_ordered_from_deterministic_to_inferred() {
        // The confidence ladder from the frozen specification, in order.
        // If a future change reorders these, the ladder in the interface and
        // the published calibration would disagree with the code.
        let ladder = [
            Confidence::LSP_EXACT_SYMBOL,
            Confidence::ROUTE_METHOD_EXACT,
            Confidence::STATIC_IMPORT,
            Confidence::CONSTANT_PROPAGATION,
            Confidence::PARTIAL_TEMPLATE_MATCH,
            Confidence::MODEL_INFERENCE,
        ];
        for pair in ladder.windows(2) {
            assert!(
                pair[0].get() > pair[1].get(),
                "confidence ladder is out of order: {} then {}",
                pair[0],
                pair[1]
            );
        }
        assert_eq!(
            Confidence::OPENAPI_CONFIRMED,
            Confidence::ROUTE_METHOD_EXACT,
            "OpenAPI confirmation and exact route match share a prior"
        );
    }

    #[test]
    fn round_trips_through_json_and_validates_on_the_way_back() {
        let json = serde_json::to_string(&Confidence::new(0.94).unwrap()).unwrap();
        assert_eq!(json, "0.94");
        let parsed: Confidence = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, Confidence::new(0.94).unwrap());

        // A hand-edited or corrupted graph file must not load an impossible
        // probability into memory.
        assert!(serde_json::from_str::<Confidence>("1.5").is_err());
    }

    #[test]
    fn displays_with_two_decimals_like_the_cli_trace_output() {
        assert_eq!(Confidence::new(0.94).unwrap().to_string(), "0.94");
        assert_eq!(Confidence::LSP_EXACT_SYMBOL.to_string(), "1.00");
    }
}
