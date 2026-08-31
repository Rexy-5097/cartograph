//! Layout cost as the graph grows, up to the M11 target size.
//!
//! # What this measures, and what it does not
//!
//! M11's acceptance criterion is *"10k-node graph at 60 FPS on the dev
//! machine"*. That is a statement about the finished application: it includes
//! WebGL rendering, the Tauri boundary and React, none of which exist yet.
//! **Nothing here can establish it**, and nothing here should be quoted as
//! though it did.
//!
//! What this establishes is the other half — how long the Rust core takes to
//! produce coordinates for a graph of a given size. Layout runs once per
//! analysis, not once per frame, so its budget is a user-visible pause rather
//! than a frame time. Recording it now means the later rendering measurement
//! can be attributed: if the application misses 60 FPS, this says whether
//! layout was implicated.
//!
//! # The shape of the input matters more than the size
//!
//! Relaxation is quadratic *within a cluster* and does no work between them,
//! so a graph's cost depends on how its nodes are distributed across
//! directories, not only on how many there are. Both cases are measured: a
//! realistic spread, and the pathological single-directory graph that is the
//! true upper bound.

#![allow(missing_docs)]

use cartograph_core::{Confidence, EdgeKind, Evidence, NodeKind, Provenance, SourceLocation};
use cartograph_graph::layout::{LayoutParams, layout};
use cartograph_graph::{ArchitectureGraph, EdgeSpec};
use cartograph_testkit::fixed_clock;
use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};

/// Builds a graph of `nodes` functions spread over `directories` directories,
/// chained so that every node has at least one edge.
fn synthetic(nodes: usize, directories: usize) -> ArchitectureGraph {
    let mut graph = ArchitectureGraph::with_clock(fixed_clock);
    let ids: Vec<_> = (0..nodes)
        .map(|i| {
            let file = format!("pkg{}/module{}.py", i % directories, i % 97);
            let location = SourceLocation::new(file, u32::try_from(i % 5000).unwrap_or(1) + 1)
                .expect("valid location");
            graph
                .add_node(NodeKind::Function, format!("symbol_{i}"), Some(location))
                .expect("valid node")
        })
        .collect();

    // A chain plus a long-range edge, so clusters have internal structure and
    // some edges cross cluster boundaries (which layout must simply ignore).
    for i in 1..nodes {
        let spec = EdgeSpec {
            source: ids[i - 1],
            target: ids[i],
            kind: EdgeKind::Call,
            confidence: Confidence::new(0.9).expect("valid confidence"),
            provenance: Provenance::RouteMatcher,
            evidence: Evidence::new("synthetic edge").expect("non-empty evidence"),
            location: SourceLocation::new("pkg0/module0.py", 1).expect("valid location"),
            commit: None,
        };
        graph.add_edge(spec).expect("endpoints exist");
    }
    graph
}

/// Layout cost against node count, at a realistic directory spread.
///
/// Roughly one directory per hundred nodes, which is the order of magnitude
/// real repositories show.
fn by_size(c: &mut Criterion) {
    let mut group = c.benchmark_group("layout_by_size");
    for nodes in [100_usize, 1_000, 10_000] {
        let directories = (nodes / 100).max(1);
        let graph = synthetic(nodes, directories);
        let params = LayoutParams::default();
        group.bench_with_input(BenchmarkId::from_parameter(nodes), &nodes, |b, _| {
            b.iter(|| layout(&graph, &params));
        });
    }
    group.finish();
}

/// The same node count concentrated into ever fewer directories.
///
/// This is the honest upper bound: relaxation is quadratic within a cluster, so
/// a thousand nodes in one directory costs far more than a thousand spread over
/// fifty, and a repository that keeps everything in one package is the case
/// that would surprise someone reading only the size curve above.
fn by_concentration(c: &mut Criterion) {
    let mut group = c.benchmark_group("layout_by_concentration");
    for directories in [200_usize, 50, 10, 1] {
        let graph = synthetic(2_000, directories);
        let params = LayoutParams::default();
        group.bench_with_input(
            BenchmarkId::from_parameter(directories),
            &directories,
            |b, _| b.iter(|| layout(&graph, &params)),
        );
    }
    group.finish();
}

criterion_group!(benches, by_size, by_concentration);
criterion_main!(benches);
