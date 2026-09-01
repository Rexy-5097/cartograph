//! Generates the deterministic 10,000-node scene the renderer benchmark uses.
//!
//! # Why a generated fixture rather than a real repository
//!
//! No repository in the benchmark corpus produces ten thousand graph nodes —
//! zulip, the largest, produces 1,308. The acceptance criterion is about the
//! renderer's behaviour at 10k, so the workload has to be constructed. This is
//! the honest way to do that: build a graph, run it through the **real**
//! `layout::layout` and the **real** `scene::compose`, and write out exactly
//! what the Tauri boundary would send.
//!
//! Nothing here fakes coordinates. If the layout engine changes, this fixture
//! changes with it, which is the point — a benchmark against hand-written
//! positions would measure a scene the product never produces.
//!
//! # Why it is not committed
//!
//! The result is roughly two megabytes of JSON that is entirely derived. It is
//! written to `desktop/public/`, which is git-ignored, and regenerated with:
//!
//! ```text
//! cargo test -p cartograph-desktop --test fixture_10k -- --ignored --nocapture
//! ```
//!
//! # Shape
//!
//! The generated repository is meant to exercise Sigma the way a real one
//! would, not to be maximally adversarial:
//!
//! - **120 directories**, so the layout produces many clusters of uneven size
//!   rather than one blob;
//! - **several node kinds** in realistic proportions, so size and colour vary;
//! - **a chain within each module** plus **cross-module calls**, giving edges
//!   that are mostly local with a minority spanning clusters — the distribution
//!   that makes a force layout's output worth drawing;
//! - **cross-stack edges** (`HttpCall`, `OrmAccess`, `Queries`) so the bright
//!   edge colours are exercised;
//! - **deliberate disconnected components**: the last few directories have no
//!   inbound or outbound edges at all.
//!
//! Everything is derived from the loop index. There is no randomness, seeded or
//! otherwise, so two runs on the same binary produce byte-identical output.

use std::io::Write as _;

use cartograph_core::{Confidence, EdgeKind, Evidence, NodeKind, Provenance, SourceLocation};
use cartograph_desktop::scene::compose;
use cartograph_graph::layout::{LayoutParams, layout};
use cartograph_graph::{ArchitectureGraph, EdgeSpec};
use cartograph_testkit::fixed_clock;

const NODES: usize = 10_000;
const DIRECTORIES: usize = 120;

/// Node kinds in roughly the proportion a real repository shows.
fn kind_for(index: usize) -> NodeKind {
    match index % 20 {
        0 => NodeKind::Class,
        1 => NodeKind::Route,
        2 => NodeKind::Module,
        3 => NodeKind::Table,
        4 => NodeKind::Method,
        _ => NodeKind::Function,
    }
}

fn build() -> ArchitectureGraph {
    let mut graph = ArchitectureGraph::with_clock(fixed_clock);
    let mut ids = Vec::with_capacity(NODES);

    for index in 0..NODES {
        let directory = index % DIRECTORIES;
        let kind = kind_for(index);
        // A table has no source location, exactly as the resolver produces it,
        // so the `<Table>` kind-bucket cluster is exercised too.
        let location = if matches!(kind, NodeKind::Table) {
            None
        } else {
            Some(
                SourceLocation::new(
                    format!("pkg{:03}/module{:02}.py", directory, index % 17),
                    u32::try_from(index % 900).unwrap_or(1) + 1,
                )
                .expect("valid location"),
            )
        };
        ids.push(
            graph
                .add_node(kind, format!("symbol_{index}"), location)
                .expect("valid node"),
        );
    }

    let edge = |graph: &mut ArchitectureGraph, from: usize, to: usize, kind: EdgeKind| {
        graph
            .add_edge(EdgeSpec {
                source: ids[from],
                target: ids[to],
                kind,
                confidence: Confidence::new(0.8).expect("valid confidence"),
                provenance: Provenance::RouteMatcher,
                evidence: Evidence::new("generated benchmark edge").expect("non-empty"),
                location: SourceLocation::new("pkg000/module00.py", 1).expect("valid location"),
                commit: None,
            })
            .expect("endpoints exist");
    };

    // The last four directories are left unconnected on purpose.
    let connected = NODES - (NODES / DIRECTORIES) * 4;

    for index in 0..connected {
        // A local chain: most edges stay inside a cluster.
        if index >= DIRECTORIES {
            edge(&mut graph, index - DIRECTORIES, index, EdgeKind::Call);
        }
        // A minority reach across clusters.
        if index % 11 == 0 {
            edge(
                &mut graph,
                index,
                (index * 7 + 13) % connected,
                EdgeKind::Import,
            );
        }
        // The cross-stack claims, which get the bright colours.
        if index % 23 == 0 {
            edge(
                &mut graph,
                index,
                (index + 137) % connected,
                EdgeKind::HttpCall,
            );
        }
        if index % 29 == 0 {
            edge(
                &mut graph,
                index,
                (index + 401) % connected,
                EdgeKind::OrmAccess,
            );
        }
        if index % 37 == 0 {
            edge(
                &mut graph,
                index,
                (index + 53) % connected,
                EdgeKind::Queries,
            );
        }
    }

    graph
}

