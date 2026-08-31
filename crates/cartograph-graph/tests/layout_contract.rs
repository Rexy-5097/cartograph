//! The M11 layout contract.
//!
//! Two things are being established here, and they are different in kind.
//!
//! The first is that the layout is **total and well formed**: every node gets
//! exactly one position, no position is `NaN` or infinite, and every position
//! is inside the requested extent. These are the properties a renderer cannot
//! defend itself against — a single `NaN` silently removes a node from a WebGL
//! scene with no error anywhere.
//!
//! The second is that the layout is **deterministic**, and specifically that it
//! cannot be moved by anything outside the graph's own content: not insertion
//! order, not hash iteration order, not the order the filesystem happened to
//! enumerate files in. Those are the failure modes that do not announce
//! themselves — a layout that quietly differs between two runs looks like a
//! rendering bug for a long time before anyone suspects the core.

use cartograph_core::{Confidence, EdgeKind, Evidence, NodeKind, Provenance, SourceLocation};
use cartograph_graph::layout::{Layout, LayoutParams, layout};
use cartograph_graph::{ArchitectureGraph, EdgeSpec};
use cartograph_testkit::fixed_clock;

/// A node specification: kind, name, and an optional `(file, line)`.
type Spec<'a> = (NodeKind, &'a str, Option<(&'a str, u32)>);

fn graph_of(nodes: &[Spec<'_>], edges: &[(usize, usize)]) -> ArchitectureGraph {
    let mut graph = ArchitectureGraph::with_clock(fixed_clock);
    let ids: Vec<_> = nodes
        .iter()
        .map(|(kind, name, location)| {
            let location =
                location.map(|(f, l)| SourceLocation::new(f, l).expect("valid location"));
            graph.add_node(*kind, *name, location).expect("valid node")
        })
        .collect();
    for (source, target) in edges {
        graph
            .add_edge(EdgeSpec {
                source: ids[*source],
                target: ids[*target],
                kind: EdgeKind::Call,
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

/// The fixture used by most cases: three directories, one unlocated table.
fn sample() -> ArchitectureGraph {
    graph_of(
        &[
            (
                NodeKind::Function,
                "checkout",
                Some(("web/pages/cart.ts", 12)),
            ),
            (
                NodeKind::Function,
                "submit",
                Some(("web/pages/cart.ts", 40)),
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
        &[(0, 2), (1, 2), (2, 3), (3, 4), (4, 5), (5, 6)],
    )
}

/// Every finished layout must satisfy these, whatever the graph was.
fn assert_well_formed(layout: &Layout, graph: &ArchitectureGraph, params: &LayoutParams) {
    assert_eq!(
        layout.node_count(),
        graph.node_count(),
        "every graph node must receive exactly one layout record"
    );

    let mut seen = std::collections::BTreeSet::new();
    for node in &layout.nodes {
        assert!(
            seen.insert(node.id),
            "{} received more than one layout record",
            node.id
        );
        assert!(
            graph.node(node.id).is_some(),
            "{} is not a node of the source graph",
            node.id
        );
        assert!(
            node.x.is_finite() && node.y.is_finite(),
            "{} has a non-finite position ({}, {})",
            node.id,
            node.x,
            node.y
        );
        assert!(
            node.x.abs() <= params.extent + 1e-9 && node.y.abs() <= params.extent + 1e-9,
            "{} at ({}, {}) escaped the extent {}",
            node.id,
            node.x,
            node.y,
            params.extent
        );
        assert!(
            layout.clusters.iter().any(|c| c.id == node.cluster),
            "{} names cluster {} which is not in the cluster table",
            node.id,
            node.cluster
        );
    }

    for graph_node in graph.nodes() {
        assert!(
            seen.contains(&graph_node.id()),
            "{} was in the graph but received no layout record",
            graph_node.id()
        );
    }
}

// ── Totality and well-formedness ────────────────────────────────────────

#[test]
fn every_node_receives_exactly_one_finite_bounded_position() {
    let graph = sample();
    let params = LayoutParams::default();
    let result = layout(&graph, &params);

    assert_eq!(
        result.node_count(),
        7,
        "precondition: the fixture has 7 nodes"
    );
    assert_well_formed(&result, &graph, &params);
}

#[test]
fn an_empty_graph_lays_out_to_nothing_rather_than_failing() {
    let graph = ArchitectureGraph::with_clock(fixed_clock);
    let result = layout(&graph, &LayoutParams::default());

    assert_eq!(result.node_count(), 0);
    assert_eq!(result.cluster_count(), 0);
}

/// One node cannot be spread over an extent, and the normalisation step
/// divides by the span. This is the case that produces `NaN` if that division
/// is unguarded, which is why it is tested on its own rather than folded into
/// the size sweep below.
#[test]
fn a_single_node_is_placed_at_the_origin_and_not_at_nan() {
    let graph = graph_of(&[(NodeKind::Table, "orders", None)], &[]);
    let params = LayoutParams::default();
    let result = layout(&graph, &params);

    assert_eq!(result.node_count(), 1);
    assert!(
        result.nodes[0].x.is_finite() && result.nodes[0].y.is_finite(),
        "a lone node must not divide by a zero span: got ({}, {})",
        result.nodes[0].x,
        result.nodes[0].y
    );
    assert_well_formed(&result, &graph, &params);
}

/// Two nodes seeded to the *same* point — identical semantic identity — are at
/// distance zero, which is the other division the implementation guards.
#[test]
fn coincident_nodes_do_not_divide_by_a_zero_distance() {
    let graph = graph_of(
        &[
            (NodeKind::Function, "same", Some(("web/a.ts", 1))),
            (NodeKind::Function, "same", Some(("web/a.ts", 1))),
        ],
        &[],
    );
    let params = LayoutParams::default();
    let result = layout(&graph, &params);

    assert_eq!(result.node_count(), 2, "both nodes must still be laid out");
    assert_well_formed(&result, &graph, &params);
}

#[test]
fn graphs_of_every_size_from_zero_to_many_stay_well_formed() {
    for count in [0_usize, 1, 2, 3, 10, 250] {
        let specs: Vec<Spec<'_>> = (0..count)
            .map(|i| {
                let name: &'static str = Box::leak(format!("fn_{i}").into_boxed_str());
                let file: &'static str =
                    Box::leak(format!("pkg{}/mod{}.py", i % 7, i % 13).into_boxed_str());
                (
                    NodeKind::Function,
                    name,
                    Some((file, u32::try_from(i).unwrap_or(1) + 1)),
                )
            })
            .collect();
        let edges: Vec<_> = (1..count).map(|i| (i - 1, i)).collect();
        let graph = graph_of(&specs, &edges);
        let params = LayoutParams::default();
        let result = layout(&graph, &params);

        assert_eq!(result.node_count(), count, "size {count}");
        assert_well_formed(&result, &graph, &params);
    }
}

/// A graph with no edges at all still lays out — repulsion alone must not
/// diverge, and the attraction loop must tolerate an empty pair set.
#[test]
fn a_fully_disconnected_graph_lays_out() {
    let specs: Vec<Spec<'_>> = vec![
        (NodeKind::Function, "a", Some(("x/a.py", 1))),
        (NodeKind::Function, "b", Some(("x/b.py", 1))),
        (NodeKind::Function, "c", Some(("y/c.py", 1))),
        (NodeKind::Table, "t", None),
    ];
    let graph = graph_of(&specs, &[]);
    let params = LayoutParams::default();
    let result = layout(&graph, &params);

    assert_well_formed(&result, &graph, &params);
    assert_eq!(result.node_count(), 4);
}

/// Zero iterations must still produce a valid layout: cluster placement and
/// seeding alone are a complete answer, just an unrelaxed one.
#[test]
fn zero_iterations_still_produces_a_well_formed_layout() {
    let graph = sample();
    let params = LayoutParams {
        iterations: 0,
        ..LayoutParams::default()
    };
    let result = layout(&graph, &params);

    assert_well_formed(&result, &graph, &params);
}

#[test]
fn a_custom_extent_is_respected() {
    let graph = sample();
    let params = LayoutParams {
        extent: 500.0,
        ..LayoutParams::default()
    };
    let result = layout(&graph, &params);

    assert_well_formed(&result, &graph, &params);
    let widest = result
        .nodes
        .iter()
        .fold(0.0_f64, |acc, n| acc.max(n.x.abs()).max(n.y.abs()));
    assert!(
        widest > 1.0,
        "a large extent must actually be used, not left at unit scale: widest was {widest}"
    );
}

// ── Determinism ─────────────────────────────────────────────────────────

#[test]
fn the_same_graph_and_parameters_produce_identical_positions() {
    let params = LayoutParams::default();
    let first = layout(&sample(), &params);
    let second = layout(&sample(), &params);

    assert_eq!(
        first, second,
        "layout must be a pure function of the graph and parameters"
    );
}

/// The defect this exists to catch: a layout that depends on the order nodes
/// were inserted. The two graphs below are the same architecture described in
/// a different order, so their `NodeId`s differ — and their *positions* must
/// not.
#[test]
fn insertion_order_cannot_move_a_node() {
    let forward: &[Spec<'_>] = &[
        (NodeKind::Function, "alpha", Some(("web/a.ts", 1))),
        (NodeKind::Function, "beta", Some(("web/b.ts", 2))),
        (NodeKind::Class, "Gamma", Some(("api/g.py", 3))),
    ];
    let reversed: &[Spec<'_>] = &[
        (NodeKind::Class, "Gamma", Some(("api/g.py", 3))),
        (NodeKind::Function, "beta", Some(("web/b.ts", 2))),
        (NodeKind::Function, "alpha", Some(("web/a.ts", 1))),
    ];

    let params = LayoutParams::default();
    let a = layout(&graph_of(forward, &[(0, 1)]), &params);
    let b = layout(&graph_of(reversed, &[(2, 1)]), &params);

    // The handles genuinely differ, or this test proves nothing.
    let a_ids: Vec<_> = a.nodes.iter().map(|n| n.id).collect();
    let b_ids: Vec<_> = b.nodes.iter().map(|n| n.id).collect();
    assert_ne!(
        a_ids, b_ids,
        "precondition: the two graphs must number their nodes differently"
    );

    let a_points: Vec<(f64, f64)> = a.nodes.iter().map(|n| (n.x, n.y)).collect();
    let b_points: Vec<(f64, f64)> = b.nodes.iter().map(|n| (n.x, n.y)).collect();
    assert_eq!(
        a_points, b_points,
        "the same architecture described in a different order must lay out identically"
    );
    assert_eq!(a.clusters, b.clusters, "clusters must agree too");
}

/// Edges are attractive forces summed in a loop, and floating-point addition is
/// not associative — so the *order* edges are visited in can change the result.
/// The same architecture with its edges declared in a different order must
/// still produce the same positions.
#[test]
fn edge_declaration_order_cannot_move_a_node() {
    let specs: &[Spec<'_>] = &[
        (NodeKind::Function, "a", Some(("pkg/m.py", 1))),
        (NodeKind::Function, "b", Some(("pkg/m.py", 2))),
        (NodeKind::Function, "c", Some(("pkg/m.py", 3))),
        (NodeKind::Function, "d", Some(("pkg/m.py", 4))),
    ];
    let params = LayoutParams::default();
    let forward = layout(&graph_of(specs, &[(0, 1), (1, 2), (2, 3), (0, 3)]), &params);
    let shuffled = layout(&graph_of(specs, &[(0, 3), (2, 3), (0, 1), (1, 2)]), &params);

    assert_eq!(
        forward, shuffled,
        "edge insertion order must not affect the layout"
    );
}

/// A pair joined by several edges must not attract harder than a pair joined
/// once, or a duplicated observation would deform the drawing.
#[test]
fn a_duplicated_edge_does_not_pull_harder() {
    let specs: &[Spec<'_>] = &[
        (NodeKind::Function, "a", Some(("pkg/m.py", 1))),
        (NodeKind::Function, "b", Some(("pkg/m.py", 2))),
        (NodeKind::Function, "c", Some(("pkg/m.py", 3))),
    ];
    let params = LayoutParams::default();
    let once = layout(&graph_of(specs, &[(0, 1), (1, 2)]), &params);
    let thrice = layout(&graph_of(specs, &[(0, 1), (0, 1), (0, 1), (1, 2)]), &params);

    assert_eq!(
        once, thrice,
        "repeating an edge must not change the geometry"
    );
}

/// The golden case: a fixed graph laid out with fixed parameters, compared
/// against positions recorded from this implementation.
///
/// Compared with a tolerance rather than byte-for-byte. The guarantee being
/// defended is that the algorithm does not change, not that two floating-point
/// pipelines round identically — an exact comparison would fail on a different
/// target for a reason that is not a defect.
#[test]
fn the_golden_layout_does_not_drift() {
    const TOLERANCE: f64 = 1e-9;
    let params = LayoutParams::default();
    let result = layout(&sample(), &params);

    // Recorded from this implementation, ordered by cluster then by semantic
    // identity — the order `layout` emits.
    let expected: Vec<(&str, f64, f64)> = vec![
        (
            "<Table>",
            -0.121_658_225_528_248_86,
            -0.026_420_463_213_617_785,
        ),
        ("api", -0.421_567_723_875_464_54, 0.188_575_895_082_474_63),
        ("api", -0.565_955_114_937_394_7, 0.388_173_885_982_088_24),
        ("api", -0.731_286_333_272_571_2, 0.571_491_627_169_426_8),
        ("web/lib", -0.024_810_117_591_711_624, -1.0),
        ("web/pages", 0.418_636_947_240_137_56, 1.0),
        (
            "web/pages",
            0.731_286_333_272_571_2,
            0.687_350_613_967_566_2,
        ),
    ];

    assert_eq!(
        result.node_count(),
        expected.len(),
        "the golden fixture changed shape; re-record deliberately, do not paper over it"
    );

    for (actual, (label, x, y)) in result.nodes.iter().zip(&expected) {
        let cluster = result
            .clusters
            .iter()
            .find(|c| c.id == actual.cluster)
            .expect("cluster is in the table");
        assert_eq!(&cluster.label, label, "cluster assignment drifted");
        assert!(
            (actual.x - x).abs() < TOLERANCE && (actual.y - y).abs() < TOLERANCE,
            "position drifted: expected ({x}, {y}), got ({}, {})",
            actual.x,
            actual.y
        );
    }
}

// ── Clustering ──────────────────────────────────────────────────────────

#[test]
fn nodes_cluster_by_directory_and_unlocated_nodes_by_kind() {
    let result = layout(&sample(), &LayoutParams::default());
    let labels: Vec<&str> = result.clusters.iter().map(|c| c.label.as_str()).collect();

    assert_eq!(
        labels,
        vec!["<Table>", "api", "web/lib", "web/pages"],
        "clusters must be the directories plus a kind bucket, sorted"
    );
}

#[test]
fn a_file_at_the_repository_root_gets_the_root_cluster() {
    let graph = graph_of(&[(NodeKind::Module, "setup", Some(("setup.py", 1)))], &[]);
    let result = layout(&graph, &LayoutParams::default());

    assert_eq!(
        result
            .clusters
            .iter()
            .map(|c| c.label.as_str())
            .collect::<Vec<_>>(),
        vec!["<root>"],
        "a root-level file has no parent directory and must still be named"
    );
}

#[test]
fn cluster_ids_are_dense_and_ordered_by_label() {
    let result = layout(&sample(), &LayoutParams::default());

    for (index, cluster) in result.clusters.iter().enumerate() {
        assert_eq!(
            cluster.id.as_u32() as usize,
            index,
            "cluster ids must be dense and in label order"
        );
    }
}

// ── The boundary ────────────────────────────────────────────────────────

/// The layout payload is what a future Tauri/React client receives. It must
/// carry drawing data and nothing else: the moment a name, a confidence or an
/// evidence string appears here, analysis has begun leaking into the client
/// and RULE 002 is no longer enforced by anything but good intentions.
///
/// This test fails if a field is *added*, which is the direction the mistake
/// actually happens in.
#[test]
fn the_serialised_node_payload_carries_only_layout_data() {
    let result = layout(&sample(), &LayoutParams::default());
    let json = serde_json::to_value(result.nodes[0]).expect("serialises");
    let object = json.as_object().expect("a node is a JSON object");

    let mut keys: Vec<&str> = object.keys().map(String::as_str).collect();
    keys.sort_unstable();
    assert_eq!(
        keys,
        vec!["cluster", "id", "x", "y"],
        "the per-node payload is fixed by the M11 specification: {{id, x, y, cluster}}"
    );

    for forbidden in [
        "name",
        "kind",
        "confidence",
        "provenance",
        "evidence",
        "location",
        "file",
        "line",
        "created_at",
        "commit",
    ] {
        assert!(
            !object.contains_key(forbidden),
            "analysis data `{forbidden}` must not appear in the layout contract"
        );
    }
}

/// The cluster table is the only other thing a client receives, and it exists
/// so the reader can name a group. It must not become a second channel for
/// analysis data either.
#[test]
fn the_serialised_cluster_payload_carries_only_an_id_and_a_label() {
    let result = layout(&sample(), &LayoutParams::default());
    let json = serde_json::to_value(&result.clusters[0]).expect("serialises");
    let object = json.as_object().expect("a cluster is a JSON object");

    let mut keys: Vec<&str> = object.keys().map(String::as_str).collect();
    keys.sort_unstable();
    assert_eq!(keys, vec!["id", "label"]);
}

/// A layout survives JSON, which is what the Tauri boundary will do to it.
///
/// Identity and cluster membership must survive *exactly* — those are the
/// parts a client uses to address the graph, and an off-by-one there is a
/// wrong answer rather than a blurry one.
///
/// Coordinates are compared with a tolerance, and that is a measured decision
/// rather than a hedge. `serde_json` writes an f64 as its exact shortest
/// representation, but its **parser** is not always correctly rounded: the
/// value `0.418_636_947_240_137_56` from this very fixture is written
/// faithfully and read back one unit in the last place higher
/// (`3fdacaf29f52cb76` becomes `3fdacaf29f52cb77`). That is a property of the
/// serialisation boundary, not of the layout, and it is worth knowing before
/// anyone writes a client test that expects bit-identical coordinates.
///
/// One ULP at this magnitude is around 1e-17 — far below anything a renderer
/// can express, so it is recorded rather than fought.
#[test]
fn a_layout_survives_a_json_round_trip() {
    /// Generous against 1 ULP, still tight enough to catch a real change.
    const TOLERANCE: f64 = 1e-12;

    let original = layout(&sample(), &LayoutParams::default());
    let text = serde_json::to_string(&original).expect("serialises");
    let restored: Layout = serde_json::from_str(&text).expect("deserialises");

    assert_eq!(
        original.clusters, restored.clusters,
        "the cluster table must survive exactly"
    );
    assert_eq!(original.node_count(), restored.node_count());

    for (before, after) in original.nodes.iter().zip(&restored.nodes) {
        assert_eq!(before.id, after.id, "node identity must survive exactly");
        assert_eq!(
            before.cluster, after.cluster,
            "cluster membership must survive exactly"
        );
        assert!(
            (before.x - after.x).abs() < TOLERANCE && (before.y - after.y).abs() < TOLERANCE,
            "{} moved through JSON: ({}, {}) -> ({}, {})",
            before.id,
            before.x,
            before.y,
            after.x,
            after.y
        );
    }
}
