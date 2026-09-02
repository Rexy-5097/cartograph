//! Cross-run node identity over a real analysed repository.
//!
//! ADR-0014's gate point 2 asks that *two independent analyses of the same
//! commit assign the same identity to every node — asserted on a real
//! repository, not a fixture alone*. That word "independent" is the whole
//! point: a fixture built once and read twice proves nothing about a pipeline
//! whose file discovery, parallelism and hash-map iteration are free to differ
//! between runs. So each run here is a separate `pipeline::run` over the same
//! checkout, and the two graphs are compared by identity alone.
//!
//! Read-only, and opt-in because the corpora are not vendored:
//!
//! ```text
//! CARTOGRAPH_IDENTITY_REPO=<path> cargo test -p cartograph-pipeline \
//!     --release --test identity_real_repository -- --ignored --nocapture
//! ```

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use cartograph_core::{NodeIdentity, identity_of};
use cartograph_pipeline::pipeline::{self, Analysis, Options};

/// One analysis of the corpus named by `CARTOGRAPH_IDENTITY_REPO`.
fn analyse() -> Option<Analysis> {
    let path = std::env::var("CARTOGRAPH_IDENTITY_REPO").ok()?;
    Some(pipeline::run(Path::new(&path), Options { quiet: true }).expect("the corpus analyses"))
}

/// Every node's identity, and the node id it happened to receive.
fn identities(analysis: &Analysis) -> BTreeMap<NodeIdentity, u64> {
    let mut out = BTreeMap::new();
    for node in analysis.graph.nodes() {
        out.insert(identity_of(node), node.id().as_u64());
    }
    out
}

/// Gate point 2 — two independent analyses agree on every node.
#[test]
#[ignore = "needs CARTOGRAPH_IDENTITY_REPO; the pinned corpora are not vendored"]
fn two_independent_analyses_assign_the_same_identity_to_every_node() {
    let Some(first) = analyse() else {
        return;
    };
    let second = analyse().expect("the second analysis runs");

    assert!(
        first.graph.node_count() > 0,
        "precondition: the corpus must analyse"
    );

    let left = identities(&first);
    let right = identities(&second);

    let left_set: BTreeSet<&NodeIdentity> = left.keys().collect();
    let right_set: BTreeSet<&NodeIdentity> = right.keys().collect();

    let only_left: Vec<_> = left_set.difference(&right_set).take(10).collect();
    let only_right: Vec<_> = right_set.difference(&left_set).take(10).collect();

    println!(
        "corpus: {} nodes / {} edges; run 1 identities {}, run 2 identities {}",
        first.graph.node_count(),
        first.graph.edge_count(),
        left.len(),
        right.len()
    );
    println!(
        "mismatches: {} only in run 1, {} only in run 2",
        left_set.difference(&right_set).count(),
        right_set.difference(&left_set).count()
    );

    assert!(
        only_left.is_empty() && only_right.is_empty(),
        "identity is not stable across independent analyses\n  only in run 1: {only_left:#?}\n  only in run 2: {only_right:#?}"
    );

    // Every node accounted for, not merely the sets agreeing in aggregate:
    // `node_for` deduplicates on the same three fields identity uses, so one
    // identity per node is the invariant, and a shortfall would mean identity
    // is coarser than the graph it describes.
    assert_eq!(
        left.len(),
        first.graph.node_count(),
        "identities collapsed distinct nodes in run 1"
    );
    assert_eq!(
        right.len(),
        second.graph.node_count(),
        "identities collapsed distinct nodes in run 2"
    );
}

/// Identity must not track the handle. If it did, this would pass trivially on
/// a deterministic pipeline and fail the moment discovery order changed.
#[test]
#[ignore = "needs CARTOGRAPH_IDENTITY_REPO; the pinned corpora are not vendored"]
fn identity_does_not_track_the_graph_local_node_id() {
    let Some(analysis) = analyse() else {
        return;
    };

    let mut by_identity: BTreeMap<NodeIdentity, u64> = BTreeMap::new();
    for node in analysis.graph.nodes() {
        by_identity.insert(identity_of(node), node.id().as_u64());
    }

    // Sorting by identity must not reproduce the allocation order; if it did,
    // identity would be a relabelling of `NodeId` rather than a semantic key.
    let by_identity_order: Vec<u64> = by_identity.values().copied().collect();
    let mut allocation_order = by_identity_order.clone();
    allocation_order.sort_unstable();

    println!(
        "{} nodes; identity order equals allocation order: {}",
        by_identity_order.len(),
        by_identity_order == allocation_order
    );

    assert_ne!(
        by_identity_order,
        allocation_order,
        "identity ordering coincides exactly with NodeId ordering across {} nodes, \
         which suggests identity is derived from the handle",
        by_identity_order.len()
    );
}

/// The corpus is read-only and the analyser is pure, so repeating an analysis
/// must reproduce the identities byte for byte.
#[test]
#[ignore = "needs CARTOGRAPH_IDENTITY_REPO; the pinned corpora are not vendored"]
fn repeated_analysis_reproduces_the_identity_set_exactly() {
    let Some(first) = analyse() else {
        return;
    };
    let second = analyse().expect("the second analysis runs");

    let left: Vec<String> = identities(&first)
        .keys()
        .map(|i| i.as_str().to_owned())
        .collect();
    let right: Vec<String> = identities(&second)
        .keys()
        .map(|i| i.as_str().to_owned())
        .collect();

    assert_eq!(left, right, "the identity sequence changed between runs");
    println!("{} identities identical across two runs", left.len());
}
