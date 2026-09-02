//! Mental-map-stable layout: `layout(prev_graph, new_graph, prev_positions)`.
//!
//! The property under test is that a reader who has looked at one revision does
//! not have to rebuild their picture for the next one. Every case therefore
//! builds the two graphs *separately*, and several allocate their node ids in
//! deliberately different orders, because a function that leaned on `NodeId`
//! would otherwise pass by coincidence.

use std::collections::{BTreeMap, BTreeSet};

use cartograph_core::{
    Confidence, EdgeKind, Evidence, NodeKind, Provenance, SourceLocation, identity_of,
};
use cartograph_graph::layout::{Layout, LayoutParams, layout, layout_stable};
use cartograph_graph::{ArchitectureGraph, EdgeSpec};

/// `(kind, name, file)` — everything identity is made of.
type Spec<'a> = (NodeKind, &'a str, &'a str);

fn build(nodes: &[Spec<'_>], edges: &[(usize, usize)]) -> ArchitectureGraph {
    let mut graph = ArchitectureGraph::new();
    let ids: Vec<_> = nodes
        .iter()
        .map(|(kind, name, file)| {
            graph
                .node_for(
                    *kind,
                    *name,
                    Some(SourceLocation::new(*file, 1).expect("location")),
                )
                .expect("node")
        })
        .collect();
    for (from, to) in edges {
        graph
            .add_edge(EdgeSpec {
                source: ids[*from],
                target: ids[*to],
                kind: EdgeKind::Call,
                confidence: Confidence::new(0.9).expect("confidence"),
                provenance: Provenance::StaticImportResolution,
                evidence: Evidence::new("call").expect("evidence"),
                location: SourceLocation::new("a/one.py", 1).expect("location"),
                commit: None,
            })
            .expect("edge");
    }
    graph
}

fn params() -> LayoutParams {
    LayoutParams::default()
}

/// Positions keyed by identity, which is how a reader compares two revisions.
fn by_identity(graph: &ArchitectureGraph, placed: &Layout) -> BTreeMap<String, (f64, f64)> {
    let mut out = BTreeMap::new();
    for node in graph.nodes() {
        if let Some(p) = placed.node(node.id()) {
            out.insert(identity_of(node).into_string(), (p.x, p.y));
        }
    }
    out
}

/// Four nodes across two directories, with a couple of edges.
fn baseline() -> ArchitectureGraph {
    build(
        &[
            (NodeKind::Function, "alpha", "a/one.py"),
            (NodeKind::Function, "beta", "a/two.py"),
            (NodeKind::Class, "Gamma", "b/three.py"),
            (NodeKind::Function, "delta", "b/four.py"),
        ],
        &[(0, 1), (2, 3)],
    )
}

// ── 1. Identical graph and positions ────────────────────────────────────

#[test]
fn an_identical_revision_leaves_every_position_untouched() {
    let previous = baseline();
    let before = layout(&previous, &params());
    let current = baseline();

    let after = layout_stable(&previous, &current, &before, &params());

    assert_eq!(
        by_identity(&current, &after),
        by_identity(&previous, &before),
        "nothing changed, so nothing may move"
    );
}

// ── 2. Same identities, different NodeIds ───────────────────────────────

/// The property the slice exists for. The second graph allocates its ids in a
/// different order, so a layout that carried `NodeId` across would place the
/// wrong artefacts.
#[test]
fn positions_follow_identity_when_node_ids_differ() {
    let previous = baseline();
    let before = layout(&previous, &params());

    // Same four artefacts, declared in reverse: every id is different.
    let current = build(
        &[
            (NodeKind::Function, "delta", "b/four.py"),
            (NodeKind::Class, "Gamma", "b/three.py"),
            (NodeKind::Function, "beta", "a/two.py"),
            (NodeKind::Function, "alpha", "a/one.py"),
        ],
        &[(3, 2), (1, 0)],
    );

    // Prove the premise: the ids genuinely differ for the same artefacts.
    let previous_ids: BTreeMap<String, u64> = previous
        .nodes()
        .map(|n| (identity_of(n).into_string(), n.id().as_u64()))
        .collect();
    let current_ids: BTreeMap<String, u64> = current
        .nodes()
        .map(|n| (identity_of(n).into_string(), n.id().as_u64()))
        .collect();
    let moved = previous_ids
        .iter()
        .filter(|(identity, id)| current_ids.get(*identity) != Some(id))
        .count();
    assert!(
        moved >= 3,
        "precondition: the ids must differ, otherwise this proves nothing"
    );

    let after = layout_stable(&previous, &current, &before, &params());

    assert_eq!(
        by_identity(&current, &after),
        by_identity(&previous, &before),
        "positions must follow identity, not the handle"
    );
}

// ── 3. Added node ───────────────────────────────────────────────────────

#[test]
fn a_new_node_is_placed_and_the_others_stay_put() {
    let previous = baseline();
    let before = layout(&previous, &params());

    let mut specs: Vec<Spec<'_>> = vec![
        (NodeKind::Function, "alpha", "a/one.py"),
        (NodeKind::Function, "beta", "a/two.py"),
        (NodeKind::Class, "Gamma", "b/three.py"),
        (NodeKind::Function, "delta", "b/four.py"),
        (NodeKind::Function, "epsilon", "a/five.py"),
    ];
    specs.sort_by_key(|s| s.1);
    let current = build(&specs, &[]);

    let after = layout_stable(&previous, &current, &before, &params());
    let old = by_identity(&previous, &before);
    let new = by_identity(&current, &after);

    for (identity, position) in &old {
        assert_eq!(
            new.get(identity),
            Some(position),
            "{identity} moved when a different node was added"
        );
    }
    let added = identity_of(
        current
            .nodes()
            .find(|n| n.name() == "epsilon")
            .expect("epsilon"),
    )
    .into_string();
    let placed = new.get(&added).expect("the new node received a position");
    assert!(placed.0.is_finite() && placed.1.is_finite());
}