#[test]
#[ignore = "writes desktop/public/scene-10k.json; run explicitly"]
fn write_the_ten_thousand_node_scene() {
    let graph = build();
    let placed = layout(&graph, &LayoutParams::default());
    let scene = compose(&graph, &placed);

    assert_eq!(
        scene.node_count(),
        NODES,
        "the fixture must be 10,000 nodes"
    );
    assert_eq!(
        scene.node_count(),
        graph.node_count(),
        "composition must be total"
    );
    for node in &scene.nodes {
        assert!(
            node.x.is_finite() && node.y.is_finite(),
            "a benchmark fixture must not contain a non-finite coordinate"
        );
    }

    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../desktop/public/scene-10k.json");
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).expect("public directory");
    }
    let json = serde_json::to_string(&scene).expect("serialises");
    let mut file = std::fs::File::create(&path).expect("writable");
    file.write_all(json.as_bytes()).expect("written");

    println!(
        "scene-10k.json: {} nodes, {} edges, {} clusters, {} bytes",
        scene.node_count(),
        scene.edge_count(),
        scene.clusters.len(),
        json.len()
    );
}

/// The generator is deterministic: two builds produce the same scene.
///
/// Runs by default and is cheap relative to what it protects — a fixture that
/// drifted between runs would make every FPS comparison meaningless.
#[test]
fn the_generated_scene_is_deterministic() {
    let first = {
        let graph = build();
        let placed = layout(&graph, &LayoutParams::default());
        compose(&graph, &placed)
    };
    let second = {
        let graph = build();
        let placed = layout(&graph, &LayoutParams::default());
        compose(&graph, &placed)
    };

    assert_eq!(first.node_count(), NODES);
    assert_eq!(first, second, "the benchmark fixture must not drift");
}

/// The shape the benchmark depends on: many clusters, real edges, and some
/// genuinely disconnected nodes. Asserted so a change to the generator that
/// quietly made the workload easier would fail here rather than silently
/// improve the measured number.
#[test]
fn the_generated_scene_has_the_shape_the_benchmark_needs() {
    let graph = build();
    let placed = layout(&graph, &LayoutParams::default());
    let scene = compose(&graph, &placed);

    assert_eq!(scene.node_count(), 10_000);
    assert!(
        scene.edge_count() > 9_000,
        "the workload must carry a realistic edge count, got {}",
        scene.edge_count()
    );
    assert!(
        scene.clusters.len() > 100,
        "the workload must span many clusters, got {}",
        scene.clusters.len()
    );

    let with_edges: std::collections::BTreeSet<_> = scene
        .edges
        .iter()
        .flat_map(|e| [e.source, e.target])
        .collect();
    let isolated = scene
        .nodes
        .iter()
        .filter(|n| !with_edges.contains(&n.id))
        .count();
    assert!(
        isolated > 0,
        "the workload must include disconnected nodes; a fully connected graph is the easy case"
    );

    // `NodeKind` is not `Ord`, so the distinct set is counted by formatting.
    let kinds: std::collections::BTreeSet<String> = scene
        .nodes
        .iter()
        .map(|n| format!("{:?}", n.kind))
        .collect();
    assert!(kinds.len() >= 5, "several node kinds must be exercised");
}

/// Writes this repository's own scene as a small, committed fixture.
///
/// The 10k fixture is synthetic by necessity. This one is not: it is whatever
/// the analyser actually finds in the Cartograph checkout, including artefacts
/// with no source location and directories holding a single node. It is small
/// enough to commit, which is what lets the **frontend** test assert that
/// Graphology receives exactly the nodes and edges Rust produced — the two
/// halves of the count check cannot otherwise meet.
#[test]
#[ignore = "writes desktop/src/__fixtures__/scene-repo.json; run explicitly"]
fn write_this_repositorys_scene() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("the repository root");
    let analysis = cartograph_pipeline::pipeline::run(
        &root,
        cartograph_pipeline::pipeline::Options { quiet: true },
    )
    .expect("this repository analyses");
    let placed = layout(&analysis.graph, &LayoutParams::default());
    let scene = compose(&analysis.graph, &placed);

    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../desktop/src/__fixtures__/scene-repo.json");
    std::fs::create_dir_all(path.parent().expect("parent")).expect("fixture directory");
    let json = serde_json::to_string_pretty(&scene).expect("serialises");
    std::fs::write(&path, &json).expect("written");

    println!(
        "scene-repo.json: {} nodes, {} edges, {} clusters",
        scene.node_count(),
        scene.edge_count(),
        scene.clusters.len()
    );
}
