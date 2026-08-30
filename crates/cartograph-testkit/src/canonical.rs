//! Canonical graph equality — the relation `incremental == clean` is stated in.
//!
//! Comparing two [`ArchitectureGraph`] values directly is wrong. `NodeId` and
//! `EdgeId` are insertion-ordered and graph-local (ADR-0011), so two runs that
//! recover exactly the same architecture can number their nodes differently
//! and compare unequal. A test written that way fails for a reason that is not
//! a defect, and the usual repair — comparing counts instead — passes for a
//! graph that is wrong in every detail.
//!
//! So equality is defined over what the graph *claims*, not how it is stored:
//! a node is its kind, name and location; an edge is its endpoints' identities
//! plus kind, confidence, provenance, evidence and location. Both sides are
//! sorted multisets, because a duplicated edge is a real difference.
//!
//! Deliberately excluded: `NodeId`, `EdgeId`, insertion order, and
//! `created_at` — a wall-clock timestamp is nondeterministic by construction
//! and comparing it would make every run differ. Nothing else is excluded.
//! Confidence, provenance and evidence text are the product's claims, and a
//! mismatch in any of them is exactly the defect this relation exists to
//! catch.

use std::fmt::Write as _;

use cartograph_core::Node;
use cartograph_graph::ArchitectureGraph;

/// A graph reduced to its semantic content, in a stable order.
///
/// Two graphs are equal when their canonical forms are equal. The strings are
/// human-readable on purpose: a failing differential test should show which
/// edge differs, not that two opaque digests disagree.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CanonicalGraph {
    /// Every node's semantic identity, sorted.
    pub nodes: Vec<String>,
    /// Every edge's semantic identity, sorted.
    pub edges: Vec<String>,
}

/// A node's semantic identity: what it is, not which slot it occupies.
///
/// A node without a location is rendered with `-`; that is a real distinction
/// (a synthesised node such as a database table has no source line) and
/// collapsing it would hide a difference.
#[must_use]
pub fn node_identity(node: &Node) -> String {
    let location = node
        .location()
        .map_or_else(|| "-".to_owned(), |l| format!("{}:{}", l.file(), l.line()));
    format!("{:?}|{}|{}", node.kind(), node.name(), location)
}

/// Reduces a graph to its canonical form.
///
/// Edges name their endpoints by *identity* rather than by id, which is what
/// makes the comparison survive different node numbering. An edge whose
/// endpoint is missing from the graph would be a corrupt graph; it is rendered
/// as `<dangling>` rather than skipped, so the corruption shows up as a
/// difference instead of vanishing.
#[must_use]
pub fn canonical(graph: &ArchitectureGraph) -> CanonicalGraph {
    let mut nodes: Vec<String> = graph.nodes().map(node_identity).collect();
    nodes.sort();

    let mut edges: Vec<String> = graph
        .edges()
        .map(|edge| {
            let endpoint = |id| {
                graph
                    .node(id)
                    .map_or_else(|| "<dangling>".to_owned(), node_identity)
            };
            let mut s = String::new();
            let _ = write!(
                s,
                "{} -[{:?} conf={:.6} prov={:?}]-> {} @ {}:{} evidence={}",
                endpoint(edge.source()),
                edge.kind(),
                edge.confidence().get(),
                edge.provenance(),
                endpoint(edge.target()),
                edge.location().file(),
                edge.location().line(),
                edge.evidence().as_str(),
            );
            s
        })
        .collect();
    edges.sort();

    CanonicalGraph { nodes, edges }
}

/// The first differences between two canonical graphs, for assertion messages.
///
/// Returns an empty vector when the graphs are equal. Reports both directions:
/// a missing edge and a spurious one are different defects, and a message that
/// only showed one would send the reader the wrong way.
#[must_use]
pub fn differences(left: &CanonicalGraph, right: &CanonicalGraph) -> Vec<String> {
    let mut out = Vec::new();
    for (label, l, r) in [
        ("node", &left.nodes, &right.nodes),
        ("edge", &left.edges, &right.edges),
    ] {
        for missing in multiset_difference(l, r) {
            out.push(format!("only in left  ({label}): {missing}"));
        }
        for extra in multiset_difference(r, l) {
            out.push(format!("only in right ({label}): {extra}"));
        }
    }
    out
}

/// Items in `a` that `b` does not have, respecting multiplicity.
fn multiset_difference(a: &[String], b: &[String]) -> Vec<String> {
    let mut counts: std::collections::HashMap<&str, isize> = std::collections::HashMap::new();
    for item in b {
        *counts.entry(item.as_str()).or_default() += 1;
    }
    let mut out = Vec::new();
    for item in a {
        let slot = counts.entry(item.as_str()).or_default();
        if *slot > 0 {
            *slot -= 1;
        } else {
            out.push(item.clone());
        }
    }
    out
}

