//! Evidence: the observation that supports an edge.

use serde::{Deserialize, Serialize};

use crate::error::CoreError;

/// The concrete observation behind an edge.
///
/// This is what a user sees when they click an edge and ask "why do you think
/// these two things are connected?" — the resolved URL, the matched route, the
/// ORM model name. In the specification's worked example the evidence for a
/// `HttpCall` edge is the string `POST /api/orders`.
///
/// # Why this is a string today
///
/// A structured evidence taxonomy (matched route, resolved template, unknown
/// segments, feature vector) is clearly where this ends up. Designing that
/// taxonomy before the resolver exists would be guessing at the shape of data
/// that has not been produced yet. The type is introduced now so that every
/// edge is *required* to carry evidence from the first commit; the internals
/// become structured at M04, when there is a real resolver to describe.
///
/// Evidence must never contain source-file text, environment variable values
/// or credentials (RULE 015). It describes a claim; it does not quote the file.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct Evidence(String);

impl Evidence {
    /// Constructs evidence from a non-empty summary.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::Empty`] if the summary is empty or only
    /// whitespace. An edge with blank evidence is indistinguishable from an
    /// edge with none, which is the state this type exists to prevent.
    pub fn new(summary: impl Into<String>) -> Result<Self, CoreError> {
        let summary = summary.into();
        if summary.trim().is_empty() {
            return Err(CoreError::Empty { field: "evidence" });
        }
        Ok(Self(summary))
    }

    /// The summary text.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for Evidence {
    type Error = CoreError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl From<Evidence> for String {
    fn from(value: Evidence) -> Self {
        value.0
    }
}

impl std::fmt::Display for Evidence {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn carries_the_observation_verbatim() {
        assert_eq!(
            Evidence::new("POST /api/orders").unwrap().as_str(),
            "POST /api/orders"
        );
    }

    #[test]
    fn rejects_blank_evidence() {
        assert!(Evidence::new("").is_err());
        assert!(Evidence::new("   \t\n").is_err());
    }

    #[test]
    fn deserialising_rejects_blank_evidence_too() {
        assert!(serde_json::from_str::<Evidence>(r#""""#).is_err());
    }
}
