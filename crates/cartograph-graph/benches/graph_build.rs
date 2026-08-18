//! Graph construction throughput.
//!
//! # Why this exists at M00
//!
//! Not because graph insertion is slow — it is not, and optimising it now
//! would be premature. It exists so that the *harness* is in place before it
//! is needed: Criterion wired into the workspace, a bench target that CI can
//! run, and a baseline recorded from the first milestone. When M10 changes the
//! graph to support incremental invalidation, the question "did that make
//! construction slower?" should already have an answer waiting.
//!
//! No number produced here is published. The performance targets in the
//! specification are about whole-repository analysis, which does not exist
//! yet. See `docs/benchmarks/README.md`.

// `criterion_group!` expands to an undocumented public function, which the
// workspace-wide `missing_docs` lint rejects. The lint is worth keeping for
// the library crates, so it is relaxed here rather than weakened everywhere.
#![allow(missing_docs)]

use cartograph_core::{EdgeKind, NodeKind, Provenance};
use cartograph_graph::{ArchitectureGraph, EdgeSpec};
use cartograph_testkit::fixed_clock;
use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};

fn build_chain(node_count: usize) -> ArchitectureGraph {
    let mut graph = ArchitectureGraph::with_clock(fixed_clock);

    let nodes: Vec<_> = (0..node_count)
        .map(|i| {
            graph
                .add_node(
                    NodeKind::Function,
                    format!("handler_{i}"),
                    Some(
                        cartograph_core::SourceLocation::new("api/handlers.py", 1)
                            .expect("relative path"),
                    ),
                )
                .expect("valid node")
        })
        .collect();

    for pair in nodes.windows(2) {
        graph
            .add_edge(EdgeSpec {
                source: pair[0],
                target: pair[1],
                kind: EdgeKind::Call,
                confidence: Provenance::LspSymbolResolution.prior_confidence(),
                provenance: Provenance::LspSymbolResolution,
                evidence: cartograph_core::Evidence::new("call site").expect("non-empty"),
                location: cartograph_core::SourceLocation::new("api/handlers.py", 1)
                    .expect("relative path"),
                commit: None,
            })
            .expect("both endpoints exist");
    }

    graph
}

fn construction(c: &mut Criterion) {
    let mut group = c.benchmark_group("graph_build");

    for size in [100_usize, 1_000, 10_000] {
        group.bench_with_input(BenchmarkId::from_parameter(size), &size, |b, &size| {
            b.iter(|| build_chain(size));
        });
    }

    group.finish();
}

fn traversal(c: &mut Criterion) {
    let graph = build_chain(10_000);
    let root = cartograph_core::NodeId::from_raw(0);

    c.bench_function("edges_from_single_node", |b| {
        b.iter(|| graph.edges_from(root).count());
    });
}

criterion_group!(benches, construction, traversal);
criterion_main!(benches);
