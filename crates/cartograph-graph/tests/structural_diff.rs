//! Structural diff between two independently built graphs.
//!
//! Each case builds the two graphs *separately*, with node ids deliberately
//! allocated in different orders where it matters, so a diff that leaned on
//! `NodeId` would fail rather than pass by coincidence.

use cartograph_core::{Confidence, EdgeKind, Evidence, NodeKind, Provenance, SourceLocation};
use cartograph_graph::ArchitectureGraph;
use cartograph_graph::diff::{ChangedField, diff};

struct Builder {
    graph: ArchitectureGraph,
}

impl Builder {
    fn new() -> Self {
        Self {
            graph: ArchitectureGraph::new(),
        }
    }

    /// Adds an edge, creating its endpoints on demand.
    #[allow(clippy::too_many_arguments)]
    fn edge(
        &mut self,
        from: (NodeKind, &str, &str),
        to: (NodeKind, &str, &str),
        kind: EdgeKind,
        confidence: f32,
        provenance: Provenance,
        file: &str,
        line: u32,
        evidence: &str,
    ) -> &mut Self {
        let source = self
            .graph
            .node_for(
                from.0,
                from.1,
                Some(SourceLocation::new(from.2, 1).expect("location")),
            )
            .expect("source");
        let target = self
            .graph
            .node_for(
                to.0,
                to.1,
                Some(SourceLocation::new(to.2, 1).expect("location")),
            )
            .expect("target");
        self.graph
            .add_edge(cartograph_graph::EdgeSpec {
                source,
                target,
                kind,
                confidence: Confidence::new(confidence).expect("confidence"),
                provenance,
                evidence: Evidence::new(evidence).expect("evidence"),
                location: SourceLocation::new(file, line).expect("location"),
                commit: None,
            })
            .expect("edge");
        self
    }
}

/// `handler` calls `helper`, observed in `api.py` at line 10.
fn baseline() -> ArchitectureGraph {
    let mut b = Builder::new();
    b.edge(
        (NodeKind::Function, "handler", "api.py"),
        (NodeKind::Function, "helper", "util.py"),
        EdgeKind::Call,
        0.9,
        Provenance::StaticImportResolution,
        "api.py",
        10,
        "helper() in handler",
    );
    b.graph
}

// ── Unchanged ───────────────────────────────────────────────────────────

#[test]
fn two_identical_analyses_produce_an_empty_diff() {
    let result = diff(&baseline(), &baseline());

    assert!(result.is_empty(), "identical graphs must not differ");
    assert_eq!(result.len(), 0);
    assert_eq!(result.unchanged, 1);
}

/// The property the milestone rests on: correspondence survives ids that do
/// not match. The right-hand graph allocates its nodes in the opposite order.
#[test]
fn correspondence_is_by_identity_not_by_node_id() {
    let left = baseline();

    let mut b = Builder::new();
    // Allocate an unrelated node first, so every id on this side is shifted.
    b.edge(
        (NodeKind::Function, "unrelated", "other.py"),
        (NodeKind::Function, "sink", "other.py"),
        EdgeKind::Call,
        0.9,
        Provenance::StaticImportResolution,
        "other.py",
        1,
        "sink() in unrelated",
    );
    b.edge(
        (NodeKind::Function, "handler", "api.py"),
        (NodeKind::Function, "helper", "util.py"),
        EdgeKind::Call,
        0.9,
        Provenance::StaticImportResolution,
        "api.py",
        10,
        "helper() in handler",
    );
    let right = b.graph;

    let result = diff(&left, &right);

    // The shared relationship is unchanged despite different ids; only the
    // genuinely new one is added.
    assert_eq!(result.unchanged, 1, "the shared edge must correspond");
    assert_eq!(result.added.len(), 1);
    assert!(result.removed.is_empty());
    assert!(result.changed.is_empty());
}

// ── Added and removed ───────────────────────────────────────────────────

#[test]
fn an_edge_only_on_the_right_is_added() {
    let left = ArchitectureGraph::new();
    let result = diff(&left, &baseline());

    assert_eq!(result.added.len(), 1);
    assert!(result.removed.is_empty());
    assert_eq!(result.added[0].identity.kind, "call");
    assert!(result.added[0].identity.source.as_str().contains("handler"));
    assert!(result.added[0].identity.target.as_str().contains("helper"));
}

#[test]
fn an_edge_only_on_the_left_is_removed() {
    let right = ArchitectureGraph::new();
    let result = diff(&baseline(), &right);

    assert_eq!(result.removed.len(), 1);
    assert!(result.added.is_empty());
    assert!(
        result.removed[0]
            .identity
            .source
            .as_str()
            .contains("handler")
    );
}

// ── Changed ─────────────────────────────────────────────────────────────

