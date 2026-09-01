//! Blast radius over a real analysed repository.
//!
//! Hand-built graphs establish that the rules are right in the small. They
//! cannot establish that they hold over a tree nobody wrote for the test: a
//! real checkout has cycles nobody intended, models with hundreds of accessors,
//! and cross-language chains whose shape no fixture anticipates.
//!
//! These run over the pinned corpora, **read-only** — the analyser opens files
//! and never writes, and no checkout is mutated. They are opt-in because this
//! repository does not vendor the corpora.
//!
//! ```text
//! CARTOGRAPH_BLAST_REPO=<path> cargo test -p cartograph-pipeline \
//!     --release --test blast_real -- --ignored --nocapture
//! ```
//!
//! The assertions are structural invariants rather than fixed counts: a corpus
//! is pinned but the analyser is not, so pinning "758 dependents" would make
//! this a change-detector. What is asserted is what must be true of *any*
//! blast radius — the target excluded, every route through an allowed edge,
//! confidence drawn from the graph, determinism across runs.

use std::collections::BTreeSet;

use cartograph_graph::blast::{blast_radius, is_dependency};
use cartograph_pipeline::pipeline::{self, Options};

/// Analyses the corpus named by `CARTOGRAPH_BLAST_REPO`, if it is set.
fn corpus() -> Option<pipeline::Analysis> {
    let path = std::env::var("CARTOGRAPH_BLAST_REPO").ok()?;
    Some(
        pipeline::run(std::path::Path::new(&path), Options { quiet: true })
            .expect("the corpus analyses"),
    )
}

/// The blast radius of every node, checked against the invariants that must
/// hold for all of them.
///
/// Exhaustive rather than sampled: a traversal that mishandled one node in a
/// thousand would pass a spot check, and a thousand is the scale here.
#[test]
#[ignore = "needs CARTOGRAPH_BLAST_REPO; the pinned corpora are not vendored"]
fn every_blast_radius_on_a_real_repository_is_well_formed() {
    let Some(analysis) = corpus() else {
        return;
    };
    let graph = &analysis.graph;
    assert!(
        graph.node_count() > 0,
        "precondition: the corpus must analyse"
    );

    let confidences: BTreeSet<u32> = graph
        .edges()
        .map(|e| e.confidence().get().to_bits())
        .collect();

    let mut largest = 0usize;
    let mut deepest = 0u32;

    for node in graph.nodes() {
        let result = blast_radius(graph, node.id());

        assert!(
            result.get(node.id()).is_none(),
            "{} appeared in its own blast radius",
            node.id()
        );

        let mut seen = BTreeSet::new();
        for entry in &result.reached {
            assert!(seen.insert(entry.node), "{} was reported twice", entry.node);
            assert!(
                graph.node(entry.node).is_some(),
                "{} is not in the graph",
                entry.node
            );
            assert!(
                entry.confidence.is_finite() && (0.0..=1.0).contains(&entry.confidence),
                "confidence {} outside [0,1]",
                entry.confidence
            );
            // ADR-0018: every reported value is one that appears on an edge.
            assert!(
                confidences.contains(&entry.confidence.to_bits()),
                "confidence {} was synthesised rather than taken from an edge",
                entry.confidence
            );
            assert!(entry.depth >= 1, "a dependent cannot be zero hops away");

            let via = graph.edge(entry.via).expect("via names a real edge");
            assert!(
                is_dependency(via.kind()),
                "reached through an excluded kind"
            );
            assert_eq!(via.source(), entry.node, "via must leave the reached node");
            assert!(!via.evidence().as_str().is_empty(), "RULE 006");
        }

        largest = largest.max(result.len());
        deepest = deepest.max(result.max_depth());
    }

    println!(
        "corpus: {} nodes, {} edges -> largest blast radius {}, deepest {}",
        graph.node_count(),
        graph.edge_count(),
        largest,
        deepest
    );
    assert!(largest > 0, "a real corpus must contain some dependency");
}