// ── 4. Removed node ─────────────────────────────────────────────────────

#[test]
fn a_removed_node_has_no_position_and_the_rest_are_unmoved() {
    let previous = baseline();
    let before = layout(&previous, &params());

    let current = build(
        &[
            (NodeKind::Function, "alpha", "a/one.py"),
            (NodeKind::Function, "beta", "a/two.py"),
            (NodeKind::Class, "Gamma", "b/three.py"),
        ],
        &[(0, 1)],
    );

    let after = layout_stable(&previous, &current, &before, &params());
    let new = by_identity(&current, &after);

    assert_eq!(after.node_count(), current.node_count());
    assert!(
        !new.keys().any(|k| k.contains("delta")),
        "the removed artefact must not be positioned"
    );
    for (identity, position) in by_identity(&previous, &before) {
        if identity.contains("delta") {
            continue;
        }
        assert_eq!(new.get(&identity), Some(&position), "{identity} moved");
    }
}

// ── 5. Unrelated change ─────────────────────────────────────────────────

/// Editing one directory must not disturb another. Under plain `layout` this
/// fails, which is the whole reason this function exists.
#[test]
fn an_unrelated_addition_does_not_disturb_another_cluster() {
    let previous = baseline();
    let before = layout(&previous, &params());

    let current = build(
        &[
            (NodeKind::Function, "alpha", "a/one.py"),
            (NodeKind::Function, "beta", "a/two.py"),
            (NodeKind::Function, "extra", "a/two.py"),
            (NodeKind::Class, "Gamma", "b/three.py"),
            (NodeKind::Function, "delta", "b/four.py"),
        ],
        &[(0, 1), (3, 4)],
    );

    let after = layout_stable(&previous, &current, &before, &params());
    let old = by_identity(&previous, &before);
    let new = by_identity(&current, &after);

    for identity in old.keys().filter(|k| k.contains("b/")) {
        assert_eq!(
            new.get(identity),
            old.get(identity),
            "{identity} is in another directory and must not have moved"
        );
    }
}

// ── 6. Persistent node with no previous position ────────────────────────

