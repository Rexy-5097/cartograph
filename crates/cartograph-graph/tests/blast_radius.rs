//! Blast radius: direction, termination, exclusion and confidence.
//!
//! The expected sets here are worked out by hand, not read off the
//! implementation, and every assertion names the artefacts rather than
//! counting them — a count passes for a set that is wrong in every member.
//!
//! Confidence is compared exactly. Under ADR-0018 every reported value is one
//! that already appears on an edge of the graph, so nothing here should ever
//! need a tolerance; if a value drifts, arithmetic has crept in where the
//! specification says there should be none, and that is the defect these tests
//! exist to catch.

#![allow(
    clippy::float_cmp,
    reason = "no arithmetic is performed on confidence; see ADR-0018"
)]

use cartograph_core::{
    Confidence, EdgeKind, Evidence, NodeId, NodeKind, Provenance, SourceLocation,
};
use cartograph_graph::blast::{blast_radius, is_dependency};
use cartograph_graph::{ArchitectureGraph, EdgeSpec};
use cartograph_testkit::fixed_clock;

/// Builds a graph from named nodes and `(from, to, kind, confidence)` edges.
struct Builder {
    graph: ArchitectureGraph,
    ids: std::collections::BTreeMap<&'static str, NodeId>,
}

impl Builder {
    fn new(names: &[&'static str]) -> Self {
        let mut graph = ArchitectureGraph::with_clock(fixed_clock);
        let mut ids = std::collections::BTreeMap::new();
        for (i, name) in names.iter().enumerate() {
            let id = graph
                .add_node(
                    NodeKind::Function,
                    *name,
                    Some(
                        SourceLocation::new(
                            format!("pkg/{name}.py"),
                            u32::try_from(i).unwrap_or(0) + 1,
                        )
                        .expect("valid location"),
                    ),
                )
                .expect("valid node");
            ids.insert(*name, id);
        }
        Self { graph, ids }
    }

    fn edge(&mut self, from: &str, to: &str, kind: EdgeKind, confidence: f32) -> &mut Self {
        let source = self.ids[from];
        let target = self.ids[to];
        self.graph
            .add_edge(EdgeSpec {
                source,
                target,
                kind,
                confidence: Confidence::new(confidence).expect("valid confidence"),
                provenance: Provenance::RouteMatcher,
                evidence: Evidence::new(format!("{from} -> {to}")).expect("non-empty"),
                location: SourceLocation::new("pkg/edge.py", 1).expect("valid location"),
                commit: None,
            })
            .expect("endpoints exist");
        self
    }

    fn id(&self, name: &str) -> NodeId {
        self.ids[name]
    }

    /// The blast radius of `name`, as a sorted list of node names.
    fn names_reaching(&self, name: &str) -> Vec<String> {
        let result = blast_radius(&self.graph, self.id(name));
        let mut names: Vec<String> = result
            .reached
            .iter()
            .map(|r| {
                self.graph
                    .node(r.node)
                    .expect("reached node is in the graph")
                    .name()
                    .to_owned()
            })
            .collect();
        names.sort();
        names
    }

    fn confidence_of(&self, target: &str, dependent: &str) -> Option<f32> {
        blast_radius(&self.graph, self.id(target))
            .get(self.id(dependent))
            .map(|r| r.confidence)
    }

    fn depth_of(&self, target: &str, dependent: &str) -> Option<u32> {
        blast_radius(&self.graph, self.id(target))
            .get(self.id(dependent))
            .map(|r| r.depth)
    }
}

// ── Direction and the oracle ────────────────────────────────────────────

/// A → B → C. Every edge points dependent → dependency, so the blast radius of
/// C is everything upstream of it.
#[test]
fn a_chain_reports_everything_upstream_of_the_target() {
    let mut b = Builder::new(&["A", "B", "C"]);
    b.edge("A", "B", EdgeKind::Call, 0.9)
        .edge("B", "C", EdgeKind::Call, 0.9);

    assert_eq!(b.names_reaching("C"), vec!["A", "B"]);
    assert_eq!(b.names_reaching("B"), vec!["A"]);
    assert_eq!(b.names_reaching("A"), Vec::<String>::new());
}

/// The target is never in its own radius. Changing a thing does not impact the
/// thing.
#[test]
fn the_target_is_excluded_from_its_own_radius() {
    let mut b = Builder::new(&["A", "B"]);
    b.edge("A", "B", EdgeKind::Call, 0.9);

    let result = blast_radius(&b.graph, b.id("B"));

    assert!(
        result.get(b.id("B")).is_none(),
        "the target reported itself"
    );
    assert_eq!(result.len(), 1);
}

/// Diamond: D is reached through both B and C, and appears once.
#[test]
fn a_diamond_collapses_to_one_entry_per_node() {
    let mut b = Builder::new(&["A", "B", "C", "D"]);
    b.edge("A", "B", EdgeKind::Call, 0.9)
        .edge("A", "C", EdgeKind::Call, 0.9)
        .edge("B", "D", EdgeKind::Call, 0.9)
        .edge("C", "D", EdgeKind::Call, 0.9);

    assert_eq!(b.names_reaching("D"), vec!["A", "B", "C"]);
    let result = blast_radius(&b.graph, b.id("D"));
    assert_eq!(result.len(), 3, "A must appear once, not once per route");
}

// ── Termination ─────────────────────────────────────────────────────────

/// A → B → C → A. Terminates, and the target stays out of its own result.
#[test]
fn a_cycle_terminates_and_excludes_the_target() {
    let mut b = Builder::new(&["A", "B", "C"]);
    b.edge("A", "B", EdgeKind::Call, 0.9)
        .edge("B", "C", EdgeKind::Call, 0.9)
        .edge("C", "A", EdgeKind::Call, 0.9);

    assert_eq!(b.names_reaching("A"), vec!["B", "C"]);
    assert_eq!(b.names_reaching("B"), vec!["A", "C"]);
}

#[test]
fn a_self_loop_terminates_and_reports_nothing() {
    let mut b = Builder::new(&["A"]);
    b.edge("A", "A", EdgeKind::Call, 0.9);

    let result = blast_radius(&b.graph, b.id("A"));

    assert!(result.is_empty(), "a node does not depend on itself");
}

#[test]
fn two_nodes_pointing_at_each_other_terminate() {
    let mut b = Builder::new(&["A", "B"]);
    b.edge("A", "B", EdgeKind::Call, 0.9)
        .edge("B", "A", EdgeKind::Call, 0.9);

    assert_eq!(b.names_reaching("A"), vec!["B"]);
    assert_eq!(b.names_reaching("B"), vec!["A"]);
}

/// A chain far deeper than any stack a recursive formulation would survive.
#[test]
fn a_very_deep_chain_does_not_overflow_the_stack() {
    let names: Vec<&'static str> = (0..5_000)
        .map(|i| -> &'static str { Box::leak(format!("n{i}").into_boxed_str()) })
        .collect();
    let mut b = Builder::new(&names);
    for pair in names.windows(2) {
        b.edge(pair[0], pair[1], EdgeKind::Call, 0.9);
    }

    let result = blast_radius(&b.graph, b.id(names[names.len() - 1]));

    assert_eq!(result.len(), names.len() - 1);
    assert_eq!(result.max_depth(), 4_999);
}

#[test]
fn duplicate_edges_between_the_same_pair_collapse() {
    let mut b = Builder::new(&["A", "B"]);
    b.edge("A", "B", EdgeKind::Call, 0.9)
        .edge("A", "B", EdgeKind::Call, 0.9)
        .edge("A", "B", EdgeKind::OrmAccess, 0.9);

    let result = blast_radius(&b.graph, b.id("B"));

    assert_eq!(result.len(), 1, "one dependent, however many edges join it");
}

// ── Isolation and absence ───────────────────────────────────────────────

#[test]
fn an_isolated_target_has_an_empty_radius() {
    let b = Builder::new(&["A", "B"]);

    assert!(blast_radius(&b.graph, b.id("A")).is_empty());
}

#[test]
fn a_disconnected_component_is_not_reported() {
    let mut b = Builder::new(&["A", "B", "X", "Y"]);
    b.edge("A", "B", EdgeKind::Call, 0.9)
        .edge("X", "Y", EdgeKind::Call, 0.9);

    assert_eq!(b.names_reaching("B"), vec!["A"]);
    assert_eq!(b.names_reaching("Y"), vec!["X"]);
}

/// An unknown handle is answered, not refused: nothing depends on a node that
/// is not in this graph.
#[test]
fn an_unknown_target_yields_an_empty_radius_without_panicking() {
    let mut b = Builder::new(&["A", "B"]);
    b.edge("A", "B", EdgeKind::Call, 0.9);

    let result = blast_radius(&b.graph, NodeId::from_raw(9_999));

    assert!(result.is_empty());
    assert_eq!(result.target, NodeId::from_raw(9_999));
}

#[test]
fn an_empty_graph_answers_nothing() {
    let graph = ArchitectureGraph::with_clock(fixed_clock);

    assert!(blast_radius(&graph, NodeId::from_raw(0)).is_empty());
}

// ── The edge allow-list ─────────────────────────────────────────────────

#[test]
fn the_four_dependency_kinds_are_traversed() {
    for kind in [
        EdgeKind::Call,
        EdgeKind::HttpCall,
        EdgeKind::OrmAccess,
        EdgeKind::Queries,
    ] {
        assert!(is_dependency(kind), "{kind:?} must be traversed");

        let mut b = Builder::new(&["A", "B"]);
        b.edge("A", "B", kind, 0.9);
        assert_eq!(b.names_reaching("B"), vec!["A"], "{kind:?}");
    }
}

/// Each excluded kind is proven to be **in the graph** before it is asserted
/// to be ignored — otherwise the test would pass on an empty graph and prove
/// nothing.
#[test]
fn the_excluded_kinds_are_present_but_never_traversed() {
    for kind in [
        EdgeKind::Import,
        EdgeKind::Inherits,
        EdgeKind::Implements,
        EdgeKind::References,
    ] {
        assert!(!is_dependency(kind), "{kind:?} must not be traversed");

        let mut b = Builder::new(&["A", "B"]);
        b.edge("A", "B", kind, 0.9);

        // Precondition: the relationship really is in the graph.
        assert_eq!(
            b.graph.edge_count(),
            1,
            "{kind:?} was not added, so ignoring it proves nothing"
        );
        assert_eq!(
            b.graph.edges_to(b.id("B")).count(),
            1,
            "{kind:?} is not an incoming edge of the target"
        );

        assert!(
            b.names_reaching("B").is_empty(),
            "{kind:?} widened the blast radius"
        );
    }
}

/// A mixed graph: only the allowed relationship carries impact.
#[test]
fn an_excluded_kind_does_not_extend_a_path_through_it() {
    let mut b = Builder::new(&["A", "B", "C"]);
    // A depends on B by an excluded kind; B depends on C by an allowed one.
    b.edge("A", "B", EdgeKind::Inherits, 0.9)
        .edge("B", "C", EdgeKind::Call, 0.9);

    assert_eq!(b.graph.edge_count(), 2, "precondition: both edges exist");
    // B is impacted by C; A is not, because the only route runs through an
    // excluded relationship.
    assert_eq!(b.names_reaching("C"), vec!["B"]);
}

// ── Confidence (ADR-0018) ───────────────────────────────────────────────

/// The weakest link, not the product.
#[test]
fn path_confidence_is_the_minimum_along_the_path() {
    let mut b = Builder::new(&["A", "B", "C"]);
    b.edge("A", "B", EdgeKind::Call, 0.9)
        .edge("B", "C", EdgeKind::Call, 0.7);

    assert_eq!(b.confidence_of("C", "A"), Some(0.7));
    assert_eq!(b.confidence_of("C", "B"), Some(0.7));
    // The product would be 0.63 and the mean 0.8; neither may appear.
    assert_ne!(b.confidence_of("C", "A"), Some(0.9 * 0.7));
}

/// The best route wins, and a worse second route does not degrade it.
#[test]
fn node_confidence_is_the_maximum_across_paths() {
    let mut b = Builder::new(&["A", "B", "D", "C"]);
    // A → B → C at 0.7, and A → D → C at 0.8.
    b.edge("A", "B", EdgeKind::Call, 0.9)
        .edge("B", "C", EdgeKind::Call, 0.7)
        .edge("A", "D", EdgeKind::Call, 0.8)
        .edge("D", "C", EdgeKind::Call, 0.8);

    assert_eq!(b.confidence_of("C", "A"), Some(0.8));
    assert_eq!(b.confidence_of("C", "B"), Some(0.7));
    assert_eq!(b.confidence_of("C", "D"), Some(0.8));
}

#[test]
fn a_three_hop_path_takes_its_weakest_edge() {
    let mut b = Builder::new(&["A", "B", "C", "D"]);
    b.edge("A", "B", EdgeKind::Call, 0.95)
        .edge("B", "C", EdgeKind::HttpCall, 0.60)
        .edge("C", "D", EdgeKind::OrmAccess, 0.85);

    assert_eq!(b.confidence_of("D", "A"), Some(0.60));
    assert_eq!(b.confidence_of("D", "B"), Some(0.60));
    assert_eq!(b.confidence_of("D", "C"), Some(0.85));
}

#[test]
fn equal_confidence_paths_agree_and_prefer_the_shorter_one() {
    let mut b = Builder::new(&["A", "B", "C"]);
    // A reaches C directly and through B, both at 0.8.
    b.edge("A", "C", EdgeKind::Call, 0.8)
        .edge("A", "B", EdgeKind::Call, 0.8)
        .edge("B", "C", EdgeKind::Call, 0.8);

    assert_eq!(b.confidence_of("C", "A"), Some(0.8));
    assert_eq!(
        b.depth_of("C", "A"),
        Some(1),
        "the shorter route wins the tie"
    );
}

/// Every reported confidence is a value that appears on some edge. Nothing is
/// synthesised — the property that makes "no arithmetic" checkable.
#[test]
fn every_reported_confidence_appears_on_an_edge() {
    let mut b = Builder::new(&["A", "B", "C", "D"]);
    b.edge("A", "B", EdgeKind::Call, 0.95)
        .edge("B", "C", EdgeKind::HttpCall, 0.62)
        .edge("C", "D", EdgeKind::OrmAccess, 0.81)
        .edge("A", "D", EdgeKind::Call, 0.73);

    let present: Vec<f32> = b.graph.edges().map(|e| e.confidence().get()).collect();
    for entry in &blast_radius(&b.graph, b.id("D")).reached {
        assert!(
            present.contains(&entry.confidence),
            "{} is not any edge's confidence",
            entry.confidence
        );
    }
}

/// Depth is reported separately because confidence does not decay with it.
#[test]
fn depth_is_the_hop_count_of_the_chosen_route() {
    let mut b = Builder::new(&["A", "B", "C"]);
    b.edge("A", "B", EdgeKind::Call, 0.9)
        .edge("B", "C", EdgeKind::Call, 0.9);

    assert_eq!(b.depth_of("C", "B"), Some(1));
    assert_eq!(b.depth_of("C", "A"), Some(2));
}

// ── Evidence ────────────────────────────────────────────────────────────

/// Each entry names the edge it was reached through, so the impact set is
/// evidenced from data the graph already holds.
#[test]
fn each_entry_names_a_real_edge_that_carries_its_evidence() {
    let mut b = Builder::new(&["A", "B", "C"]);
    b.edge("A", "B", EdgeKind::Call, 0.9)
        .edge("B", "C", EdgeKind::OrmAccess, 0.8);

    for entry in &blast_radius(&b.graph, b.id("C")).reached {
        let edge = b.graph.edge(entry.via).expect("via names a real edge");
        assert!(!edge.evidence().as_str().is_empty(), "RULE 006");
        assert!(
            is_dependency(edge.kind()),
            "reached through an excluded kind"
        );
        assert_eq!(edge.source(), entry.node, "via must leave the reached node");
    }
}

// ── Determinism ─────────────────────────────────────────────────────────

#[test]
fn the_same_graph_gives_the_same_answer_every_time() {
    let mut b = Builder::new(&["A", "B", "C", "D"]);
    b.edge("A", "B", EdgeKind::Call, 0.9)
        .edge("B", "D", EdgeKind::Call, 0.7)
        .edge("A", "C", EdgeKind::Call, 0.8)
        .edge("C", "D", EdgeKind::Call, 0.8);

    let first = blast_radius(&b.graph, b.id("D"));
    for _ in 0..8 {
        assert_eq!(blast_radius(&b.graph, b.id("D")), first);
    }
}

/// The defect this exists to catch: an answer that depends on the order the
/// architecture was described in. The two graphs below are the same system
/// with different insertion orders, so their `NodeId`s differ — and the
/// reported names, confidences and depths must not.
#[test]
fn insertion_order_cannot_change_the_answer() {
    let mut forward = Builder::new(&["A", "B", "C"]);
    forward
        .edge("A", "B", EdgeKind::Call, 0.9)
        .edge("B", "C", EdgeKind::Call, 0.7);

    let mut reversed = Builder::new(&["C", "B", "A"]);
    reversed
        .edge("B", "C", EdgeKind::Call, 0.7)
        .edge("A", "B", EdgeKind::Call, 0.9);

    // Precondition: the handles genuinely differ, or this proves nothing.
    assert_ne!(
        forward.id("C").as_u64(),
        reversed.id("C").as_u64(),
        "precondition: the two graphs must number their nodes differently"
    );

    assert_eq!(forward.names_reaching("C"), reversed.names_reaching("C"));
    assert_eq!(
        forward.confidence_of("C", "A"),
        reversed.confidence_of("C", "A")
    );
    assert_eq!(forward.depth_of("C", "A"), reversed.depth_of("C", "A"));
}

/// Output order is semantic, not discovery order or handle order.
#[test]
fn results_are_ordered_by_semantic_identity() {
    let mut b = Builder::new(&["zeta", "alpha", "middle", "target"]);
    b.edge("zeta", "target", EdgeKind::Call, 0.9)
        .edge("alpha", "target", EdgeKind::Call, 0.9)
        .edge("middle", "target", EdgeKind::Call, 0.9);

    let result = blast_radius(&b.graph, b.id("target"));
    let order: Vec<&str> = result
        .reached
        .iter()
        .map(|r| b.graph.node(r.node).expect("present").name())
        .collect();

    // Sorted by `(kind, name, file)`, so alphabetical here — not the order
    // they were added, which was zeta, alpha, middle.
    assert_eq!(order, vec!["alpha", "middle", "zeta"]);
}

// ── Scale ───────────────────────────────────────────────────────────────

#[test]
fn a_large_fan_in_is_reported_completely() {
    let names: Vec<&'static str> = std::iter::once("target")
        .chain((0..2_000).map(|i| -> &'static str { Box::leak(format!("d{i}").into_boxed_str()) }))
        .collect();
    let mut b = Builder::new(&names);
    for name in names.iter().skip(1) {
        b.edge(name, "target", EdgeKind::OrmAccess, 0.8);
    }

    let result = blast_radius(&b.graph, b.id("target"));

    assert_eq!(result.len(), 2_000);
    assert_eq!(result.max_depth(), 1);
}

#[test]
fn a_large_fan_out_from_the_target_reports_nothing() {
    let names: Vec<&'static str> = std::iter::once("target")
        .chain((0..500).map(|i| -> &'static str { Box::leak(format!("u{i}").into_boxed_str()) }))
        .collect();
    let mut b = Builder::new(&names);
    for name in names.iter().skip(1) {
        b.edge("target", name, EdgeKind::Call, 0.8);
    }

    // The target depends on 500 things; nothing depends on the target.
    assert!(
        blast_radius(&b.graph, b.id("target")).is_empty(),
        "fan-out is not blast radius; the direction would be reversed"
    );
}

// ── The cross-stack shape the milestone is about ────────────────────────

/// The chain M12 exists to surface, in miniature: a React component reaching a
/// database table through an HTTP call and an ORM access.
#[test]
fn a_cross_stack_chain_is_reported_end_to_end() {
    let mut b = Builder::new(&["Component", "apiClient", "handler", "Model", "table"]);
    b.edge("Component", "apiClient", EdgeKind::Call, 0.99)
        .edge("apiClient", "handler", EdgeKind::HttpCall, 0.98)
        .edge("handler", "Model", EdgeKind::OrmAccess, 0.80)
        .edge("Model", "table", EdgeKind::Queries, 0.95);

    assert_eq!(
        b.names_reaching("table"),
        vec!["Component", "Model", "apiClient", "handler"]
    );
    // The weakest step is the ORM access at 0.80, and it caps everything
    // upstream of it — including the component four hops away.
    assert_eq!(b.confidence_of("table", "Component"), Some(0.80));
    assert_eq!(b.depth_of("table", "Component"), Some(4));
    // The model itself is reached over the strong Queries edge alone.
    assert_eq!(b.confidence_of("table", "Model"), Some(0.95));
}
