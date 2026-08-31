//! Identifiers for nodes, edges and commits.

use serde::{Deserialize, Serialize};

use crate::error::CoreError;

/// Handle to a node in an architecture graph.
///
/// Opaque and assigned by the graph that owns the node. Identity is *not* yet
/// content-addressed: two analyses of the same repository may assign different
/// `NodeId`s to the same function.
///
/// M10 built its incremental engine without it — per-file results are keyed by
/// content and dependency identity, never by `NodeId`, and graphs are compared
/// by semantic identity rather than by handle — so stable identity is deferred
/// again, with its consumers named: M12 (blast radius) and M13 (structural
/// diff) both require it. See ADR-0014.
///
/// # Handles are graph-local
///
/// Ids are allocated per graph and start from zero, so the same `NodeId` names
/// different artefacts in two different graphs. Passing a handle from one
/// graph to another does not fail loudly — it silently addresses whatever
/// happens to occupy that slot. Until identity becomes content-addressed, do
/// not mix handles across graphs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct NodeId(u64);

impl NodeId {
    /// Wraps a raw identifier.
    ///
    /// Callers outside the graph should not need this; it exists so graphs can
    /// be reconstructed from storage.
    #[must_use]
    pub const fn from_raw(raw: u64) -> Self {
        Self(raw)
    }

    /// The raw identifier.
    #[must_use]
    pub const fn as_u64(self) -> u64 {
        self.0
    }
}

impl std::fmt::Display for NodeId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "n{}", self.0)
    }
}

/// Handle to an edge in an architecture graph.
///
/// See [`NodeId`] for the identity caveat; it applies equally here.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct EdgeId(u64);

impl EdgeId {
    /// Wraps a raw identifier.
    #[must_use]
    pub const fn from_raw(raw: u64) -> Self {
        Self(raw)
    }

    /// The raw identifier.
    #[must_use]
    pub const fn as_u64(self) -> u64 {
        self.0
    }
}

impl std::fmt::Display for EdgeId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "e{}", self.0)
    }
}

/// A Git object identifier, recording which revision an edge was derived from.
///
/// Stored as text rather than bytes because it is round-tripped through JSON
/// and shown to users verbatim. Validated on construction so a malformed value
/// cannot reach the interface claiming to be a commit.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct CommitId(String);

impl CommitId {
    /// Constructs a commit id from 7 to 64 hexadecimal characters.
    ///
    /// The range covers an abbreviated SHA-1 through a full SHA-256 object id.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::InvalidCommitId`] if the value is the wrong length
    /// or contains a non-hexadecimal character.
    pub fn new(value: impl Into<String>) -> Result<Self, CoreError> {
        let value = value.into();
        let valid_length = (7..=64).contains(&value.len());
        let all_hex = value.chars().all(|c| c.is_ascii_hexdigit());
        if !valid_length || !all_hex {
            return Err(CoreError::InvalidCommitId { value });
        }
        Ok(Self(value.to_ascii_lowercase()))
    }

    /// The identifier as text.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for CommitId {
    type Error = CoreError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl From<CommitId> for String {
    fn from(value: CommitId) -> Self {
        value.0
    }
}

impl std::fmt::Display for CommitId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn node_and_edge_ids_render_distinguishably() {
        assert_eq!(NodeId::from_raw(12).to_string(), "n12");
        assert_eq!(EdgeId::from_raw(12).to_string(), "e12");
    }

    #[test]
    fn accepts_abbreviated_and_full_object_ids() {
        assert_eq!(CommitId::new("a3f9c21").unwrap().as_str(), "a3f9c21");
        let sha1 = "a3f9c2158b0d4e6f9c7a1b2d3e4f5a6b7c8d9e0f";
        assert_eq!(CommitId::new(sha1).unwrap().as_str(), sha1);
    }

    #[test]
    fn normalises_case_so_the_same_commit_compares_equal() {
        assert_eq!(
            CommitId::new("A3F9C21").unwrap(),
            CommitId::new("a3f9c21").unwrap()
        );
    }

    #[test]
    fn rejects_values_that_are_not_object_ids() {
        assert!(CommitId::new("a3f9c").is_err(), "too short");
        assert!(CommitId::new("z3f9c21").is_err(), "not hexadecimal");
        assert!(CommitId::new("").is_err(), "empty");
        assert!(
            CommitId::new("HEAD").is_err(),
            "a revision, not an object id"
        );
    }

    #[test]
    fn deserialising_validates() {
        assert!(serde_json::from_str::<CommitId>("\"a3f9c21\"").is_ok());
        assert!(serde_json::from_str::<CommitId>("\"not-a-commit\"").is_err());
    }
}
