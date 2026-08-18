//! Fixtures and builders for Cartograph's tests.
//!
//! Constructing a valid [`Edge`] takes ten arguments by design — the domain
//! model refuses to build a relationship that cannot explain itself. That is
//! correct for production code and tedious in a test that only cares about one
//! field. This crate absorbs the tedium so tests stay readable, without
//! weakening the rules the domain model enforces.
//!
//! Nothing here is a runtime dependency: the crate is `publish = false` and is
//! only ever a dev-dependency.
//!
//! # What this crate is not
//!
//! [`cross_stack_chain`] returns the specification's worked example as **fixed
//! test data**. It is hand-written, not resolved. No parser or resolver exists
//! until M01-M06; the fixture exists so that graph code can be exercised
//! against a realistic shape in the meantime, and so the shape itself is
//! reviewed early.

use cartograph_core::{
    CommitId, Confidence, Edge, EdgeId, EdgeKind, Evidence, Node, NodeId, NodeKind, Provenance,
    SourceLocation,
};
use time::OffsetDateTime;
use time::macros::datetime;

/// A fixed timestamp, so serialised fixtures compare byte for byte.
///
/// Matches the `created_at` in the specification's worked edge record.
pub const FIXED_TIME: OffsetDateTime = datetime!(2026-08-17 09:14:22 UTC);

/// Returns [`FIXED_TIME`]. Pass to
/// `ArchitectureGraph::with_clock` to get deterministic edges.
#[must_use]
pub fn fixed_clock() -> OffsetDateTime {
    FIXED_TIME
}

/// Builds a [`Node`], panicking on invalid input.
///
/// # Panics
///
/// Panics if `name` is empty or `location` is not a valid repository-relative
/// path. Tests should fail loudly on a malformed fixture.
#[must_use]
pub fn node(raw_id: u64, kind: NodeKind, name: &str, location: Option<(&str, u32)>) -> Node {
    let location = location.map(|(file, line)| {
        SourceLocation::new(file, line).expect("fixture location must be repository-relative")
    });
    Node::new(NodeId::from_raw(raw_id), kind, name, location).expect("fixture node must be valid")
}

/// Builds an [`Edge`] with sensible defaults, panicking on invalid input.
///
/// Confidence defaults to the prior for `provenance`, and the commit is the
/// specification's `a3f9c21`.
///
/// # Panics
///
/// Panics if `evidence` is blank or `location` is not repository-relative.
#[must_use]
pub fn edge(
    raw_id: u64,
    source: u64,
    target: u64,
    kind: EdgeKind,
    provenance: Provenance,
    evidence: &str,
    location: (&str, u32),
) -> Edge {
    edge_with_confidence(
        raw_id,
        source,
        target,
        kind,
        provenance,
        provenance.prior_confidence(),
        evidence,
        location,
    )
}

/// Builds an [`Edge`] with an explicit confidence.
///
/// # Panics
///
/// Panics if `evidence` is blank or `location` is not repository-relative.
#[must_use]
#[allow(clippy::too_many_arguments)]
pub fn edge_with_confidence(
    raw_id: u64,
    source: u64,
    target: u64,
    kind: EdgeKind,
    provenance: Provenance,
    confidence: Confidence,
    evidence: &str,
    location: (&str, u32),
) -> Edge {
    Edge::new(
        EdgeId::from_raw(raw_id),
        NodeId::from_raw(source),
        NodeId::from_raw(target),
        kind,
        confidence,
        provenance,
        Evidence::new(evidence).expect("fixture evidence must not be blank"),
        SourceLocation::new(location.0, location.1)
            .expect("fixture location must be repository-relative"),
        Some(CommitId::new("a3f9c21").expect("fixture commit must be a valid object id")),
        FIXED_TIME,
    )
}

/// One hop of the specification's cross-stack chain.
pub struct ChainHop {
    /// The artefact this hop starts at.
    pub source: Node,
    /// The artefact this hop reaches.
    pub target: Node,
    /// The relationship, with its evidence.
    pub edge: Edge,
}

/// The worked example from the frozen specification, as fixed test data.
///
/// ```text
/// CheckoutButton.tsx -> POST /api/orders -> create_order() -> Order -> orders
/// ```
///
/// Four artefacts, three languages, one causal path. Hand-written: see the
/// crate documentation.
///
/// # Panics
///
/// Panics if this fixture is edited into an invalid state — a blank evidence
/// string, an absolute path, a confidence outside the unit interval. The
/// fixture is a constant, so a panic here means the file was broken, not that
/// a caller passed something wrong.
#[must_use]
pub fn cross_stack_chain() -> Vec<ChainHop> {
    let button = node(
        0,
        NodeKind::Function,
        "onSubmit",
        Some(("web/src/CheckoutButton.tsx", 34)),
    );
    let route = node(1, NodeKind::Route, "POST /api/orders/{*}", None);
    let handler = node(
        2,
        NodeKind::Function,
        "create_order",
        Some(("api/orders.py", 12)),
    );
    let model = node(3, NodeKind::Class, "Order", Some(("api/models.py", 8)));
    let table = node(4, NodeKind::Table, "orders", None);

    vec![
        ChainHop {
            source: button.clone(),
            target: route.clone(),
            edge: edge_with_confidence(
                0,
                0,
                1,
                EdgeKind::HttpCall,
                Provenance::TemplateEvaluation,
                Confidence::new(0.94).expect("0.94 is in the unit interval"),
                "POST /api/orders",
                ("web/src/CheckoutButton.tsx", 34),
            ),
        },
        ChainHop {
            source: route,
            target: handler.clone(),
            edge: edge(
                1,
                1,
                2,
                EdgeKind::Call,
                Provenance::RouteMatcher,
                "POST /api/orders/{*}",
                ("api/orders.py", 11),
            ),
        },
        ChainHop {
            source: handler,
            target: model.clone(),
            edge: edge_with_confidence(
                2,
                2,
                3,
                EdgeKind::OrmAccess,
                Provenance::OrmResolution,
                Confidence::new(0.97).expect("0.97 is in the unit interval"),
                "Order(...)",
                ("api/orders.py", 18),
            ),
        },
        ChainHop {
            source: model,
            target: table,
            edge: edge(
                3,
                3,
                4,
                EdgeKind::Queries,
                Provenance::OrmResolution,
                "__tablename__ = \"orders\"",
                ("api/models.py", 10),
            ),
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_chain_is_connected_end_to_end() {
        let chain = cross_stack_chain();
        assert_eq!(chain.len(), 4, "four hops across three languages");

        for pair in chain.windows(2) {
            assert_eq!(
                pair[0].edge.target(),
                pair[1].edge.source(),
                "the chain is broken: {} does not continue into {}",
                pair[0].target.name(),
                pair[1].source.name()
            );
        }
    }

    #[test]
    fn every_hop_carries_its_evidence() {
        for hop in cross_stack_chain() {
            assert!(
                !hop.edge.evidence().as_str().is_empty(),
                "{} -> {} has no evidence",
                hop.source.name(),
                hop.target.name()
            );
        }
    }
}