#[test]
fn a_persistent_node_missing_from_the_previous_layout_is_placed_afresh() {
    let previous = baseline();
    let full = layout(&previous, &params());

    // A previous layout that simply does not mention `delta`.
    let partial = Layout {
        nodes: full
            .nodes
            .iter()
            .filter(|n| {
                previous
                    .node(n.id)
                    .is_some_and(|node| node.name() != "delta")
            })
            .copied()
            .collect(),
        clusters: full.clusters.clone(),
    };

    let current = baseline();
    let after = layout_stable(&previous, &current, &partial, &params());

    assert_eq!(after.node_count(), current.node_count());
    let new = by_identity(&current, &after);
    let fresh = by_identity(&current, &layout(&current, &params()));
    let delta = new
        .iter()
        .find(|(k, _)| k.contains("delta"))
        .expect("delta positioned")
        .0;
    assert_eq!(
        new.get(delta),
        fresh.get(delta),
        "an unpositioned node falls back to its fresh placement"
    );
}

// ── 7 & 8. Determinism, independent of order and allocation ─────────────

#[test]
fn repeated_runs_are_identical() {
    let previous = baseline();
    let before = layout(&previous, &params());
    let current = baseline();

    let first = layout_stable(&previous, &current, &before, &params());
    for _ in 0..8 {
        let again = layout_stable(&previous, &current, &before, &params());
        assert_eq!(first.nodes, again.nodes);
        assert_eq!(first.clusters, again.clusters);
    }
}

#[test]
fn declaration_order_cannot_change_the_result() {
    let previous = baseline();
    let before = layout(&previous, &params());

    let forward = build(
        &[
            (NodeKind::Function, "alpha", "a/one.py"),
            (NodeKind::Function, "beta", "a/two.py"),
            (NodeKind::Class, "Gamma", "b/three.py"),
        ],
        &[(0, 1)],
    );
    let backward = build(
        &[
            (NodeKind::Class, "Gamma", "b/three.py"),
            (NodeKind::Function, "beta", "a/two.py"),
            (NodeKind::Function, "alpha", "a/one.py"),
        ],
        &[(2, 1)],
    );

    let a = layout_stable(&previous, &forward, &before, &params());
    let b = layout_stable(&previous, &backward, &before, &params());

    assert_eq!(by_identity(&forward, &a), by_identity(&backward, &b));
}

// ── 9. Cluster invariants (ADR-0015) ────────────────────────────────────

#[test]
fn the_result_satisfies_the_layout_contract() {
    let previous = baseline();
    let before = layout(&previous, &params());
    let current = build(
        &[
            (NodeKind::Function, "alpha", "a/one.py"),
            (NodeKind::Class, "Gamma", "b/three.py"),
            (NodeKind::Function, "zeta", "c/six.py"),
        ],
        &[],
    );

    let after = layout_stable(&previous, &current, &before, &params());
    let p = params();

    assert_eq!(after.node_count(), current.node_count());
    let mut seen = BTreeSet::new();
    for placed in &after.nodes {
        assert!(seen.insert(placed.id), "duplicate layout record");
        assert!(current.node(placed.id).is_some(), "foreign node");
        assert!(placed.x.is_finite() && placed.y.is_finite());
        assert!(
            placed.x.abs() <= p.extent + 1e-9 && placed.y.abs() <= p.extent + 1e-9,
            "({}, {}) escaped the extent",
            placed.x,
            placed.y
        );
        assert!(
            after.clusters.iter().any(|c| c.id == placed.cluster),
            "cluster not in the table"
        );
    }
}

/// Cluster ids come from the new graph. A preserved position must not drag a
/// stale cluster handle with it into a renumbered table.
#[test]
fn cluster_ids_come_from_the_new_graph() {
    let previous = baseline();
    let before = layout(&previous, &params());

    // A directory sorting before both existing ones renumbers every cluster.
    let current = build(
        &[
            (NodeKind::Function, "aardvark", "aaa/zero.py"),
            (NodeKind::Function, "alpha", "a/one.py"),
            (NodeKind::Class, "Gamma", "b/three.py"),
        ],
        &[],
    );

    let after = layout_stable(&previous, &current, &before, &params());
    let fresh = layout(&current, &params());

    let stable_clusters: BTreeMap<u64, u32> = after
        .nodes
        .iter()
        .map(|n| (n.id.as_u64(), n.cluster.as_u32()))
        .collect();
    let fresh_clusters: BTreeMap<u64, u32> = fresh
        .nodes
        .iter()
        .map(|n| (n.id.as_u64(), n.cluster.as_u32()))
        .collect();

    assert_eq!(
        stable_clusters, fresh_clusters,
        "cluster assignment must match a fresh layout of the new graph"
    );
    assert_eq!(after.clusters, fresh.clusters);
}

