//! Edges: relationships, and how each one was derived.

use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

use crate::{
    confidence::Confidence,
    evidence::Evidence,
    id::{CommitId, EdgeId, NodeId},
    location::SourceLocation,
    provenance::Provenance,
};

/// What kind of relationship an edge asserts.
///
/// The set is fixed by the frozen specification. `HttpCall`, `OrmAccess` and
/// `Queries` are the cross-stack kinds — the ones no competing tool produces,
/// and the reason the resolver exists.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
#[non_exhaustive]
pub enum EdgeKind {
    /// One module imports another.
    Import,
    /// One symbol calls another.
    Call,
    /// A class inherits from another.
    Inherits,
    /// A class implements an interface.
    Implements,
    /// A symbol references another without calling it.
    References,
    /// A frontend call site reaches a backend route over HTTP.
    HttpCall,
    /// Code accesses a database through an ORM model.
    OrmAccess,
    /// Code queries a table directly.
    Queries,
}

/// A relationship between two nodes, together with the case for believing it.
///
/// # The rule this type enforces
///
/// Cartograph never stores `A -> B`. Every edge carries [`Confidence`],
/// [`Provenance`], [`Evidence`] and the [`SourceLocation`] where the claim was
/// observed (RULES 004-006). There is deliberately no `Default`, no builder
/// that can be finished early, and no public field: the only way to obtain an
/// `Edge` is [`Edge::new`], which takes all four. An edge that cannot explain
/// itself is not merely discouraged — it does not compile.
///
/// This is the field set from the specification's worked example:
///
/// ```text
/// source      CheckoutButton.tsx:34
/// target      orders.py:12 create_order
/// kind        HttpCall
/// confidence  0.94
/// provenance  route-matcher
/// evidence    POST /api/orders
/// commit      a3f9c21
/// created_at  2026-08-17T09:14:22Z
/// ```
///
/// # No language model may construct one of these
///
/// Edges are computed by static analysis. A model may later *explain* an edge
/// that already exists, but nothing in the resolver pipeline may ask a model
/// what connects to what (RULE 007, `docs/adr/ADR-0007-no-llm-graph-construction.md`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Edge {
    id: EdgeId,
    source: NodeId,
    target: NodeId,
    kind: EdgeKind,
    confidence: Confidence,
    provenance: Provenance,
    evidence: Evidence,
    location: SourceLocation,
    #[serde(skip_serializing_if = "Option::is_none")]
    commit: Option<CommitId>,
    #[serde(with = "time::serde::rfc3339")]
    created_at: OffsetDateTime,
}

impl Edge {
    /// Constructs an edge.
    ///
    /// Every argument is mandatory except `commit`, which is `None` until Git
    /// integration lands. `None` means "not known", not "no commit" — unknown
    /// values stay explicitly unknown (RULE 009).
    #[allow(clippy::too_many_arguments)]
    #[must_use]
    pub fn new(
        id: EdgeId,
        source: NodeId,
        target: NodeId,
        kind: EdgeKind,
        confidence: Confidence,
        provenance: Provenance,
        evidence: Evidence,
        location: SourceLocation,
        commit: Option<CommitId>,
        created_at: OffsetDateTime,
    ) -> Self {
        Self {
            id,
            source,
            target,
            kind,
            confidence,
            provenance,
            evidence,
            location,
            commit,
            created_at,
        }
    }

    /// The edge's handle.
    #[must_use]
    pub fn id(&self) -> EdgeId {
        self.id
    }

    /// The node the relationship starts at.
    #[must_use]
    pub fn source(&self) -> NodeId {
        self.source
    }

    /// The node the relationship points to.
    #[must_use]
    pub fn target(&self) -> NodeId {
        self.target
    }

    /// What kind of relationship this is.
    #[must_use]
    pub fn kind(&self) -> EdgeKind {
        self.kind
    }

    /// How much this edge should be trusted.
    #[must_use]
    pub fn confidence(&self) -> Confidence {
        self.confidence
    }

    /// Which analysis produced this edge.
    #[must_use]
    pub fn provenance(&self) -> Provenance {
        self.provenance
    }