/// The same corpus, analysed twice, must answer identically.
#[test]
#[ignore = "needs CARTOGRAPH_BLAST_REPO; the pinned corpora are not vendored"]
fn a_real_blast_radius_is_deterministic_across_analyses() {
    let Some(first) = corpus() else {
        return;
    };
    let Some(second) = corpus() else {
        return;
    };

    // The node with the largest fan-in, which is the most interesting target
    // and the one most likely to expose an ordering dependency.
    let target = first
        .graph
        .nodes()
        .max_by_key(|n| first.graph.edges_to(n.id()).count())
        .expect("a non-empty graph")
        .id();
    let identity = |g: &cartograph_graph::ArchitectureGraph, id| {
        let n = g.node(id).expect("present");
        format!("{:?}|{}", n.kind(), n.name())
    };
    let name = identity(&first.graph, target);

    let a = blast_radius(&first.graph, target);
    let second_target = second
        .graph
        .nodes()
        .find(|n| identity(&second.graph, n.id()) == name)
        .expect("the same artefact exists in the second analysis")
        .id();
    let b = blast_radius(&second.graph, second_target);

    assert_eq!(a.len(), b.len(), "two analyses disagreed on {name}");
    let names = |g: &cartograph_graph::ArchitectureGraph,
                 r: &cartograph_graph::blast::BlastRadius| {
        r.reached
            .iter()
            .map(|e| identity(g, e.node))
            .collect::<Vec<_>>()
    };
    assert_eq!(
        names(&first.graph, &a),
        names(&second.graph, &b),
        "the reported set or its order differed between analyses"
    );
    let conf = |r: &cartograph_graph::blast::BlastRadius| {
        r.reached
            .iter()
            .map(|e| e.confidence.to_bits())
            .collect::<Vec<_>>()
    };
    assert_eq!(conf(&a), conf(&b), "confidences differed between analyses");

    println!("deterministic on {name}: {} dependents", a.len());
}

/// Reports the shape of the largest blast radii, and how far they cross.
///
/// Measures rather than asserts a number, because the interesting property —
/// does impact actually cross the language boundary? — is a fact about the
/// corpus and the resolver, not something this traversal can guarantee.
#[test]
#[ignore = "needs CARTOGRAPH_BLAST_REPO; measures rather than asserts"]
fn report_the_largest_blast_radii() {
    let Some(analysis) = corpus() else {
        return;
    };
    let graph = &analysis.graph;

    // Extension via `Path`, so the comparison is the one the platform makes
    // rather than a case-sensitive string suffix.
    let language = |id| {
        graph.node(id).and_then(|n| n.location()).map_or("-", |l| {
            match std::path::Path::new(l.file())
                .extension()
                .and_then(std::ffi::OsStr::to_str)
            {
                Some("ts" | "tsx") => "TS",
                Some("py" | "pyi") => "PY",
                _ => "?",
            }
        })
    };

    let mut rows: Vec<_> = graph
        .nodes()
        .map(|n| {
            let r = blast_radius(graph, n.id());
            let ts = r
                .reached
                .iter()
                .filter(|e| language(e.node) == "TS")
                .count();
            let py = r
                .reached
                .iter()
                .filter(|e| language(e.node) == "PY")
                .count();
            (r.len(), ts, py, r.max_depth(), n.id())
        })
        .collect();
    rows.sort_by_key(|row| std::cmp::Reverse(row.0));

    println!(
        "{:>6} {:>5} {:>5} {:>6}  {:<10} name",
        "total", "TS", "PY", "depth", "kind"
    );
    for (total, ts, py, depth, id) in rows.iter().take(10) {
        let n = graph.node(*id).expect("present");
        println!(
            "{total:>6} {ts:>5} {py:>5} {depth:>6}  {:<10} {}",
            format!("{:?}", n.kind()),
            n.name()
        );
    }
    let crossing = rows.iter().filter(|(_, ts, _, _, _)| *ts > 0).count();
    println!("targets whose impact crosses into TypeScript: {crossing}");
}
