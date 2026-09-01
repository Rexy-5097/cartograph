//! The render scene: totality, verbatim coordinates, and a pinned payload.
//!
//! The property under test is narrow and load-bearing: **the scene is derived
//! from the Rust layout and adds nothing to it**. If a coordinate can change on
//! its way to the renderer, then ADR-0001's claim that layout is computed in
//! Rust is no longer true of the thing actually drawn, and the renderer swap
//! ADR-0006 protects stops being contained.
//!
//! Coordinates are compared with `==` rather than a tolerance throughout. That
//! is deliberate and it is the point: a tolerance would pass a frontend-side
//! transform that scaled every position by 1.0000001, which is exactly the
//! defect this file exists to prevent.

// Exact float comparison is the assertion, not an oversight. `clippy::float_cmp`
// is right in general and wrong here: the property under test is that a
// coordinate is *copied*, and a tolerance would pass the exact defect these
// tests exist to catch — a frontend or composition step that scaled every
// position by a factor close to one.
#![allow(clippy::float_cmp)]

use cartograph_core::{Confidence, EdgeKind, Evidence, NodeKind, Provenance, SourceLocation};
use cartograph_desktop::scene::compose;
use cartograph_graph::layout::{LayoutParams, layout};
use cartograph_graph::{ArchitectureGraph, EdgeSpec};
use cartograph_testkit::fixed_clock;