/// A coordinate from a previous layout with a larger extent must not smuggle an
/// out-of-range point into a contract that forbids it.
#[test]
fn a_previous_position_outside_the_extent_is_refused() {
    let previous = baseline();
    let wide = LayoutParams {
        extent: 1000.0,
        ..params()
    };
    let before = layout(&previous, &wide);
    let current = baseline();

    let after = layout_stable(&previous, &current, &before, &params());
    let p = params();

    for placed in &after.nodes {
        assert!(
            placed.x.abs() <= p.extent + 1e-9 && placed.y.abs() <= p.extent + 1e-9,
            "({}, {}) escaped the extent {}",
            placed.x,
            placed.y,
            p.extent
        );
    }
}

// ── 10 & 11. Degenerate inputs ──────────────────────────────────────────

#[test]
fn an_empty_previous_layout_falls_back_to_a_plain_layout() {
    let previous = ArchitectureGraph::new();
    let empty = Layout {
        nodes: Vec::new(),
        clusters: Vec::new(),
    };
    let current = baseline();

    let after = layout_stable(&previous, &current, &empty, &params());

    assert_eq!(
        after.nodes,
        layout(&current, &params()).nodes,
        "with nothing to preserve the result is exactly a fresh layout"
    );
}

#[test]
fn an_empty_new_graph_lays_out_to_nothing() {
    let previous = baseline();
    let before = layout(&previous, &params());
    let current = ArchitectureGraph::new();

    let after = layout_stable(&previous, &current, &before, &params());

    assert_eq!(after.node_count(), 0);
    assert_eq!(after.cluster_count(), 0);
}

#[test]
fn both_graphs_empty_is_an_empty_layout() {
    let empty_graph = ArchitectureGraph::new();
    let empty = Layout {
        nodes: Vec::new(),
        clusters: Vec::new(),
    };

    let after = layout_stable(&empty_graph, &empty_graph, &empty, &params());

    assert_eq!(after.node_count(), 0);
}

// ── 12. No movement without cause ───────────────────────────────────────

/// The headline claim, stated as its own test: across a revision that adds and
/// removes artefacts, every surviving one is exactly where it was — which plain
/// `layout` does not manage, and this asserts the difference rather than
/// assuming it.
#[test]
fn survivors_do_not_move_where_a_plain_layout_would_move_them() {
    let previous = baseline();
    let before = layout(&previous, &params());

    let current = build(
        &[
            (NodeKind::Function, "alpha", "a/one.py"),
            (NodeKind::Function, "beta", "a/two.py"),
            (NodeKind::Function, "newcomer", "a/two.py"),
            (NodeKind::Class, "Gamma", "b/three.py"),
        ],
        &[(0, 1)],
    );

    let old = by_identity(&previous, &before);
    let stable = by_identity(
        &current,
        &layout_stable(&previous, &current, &before, &params()),
    );
    let plain = by_identity(&current, &layout(&current, &params()));

    let survivors: Vec<&String> = old.keys().filter(|k| stable.contains_key(*k)).collect();
    assert!(survivors.len() >= 3, "precondition: artefacts must survive");

    for identity in &survivors {
        assert_eq!(
            stable.get(*identity),
            old.get(*identity),
            "{identity} moved under the stable layout"
        );
    }

    let disturbed = survivors
        .iter()
        .filter(|identity| plain.get(**identity) != old.get(**identity))
        .count();
    assert!(
        disturbed > 0,
        "precondition: a plain layout must actually disturb something, \
         or this test proves nothing about stability"
    );
}