#[test]
fn a_confidence_difference_is_a_change() {
    let mut b = Builder::new();
    b.edge(
        (NodeKind::Function, "handler", "api.py"),
        (NodeKind::Function, "helper", "util.py"),
        EdgeKind::Call,
        0.5,
        Provenance::StaticImportResolution,
        "api.py",
        10,
        "helper() in handler",
    );

    let result = diff(&baseline(), &b.graph);

    assert_eq!(result.changed.len(), 1);
    assert!(result.added.is_empty() && result.removed.is_empty());
    assert_eq!(result.changed[0].fields, vec![ChangedField::Confidence]);
    assert!((result.changed[0].before.confidence - 0.9).abs() < f32::EPSILON);
    assert!((result.changed[0].after.confidence - 0.5).abs() < f32::EPSILON);
}

#[test]
fn a_provenance_difference_is_a_change() {
    let mut b = Builder::new();
    b.edge(
        (NodeKind::Function, "handler", "api.py"),
        (NodeKind::Function, "helper", "util.py"),
        EdgeKind::Call,
        0.9,
        Provenance::RouteMatcher,
        "api.py",
        10,
        "helper() in handler",
    );

    let result = diff(&baseline(), &b.graph);

    assert_eq!(result.changed.len(), 1);
    assert_eq!(result.changed[0].fields, vec![ChangedField::Provenance]);
}

#[test]
fn observing_the_claim_in_a_different_file_is_a_change() {
    let mut b = Builder::new();
    b.edge(
        (NodeKind::Function, "handler", "api.py"),
        (NodeKind::Function, "helper", "util.py"),
        EdgeKind::Call,
        0.9,
        Provenance::StaticImportResolution,
        "routes.py",
        10,
        "helper() in handler",
    );

    let result = diff(&baseline(), &b.graph);

    assert_eq!(result.changed.len(), 1);
    assert_eq!(result.changed[0].fields, vec![ChangedField::File]);
}

/// The judgement call in the module docs, pinned: a line shift is the same
/// observation at a new offset, not a different one.
#[test]
fn a_line_shift_alone_is_not_a_change() {
    let mut b = Builder::new();
    b.edge(
        (NodeKind::Function, "handler", "api.py"),
        (NodeKind::Function, "helper", "util.py"),
        EdgeKind::Call,
        0.9,
        Provenance::StaticImportResolution,
        "api.py",
        512,
        "helper() in handler",
    );

    let result = diff(&baseline(), &b.graph);

    assert!(
        result.is_empty(),
        "moving the observation down the file is not a change: {:#?}",
        result.changed
    );
    assert_eq!(result.unchanged, 1);
}

#[test]
fn several_differing_fields_are_all_reported() {
    let mut b = Builder::new();
    b.edge(
        (NodeKind::Function, "handler", "api.py"),
        (NodeKind::Function, "helper", "util.py"),
        EdgeKind::Call,
        0.4,
        Provenance::RouteMatcher,
        "routes.py",
        10,
        "helper() in handler",
    );

    let result = diff(&baseline(), &b.graph);

    assert_eq!(
        result.changed[0].fields,
        vec![
            ChangedField::Confidence,
            ChangedField::Provenance,
            ChangedField::File
        ]
    );
}

// ── Endpoint and kind changes are remove + add ──────────────────────────

#[test]
fn a_different_target_is_a_removal_and_an_addition() {
    let mut b = Builder::new();
    b.edge(
        (NodeKind::Function, "handler", "api.py"),
        (NodeKind::Function, "other", "util.py"),
        EdgeKind::Call,
        0.9,
        Provenance::StaticImportResolution,
        "api.py",
        10,
        "other() in handler",
    );

    let result = diff(&baseline(), &b.graph);

    assert_eq!(result.added.len(), 1);
    assert_eq!(result.removed.len(), 1);
    assert!(
        result.changed.is_empty(),
        "a different endpoint is a different claim, not an adjusted one"
    );
}

#[test]
fn a_different_source_is_a_removal_and_an_addition() {
    let mut b = Builder::new();
    b.edge(
        (NodeKind::Function, "other_handler", "api.py"),
        (NodeKind::Function, "helper", "util.py"),
        EdgeKind::Call,
        0.9,
        Provenance::StaticImportResolution,
        "api.py",
        10,
        "helper() in other_handler",
    );

    let result = diff(&baseline(), &b.graph);

    assert_eq!(result.added.len(), 1);
    assert_eq!(result.removed.len(), 1);
    assert!(result.changed.is_empty());
}

#[test]
fn a_different_edge_kind_is_a_removal_and_an_addition() {
    let mut b = Builder::new();
    b.edge(
        (NodeKind::Function, "handler", "api.py"),
        (NodeKind::Function, "helper", "util.py"),
        EdgeKind::References,
        0.9,
        Provenance::StaticImportResolution,
        "api.py",
        10,
        "helper() in handler",
    );

    let result = diff(&baseline(), &b.graph);

    assert_eq!(result.added.len(), 1);
    assert_eq!(result.removed.len(), 1);
    assert!(result.changed.is_empty());
    assert_eq!(result.removed[0].identity.kind, "call");
    assert_eq!(result.added[0].identity.kind, "references");
}

// ── Evidence ────────────────────────────────────────────────────────────