type Spec<'a> = (NodeKind, &'a str, Option<(&'a str, u32)>);

fn graph_of(nodes: &[Spec<'_>], edges: &[(usize, usize, EdgeKind)]) -> ArchitectureGraph {
    let mut graph = ArchitectureGraph::with_clock(fixed_clock);
    let ids: Vec<_> = nodes
        .iter()
        .map(|(kind, name, location)| {
            let location =
                location.map(|(f, l)| SourceLocation::new(f, l).expect("valid location"));
            graph.add_node(*kind, *name, location).expect("valid node")
        })
        .collect();
    for (source, target, kind) in edges {
        graph
            .add_edge(EdgeSpec {
                source: ids[*source],
                target: ids[*target],
                kind: *kind,
                confidence: Confidence::new(0.9).expect("valid confidence"),
                provenance: Provenance::RouteMatcher,
                evidence: Evidence::new("test edge").expect("non-empty evidence"),
                location: SourceLocation::new("web/app.ts", 1).expect("valid location"),
                commit: None,
            })
            .expect("endpoints exist");
    }
    graph
}

fn sample() -> ArchitectureGraph {
    graph_of(
        &[
            (
                NodeKind::Function,
                "checkout",
                Some(("web/pages/cart.ts", 12)),
            ),
            (NodeKind::Function, "client", Some(("web/lib/api.ts", 3))),
            (NodeKind::Route, "/orders", Some(("api/routes.py", 8))),
            (
                NodeKind::Function,
                "create_order",
                Some(("api/routes.py", 20)),
            ),
            (NodeKind::Class, "Order", Some(("api/models.py", 5))),
            (NodeKind::Table, "orders", None),
        ],
        &[
            (0, 1, EdgeKind::Call),
            (1, 2, EdgeKind::HttpCall),
            (2, 3, EdgeKind::Call),
            (3, 4, EdgeKind::OrmAccess),
            (4, 5, EdgeKind::Queries),
        ],
    )
}

// ── Totality ────────────────────────────────────────────────────────────

#[test]
fn every_graph_node_becomes_exactly_one_scene_node() {
    let graph = sample();
    let placed = layout(&graph, &LayoutParams::default());
    let scene = compose(&graph, &placed);

    assert_eq!(scene.node_count(), graph.node_count());
    assert_eq!(
        scene.node_count(),
        6,
        "precondition: the fixture has 6 nodes"
    );

    let mut seen = std::collections::BTreeSet::new();
    for node in &scene.nodes {
        assert!(seen.insert(node.id), "{} appears twice", node.id);
        assert!(
            graph.node(node.id).is_some(),
            "{} is not in the graph",
            node.id
        );
    }
    for node in graph.nodes() {
        assert!(seen.contains(&node.id()), "{} was dropped", node.id());
    }
}

#[test]
fn every_graph_edge_becomes_exactly_one_scene_edge_with_its_endpoints() {
    let graph = sample();
    let placed = layout(&graph, &LayoutParams::default());
    let scene = compose(&graph, &placed);

    assert_eq!(scene.edge_count(), graph.edge_count());
    assert_eq!(
        scene.edge_count(),
        5,
        "precondition: the fixture has 5 edges"
    );

    let mut seen = std::collections::BTreeSet::new();
    for edge in &scene.edges {
        assert!(seen.insert(edge.id), "{:?} appears twice", edge.id);
        let original = graph.edge(edge.id).expect("edge is in the graph");
        assert_eq!(edge.source, original.source(), "source was rewritten");
        assert_eq!(edge.target, original.target(), "target was rewritten");
        assert_eq!(edge.kind, original.kind(), "kind was rewritten");
    }
}

#[test]
fn an_empty_graph_composes_to_an_empty_scene() {
    let graph = ArchitectureGraph::with_clock(fixed_clock);
    let placed = layout(&graph, &LayoutParams::default());
    let scene = compose(&graph, &placed);

    assert_eq!(scene.node_count(), 0);
    assert_eq!(scene.edge_count(), 0);
    assert!(scene.clusters.is_empty());
}

// ── The boundary: coordinates are copied, never computed ────────────────

/// The load-bearing assertion of this slice.
#[test]
fn a_scene_copies_layout_coordinates_verbatim() {
    let graph = sample();
    let placed = layout(&graph, &LayoutParams::default());
    let scene = compose(&graph, &placed);

    for node in &scene.nodes {
        let source = placed.node(node.id).expect("the layout positioned it");
        assert!(
            node.x == source.x && node.y == source.y,
            "{} moved between layout and scene: layout ({}, {}), scene ({}, {})",
            node.id,
            source.x,
            source.y,
            node.x,
            node.y
        );
        assert_eq!(node.cluster, source.cluster, "{} changed cluster", node.id);
    }
}

/// A non-default extent produces coordinates far outside `[-1, 1]`. If anything
/// normalised on the way through, this is where it would show.
#[test]
fn a_large_extent_survives_composition_unscaled() {
    let graph = sample();
    let params = LayoutParams {
        extent: 5000.0,
        ..LayoutParams::default()
    };
    let placed = layout(&graph, &params);
    let scene = compose(&graph, &placed);

    let widest = scene
        .nodes
        .iter()
        .fold(0.0_f64, |acc, n| acc.max(n.x.abs()).max(n.y.abs()));
    assert!(
        widest > 1.0,
        "composition appears to have rescaled: widest coordinate {widest}"
    );
    for node in &scene.nodes {
        let source = placed.node(node.id).expect("positioned");
        assert!(node.x == source.x && node.y == source.y);
    }
}

#[test]
fn clusters_are_carried_through_unchanged() {
    let graph = sample();
    let placed = layout(&graph, &LayoutParams::default());
    let scene = compose(&graph, &placed);

    assert_eq!(scene.clusters, placed.clusters);
    for node in &scene.nodes {
        assert!(
            scene.clusters.iter().any(|c| c.id == node.cluster),
            "{} names a cluster absent from the table",
            node.id
        );
    }
}

// ── The payload is pinned ───────────────────────────────────────────────

/// Slice 4 adds the evidence record. Until then the scene must not carry it:
/// shipping confidence, provenance or evidence here would mean every node and
/// edge pays transport for data nothing draws, and the next slice's boundary
/// would already be blurred.
#[test]
fn the_scene_carries_no_analysis_payload() {
    let graph = sample();
    let placed = layout(&graph, &LayoutParams::default());
    let scene = compose(&graph, &placed);

    let node = serde_json::to_value(&scene.nodes[0]).expect("serialises");
    let mut keys: Vec<&str> = node
        .as_object()
        .expect("object")
        .keys()
        .map(String::as_str)
        .collect();
    keys.sort_unstable();
    assert_eq!(keys, vec!["cluster", "id", "kind", "label", "x", "y"]);

    let edge = serde_json::to_value(scene.edges[0]).expect("serialises");
    let mut keys: Vec<&str> = edge
        .as_object()
        .expect("object")
        .keys()
        .map(String::as_str)
        .collect();
    keys.sort_unstable();
    assert_eq!(keys, vec!["id", "kind", "source", "target"]);

    let whole = serde_json::to_string(&scene).expect("serialises");
    for forbidden in [
        "confidence",
        "provenance",
        "evidence",
        "createdAt",
        "commit",
    ] {
        assert!(
            !whole.contains(forbidden),
            "`{forbidden}` reached the render payload; that is Slice 4's data"
        );
    }
}

/// The exact case PART 17 asks for: a node the Rust side places at a known
/// coordinate arrives at the boundary carrying that coordinate. The frontend
/// half of this pair lives in `desktop/src/graph.test.ts`.
#[test]
fn a_known_coordinate_reaches_the_boundary_unchanged() {
    let graph = graph_of(&[(NodeKind::Table, "orders", None)], &[]);
    let placed = layout(&graph, &LayoutParams::default());
    let scene = compose(&graph, &placed);

    let positioned = &placed.nodes[0];
    let drawn = &scene.nodes[0];
    assert_eq!(drawn.id, positioned.id);
    assert!(drawn.x == positioned.x && drawn.y == positioned.y);

    // And it survives the transport the Tauri boundary actually performs.
    let text = serde_json::to_string(&scene).expect("serialises");
    let back: cartograph_desktop::scene::Scene = serde_json::from_str(&text).expect("round-trips");
    assert!(back.nodes[0].x == positioned.x && back.nodes[0].y == positioned.y);
    assert_eq!(back, scene);
}

// ── Adversarial ─────────────────────────────────────────────────────────

/// Composing a graph with a layout computed from a *different* graph must not
/// emit an edge whose endpoint the renderer never received. A dangling endpoint
/// in the frontend is a silent defect; dropping it here is diagnosable.
#[test]
fn an_edge_whose_endpoint_is_absent_from_the_layout_is_dropped() {
    let full = sample();
    let subset = graph_of(
        &[(
            NodeKind::Function,
            "checkout",
            Some(("web/pages/cart.ts", 12)),
        )],
        &[],
    );
    let partial = layout(&subset, &LayoutParams::default());

    let scene = compose(&full, &partial);

    let ids: std::collections::BTreeSet<_> = scene.nodes.iter().map(|n| n.id).collect();
    for edge in &scene.edges {
        assert!(
            ids.contains(&edge.source) && ids.contains(&edge.target),
            "edge {:?} references a node absent from the scene",
            edge.id
        );
    }
}

/// Two artefacts with the same name in different files are different nodes, and
/// must stay different in the scene — collapsing them would silently merge two
/// parts of the system on screen.
#[test]
fn same_named_artefacts_in_different_files_stay_distinct() {
    let graph = graph_of(
        &[
            (NodeKind::Function, "handler", Some(("a/one.py", 1))),
            (NodeKind::Function, "handler", Some(("b/two.py", 1))),
        ],
        &[],
    );
    let placed = layout(&graph, &LayoutParams::default());
    let scene = compose(&graph, &placed);

    assert_eq!(scene.node_count(), 2);
    assert_ne!(scene.nodes[0].id, scene.nodes[1].id);
}