    /// The observation supporting this edge.
    #[must_use]
    pub fn evidence(&self) -> &Evidence {
        &self.evidence
    }

    /// Where the claim was observed in the source.
    #[must_use]
    pub fn location(&self) -> &SourceLocation {
        &self.location
    }

    /// The revision this edge was derived from, if known.
    #[must_use]
    pub fn commit(&self) -> Option<&CommitId> {
        self.commit.as_ref()
    }

    /// When this edge was derived.
    #[must_use]
    pub fn created_at(&self) -> OffsetDateTime {
        self.created_at
    }

    /// Whether the analysis that produced this edge computes rather than
    /// estimates.
    ///
    /// Convenience for the interface, which renders derived edges solid and
    /// inferred edges dashed.
    #[must_use]
    pub fn is_deterministic(&self) -> bool {
        self.provenance.is_deterministic()
    }
}

#[cfg(test)]
mod tests {
    use time::macros::datetime;

    use super::*;

    fn cross_stack_edge() -> Edge {
        Edge::new(
            EdgeId::from_raw(1),
            NodeId::from_raw(1),
            NodeId::from_raw(2),
            EdgeKind::HttpCall,
            Confidence::new(0.94).unwrap(),
            Provenance::RouteMatcher,
            Evidence::new("POST /api/orders").unwrap(),
            SourceLocation::new("web/src/CheckoutButton.tsx", 34).unwrap(),
            Some(CommitId::new("a3f9c21").unwrap()),
            datetime!(2026-08-17 09:14:22 UTC),
        )
    }

    #[test]
    fn reproduces_the_specifications_worked_example() {
        let edge = cross_stack_edge();

        assert_eq!(edge.kind(), EdgeKind::HttpCall);
        assert_eq!(edge.confidence().to_string(), "0.94");
        assert_eq!(edge.provenance().to_string(), "route-matcher");
        assert_eq!(edge.evidence().as_str(), "POST /api/orders");
        assert_eq!(edge.location().to_string(), "web/src/CheckoutButton.tsx:34");
        assert_eq!(edge.commit().unwrap().as_str(), "a3f9c21");
    }

    #[test]
    fn round_trips_through_json_without_losing_the_case_for_the_edge() {
        let edge = cross_stack_edge();
        let json = serde_json::to_string(&edge).unwrap();
        let restored: Edge = serde_json::from_str(&json).unwrap();
        assert_eq!(edge, restored);
    }

    #[test]
    fn serialises_created_at_as_rfc_3339() {
        let json = serde_json::to_value(cross_stack_edge()).unwrap();
        assert_eq!(json["created_at"], "2026-08-17T09:14:22Z");
    }

    #[test]
    fn an_unknown_commit_is_absent_rather_than_guessed() {
        let edge = Edge::new(
            EdgeId::from_raw(2),
            NodeId::from_raw(1),
            NodeId::from_raw(2),
            EdgeKind::Import,
            Confidence::STATIC_IMPORT,
            Provenance::StaticImportResolution,
            Evidence::new("import { api } from './api'").unwrap(),
            SourceLocation::new("web/src/CheckoutButton.tsx", 3).unwrap(),
            None,
            datetime!(2026-08-17 09:14:22 UTC),
        );

        assert_eq!(edge.commit(), None);
        let json = serde_json::to_value(&edge).unwrap();
        assert!(
            json.get("commit").is_none(),
            "an unknown commit must be absent, not null or empty"
        );
    }

    #[test]
    fn reports_whether_the_producing_analysis_was_deterministic() {
        assert!(cross_stack_edge().is_deterministic());

        let inferred = Edge::new(
            EdgeId::from_raw(3),
            NodeId::from_raw(1),
            NodeId::from_raw(2),
            EdgeKind::HttpCall,
            Confidence::PARTIAL_TEMPLATE_MATCH,
            Provenance::TemplateEvaluation,
            Evidence::new("POST /api/v1/orders/{unknown}").unwrap(),
            SourceLocation::new("web/src/CheckoutButton.tsx", 34).unwrap(),
            None,
            datetime!(2026-08-17 09:14:22 UTC),
        );
        assert!(!inferred.is_deterministic());
    }
}