/// Asserts two graphs are semantically equal, naming what differs.
///
/// # Panics
///
/// When the canonical forms differ, with the differing nodes and edges listed.
pub fn assert_graphs_equal(left: &ArchitectureGraph, right: &ArchitectureGraph, context: &str) {
    let (l, r) = (canonical(left), canonical(right));
    if l == r {
        return;
    }
    let diffs = differences(&l, &r);
    panic!(
        "{context}: graphs differ ({} nodes/{} edges vs {} nodes/{} edges)\n{}",
        l.nodes.len(),
        l.edges.len(),
        r.nodes.len(),
        r.edges.len(),
        diffs.join("\n")
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use cartograph_core::{Confidence, EdgeKind, Evidence, NodeKind, Provenance, SourceLocation};
    use cartograph_graph::EdgeSpec;

    fn graph_with(table_name: &str, confidence: f32) -> ArchitectureGraph {
        let mut g = ArchitectureGraph::new();
        let handler = g
            .add_node(
                NodeKind::Function,
                "create_order",
                Some(SourceLocation::new("api/orders.py", 10).unwrap()),
            )
            .unwrap();
        let table = g.add_node(NodeKind::Table, table_name, None).unwrap();
        g.add_edge(EdgeSpec {
            source: handler,
            target: table,
            kind: EdgeKind::Queries,
            confidence: Confidence::new(confidence).unwrap(),
            provenance: Provenance::OrmResolution,
            evidence: Evidence::new("maps to table").unwrap(),
            location: SourceLocation::new("api/orders.py", 10).unwrap(),
            commit: None,
        })
        .unwrap();
        g
    }

    #[test]
    fn identical_graphs_are_equal_despite_independent_construction() {
        let a = graph_with("orders", 0.8);
        let b = graph_with("orders", 0.8);
        assert_eq!(canonical(&a), canonical(&b));
        assert!(differences(&canonical(&a), &canonical(&b)).is_empty());
    }

    #[test]
    fn a_changed_table_name_is_a_difference() {
        let a = graph_with("orders", 0.8);
        let b = graph_with("purchase_orders", 0.8);
        assert_ne!(canonical(&a), canonical(&b));
        assert!(!differences(&canonical(&a), &canonical(&b)).is_empty());
    }

    /// The failure mode a count-based comparison would miss entirely: same
    /// shape, same names, different claim.
    #[test]
    fn a_changed_confidence_is_a_difference() {
        let a = graph_with("orders", 0.8);
        let b = graph_with("orders", 0.9);
        assert_eq!(a.node_count(), b.node_count());
        assert_eq!(a.edge_count(), b.edge_count());
        assert_ne!(canonical(&a), canonical(&b));
    }

    /// Node numbering must not participate in equality: insertion order here
    /// differs from `graph_with`, and the graphs are still the same graph.
    #[test]
    fn insertion_order_does_not_affect_equality() {
        let a = graph_with("orders", 0.8);
        let mut b = ArchitectureGraph::new();
        let table = b.add_node(NodeKind::Table, "orders", None).unwrap();
        let handler = b
            .add_node(
                NodeKind::Function,
                "create_order",
                Some(SourceLocation::new("api/orders.py", 10).unwrap()),
            )
            .unwrap();
        b.add_edge(EdgeSpec {
            source: handler,
            target: table,
            kind: EdgeKind::Queries,
            confidence: Confidence::new(0.8).unwrap(),
            provenance: Provenance::OrmResolution,
            evidence: Evidence::new("maps to table").unwrap(),
            location: SourceLocation::new("api/orders.py", 10).unwrap(),
            commit: None,
        })
        .unwrap();

        // The handler is node 1 here and node 0 in `a`: the numbering really
        // does differ, so the equality below is doing work.
        assert_ne!(handler.as_u64(), a.nodes().next().unwrap().id().as_u64());
        assert_eq!(canonical(&a), canonical(&b));
    }

    #[test]
    fn a_duplicated_edge_is_not_absorbed() {
        let a = graph_with("orders", 0.8);
        let mut b = graph_with("orders", 0.8);
        let (source, target) = {
            let e = b.edges().next().unwrap();
            (e.source(), e.target())
        };
        b.add_edge(EdgeSpec {
            source,
            target,
            kind: EdgeKind::Queries,
            confidence: Confidence::new(0.8).unwrap(),
            provenance: Provenance::OrmResolution,
            evidence: Evidence::new("maps to table").unwrap(),
            location: SourceLocation::new("api/orders.py", 10).unwrap(),
            commit: None,
        })
        .unwrap();
        assert_ne!(canonical(&a), canonical(&b));
    }
}