/// Every reported edge must be able to explain itself: a diff entry with no
/// evidence is a claim the reader cannot check.
#[test]
fn every_reported_edge_carries_resolvable_evidence() {
    let mut b = Builder::new();
    b.edge(
        (NodeKind::Function, "handler", "api.py"),
        (NodeKind::Function, "helper", "util.py"),
        EdgeKind::Call,
        0.5,
        Provenance::StaticImportResolution,
        "api.py",
        10,
        "helper() in handler",
    );
    b.edge(
        (NodeKind::Function, "handler", "api.py"),
        (NodeKind::Class, "Order", "models.py"),
        EdgeKind::OrmAccess,
        0.8,
        Provenance::OrmResolution,
        "api.py",
        14,
        "Order.objects in handler",
    );

    let result = diff(&baseline(), &b.graph);
    assert!(!result.is_empty(), "precondition: something must differ");

    for appearance in result.added.iter().chain(result.removed.iter()) {
        assert!(!appearance.facts.evidence.trim().is_empty());
        assert!(!appearance.facts.file.trim().is_empty());
        assert!(appearance.facts.line >= 1);
        assert!((0.0..=1.0).contains(&appearance.facts.confidence));
    }
    for change in &result.changed {
        for facts in [&change.before, &change.after] {
            assert!(!facts.evidence.trim().is_empty());
            assert!(!facts.file.trim().is_empty());
        }
        assert!(!change.fields.is_empty(), "a change must say what changed");
    }
}

// ── Determinism ─────────────────────────────────────────────────────────

#[test]
fn the_same_pair_of_graphs_always_diffs_identically() {
    let mut b = Builder::new();
    for (name, confidence) in [("a", 0.1_f32), ("b", 0.2), ("c", 0.3)] {
        b.edge(
            (NodeKind::Function, "handler", "api.py"),
            (NodeKind::Function, name, "util.py"),
            EdgeKind::Call,
            confidence,
            Provenance::StaticImportResolution,
            "api.py",
            10,
            "call",
        );
    }
    let right = b.graph;
    let left = baseline();

    let first = diff(&left, &right);
    for _ in 0..8 {
        assert_eq!(diff(&left, &right), first, "the diff changed between runs");
    }
}

#[test]
fn results_are_ordered_by_identity() {
    let mut b = Builder::new();
    for name in ["zeta", "alpha", "mu"] {
        b.edge(
            (NodeKind::Function, "handler", "api.py"),
            (NodeKind::Function, name, "util.py"),
            EdgeKind::Call,
            0.9,
            Provenance::StaticImportResolution,
            "api.py",
            10,
            "call",
        );
    }

    let empty = ArchitectureGraph::new();
    let result = diff(&empty, &b.graph);

    let targets: Vec<&str> = result
        .added
        .iter()
        .map(|a| a.identity.target.as_str())
        .collect();
    let mut sorted = targets.clone();
    sorted.sort_unstable();
    assert_eq!(
        targets, sorted,
        "added edges must be emitted in a stable order"
    );
}

// ── Parallel edges ──────────────────────────────────────────────────────

/// Two edges asserting the same relationship differ only in evidence. The
/// identical pair must match as unchanged rather than be mistaken for a change
/// against the other.
#[test]
fn parallel_edges_pair_off_before_anything_is_called_changed() {
    let mut left = Builder::new();
    let mut right = Builder::new();
    for builder in [&mut left, &mut right] {
        builder.edge(
            (NodeKind::Function, "handler", "api.py"),
            (NodeKind::Function, "helper", "util.py"),
            EdgeKind::Call,
            0.9,
            Provenance::StaticImportResolution,
            "api.py",
            10,
            "first call site",
        );
    }
    // Only the right has a second, weaker observation of the same relationship.
    right.edge(
        (NodeKind::Function, "handler", "api.py"),
        (NodeKind::Function, "helper", "util.py"),
        EdgeKind::Call,
        0.4,
        Provenance::StaticImportResolution,
        "api.py",
        20,
        "second call site",
    );

    let result = diff(&left.graph, &right.graph);

    assert_eq!(
        result.unchanged, 1,
        "the identical observation must pair off"
    );
    assert_eq!(
        result.added.len(),
        1,
        "the extra observation is an addition"
    );
    assert!(
        result.changed.is_empty(),
        "an extra parallel edge is not a change to the existing one"
    );
}

// ── Degenerate inputs ───────────────────────────────────────────────────

#[test]
fn two_empty_graphs_diff_to_nothing() {
    let left = ArchitectureGraph::new();
    let right = ArchitectureGraph::new();

    let result = diff(&left, &right);

    assert!(result.is_empty());
    assert_eq!(result.unchanged, 0);
}

#[test]
fn a_graph_with_nodes_but_no_edges_diffs_to_nothing() {
    let mut left = ArchitectureGraph::new();
    left.node_for(
        NodeKind::Function,
        "lonely",
        Some(SourceLocation::new("a.py", 1).expect("location")),
    )
    .expect("node");
    let right = ArchitectureGraph::new();

    // Nodes alone assert no relationship, so there is nothing to report.
    assert!(diff(&left, &right).is_empty());
}
