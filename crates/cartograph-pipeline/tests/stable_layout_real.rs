//! Mental-map-stable layout over two real analysed repository states.
//!
//! Hand-built graphs establish the rules in the small. They cannot show what
//! happens across a real revision, where hundreds of artefacts are renumbered
//! by discovery order alone and a handful genuinely appear and disappear. This
//! runs the layout against two separately analysed trees and checks the claim
//! the slice exists to make: what survived did not move.
//!
//! Read-only, and opt-in because the corpora are not vendored:
//!
//! ```text
//! CARTOGRAPH_LAYOUT_BEFORE=<path> CARTOGRAPH_LAYOUT_AFTER=<path> \
//!     cargo test -p cartograph-pipeline --release --test stable_layout_real \
//!     -- --ignored --nocapture
//! ```

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use cartograph_core::identity_of;
use cartograph_graph::ArchitectureGraph;
use cartograph_graph::layout::{Layout, LayoutParams, layout, layout_stable};
use cartograph_pipeline::pipeline::{self, Analysis, Options};

fn analyse(var: &str) -> Option<Analysis> {
    let path = std::env::var(var).ok()?;
    Some(pipeline::run(Path::new(&path), Options { quiet: true }).expect("the corpus analyses"))
}

/// Positions keyed by identity — how a reader compares two revisions.
fn by_identity(graph: &ArchitectureGraph, placed: &Layout) -> BTreeMap<String, (f64, f64)> {
    let mut out = BTreeMap::new();
    for node in graph.nodes() {
        if let Some(p) = placed.node(node.id()) {
            out.insert(identity_of(node).into_string(), (p.x, p.y));
        }
    }
    out
}

#[test]
#[ignore = "needs CARTOGRAPH_LAYOUT_BEFORE and _AFTER; the corpora are not vendored"]
fn survivors_keep_their_positions_across_two_real_revisions() {
    let (Some(before_analysis), Some(after_analysis)) = (
        analyse("CARTOGRAPH_LAYOUT_BEFORE"),
        analyse("CARTOGRAPH_LAYOUT_AFTER"),
    ) else {
        return;
    };

    let params = LayoutParams::default();
    let previous = &before_analysis.graph;
    let current = &after_analysis.graph;

    let before = layout(previous, &params);
    let stable = layout_stable(previous, current, &before, &params);
    let plain = layout(current, &params);

    let old = by_identity(previous, &before);
    let new = by_identity(current, &stable);
    let fresh = by_identity(current, &plain);

    let old_keys: BTreeSet<&String> = old.keys().collect();
    let new_keys: BTreeSet<&String> = new.keys().collect();
    let survivors: Vec<&&String> = old_keys.intersection(&new_keys).collect();
    let added: Vec<&&String> = new_keys.difference(&old_keys).collect();
    let removed: Vec<&&String> = old_keys.difference(&new_keys).collect();

    println!(
        "before {} nodes / after {} nodes",
        previous.node_count(),
        current.node_count()
    );
    println!(
        "survivors {} · added {} · removed {}",
        survivors.len(),
        added.len(),
        removed.len()
    );

    assert!(
        !survivors.is_empty(),
        "precondition: artefacts must survive"
    );

    // 1. Every survivor is exactly where it was.
    let mut moved = 0usize;
    for identity in &survivors {
        if new.get(**identity) != old.get(**identity) {
            moved += 1;
        }
    }
    assert_eq!(moved, 0, "{moved} surviving artefacts moved");

    // 2. A plain layout genuinely would have moved them, or the assertion above
    //    proves nothing about stability.
    let disturbed = survivors
        .iter()
        .filter(|identity| fresh.get(***identity) != old.get(***identity))
        .count();
    println!(
        "a plain layout would have moved {disturbed} of {} survivors",
        survivors.len()
    );
    assert!(
        disturbed > 0,
        "precondition: a plain layout must disturb something for this to mean anything"
    );

    // 3. New artefacts are positioned, finitely and within the extent.
    for identity in &added {
        let (x, y) = new.get(**identity).expect("added artefact positioned");
        assert!(x.is_finite() && y.is_finite());
        assert!(x.abs() <= params.extent + 1e-9 && y.abs() <= params.extent + 1e-9);
    }

    // 4. Removed artefacts have no position.
    for identity in &removed {
        assert!(
            !new.contains_key(**identity),
            "a removed artefact was positioned"
        );
    }

    // 5. One record per node of the new graph, and no others.
    assert_eq!(stable.node_count(), current.node_count());
}

/// Correspondence must survive the renumbering a real revision causes, which is
/// the case a fixture cannot produce at scale.
#[test]
#[ignore = "needs CARTOGRAPH_LAYOUT_BEFORE and _AFTER; the corpora are not vendored"]
fn correspondence_survives_real_node_id_renumbering() {
    let (Some(before_analysis), Some(after_analysis)) = (
        analyse("CARTOGRAPH_LAYOUT_BEFORE"),
        analyse("CARTOGRAPH_LAYOUT_AFTER"),
    ) else {
        return;
    };

    let previous = &before_analysis.graph;
    let current = &after_analysis.graph;

    let before_ids: BTreeMap<String, u64> = previous
        .nodes()
        .map(|n| (identity_of(n).into_string(), n.id().as_u64()))
        .collect();
    let after_ids: BTreeMap<String, u64> = current
        .nodes()
        .map(|n| (identity_of(n).into_string(), n.id().as_u64()))
        .collect();

    let shared: Vec<&String> = before_ids
        .keys()
        .filter(|k| after_ids.contains_key(*k))
        .collect();
    let renumbered = shared
        .iter()
        .filter(|k| before_ids.get(**k) != after_ids.get(**k))
        .count();

    println!(
        "shared artefacts {} · renumbered {}",
        shared.len(),
        renumbered
    );

    assert!(
        renumbered > 0,
        "precondition: a real revision must renumber something, or identity \
         correspondence is untested here"
    );

    // With that many handles moving, a layout keyed on `NodeId` could not hold
    // survivors still — yet it does.
    let params = LayoutParams::default();
    let before = layout(previous, &params);
    let stable = layout_stable(previous, current, &before, &params);
    let old = by_identity(previous, &before);
    let new = by_identity(current, &stable);

    let held = shared
        .iter()
        .filter(|k| new.get(**k) == old.get(**k))
        .count();
    println!("survivors holding position: {held} of {}", shared.len());
    assert_eq!(held, shared.len(), "some shared artefact moved");
}

/// Repeating the whole operation reproduces the same positions.
#[test]
#[ignore = "needs CARTOGRAPH_LAYOUT_BEFORE and _AFTER; the corpora are not vendored"]
fn the_real_stable_layout_is_deterministic() {
    let (Some(before_analysis), Some(after_analysis)) = (
        analyse("CARTOGRAPH_LAYOUT_BEFORE"),
        analyse("CARTOGRAPH_LAYOUT_AFTER"),
    ) else {
        return;
    };

    let params = LayoutParams::default();
    let before = layout(&before_analysis.graph, &params);

    let first = layout_stable(
        &before_analysis.graph,
        &after_analysis.graph,
        &before,
        &params,
    );
    let second = layout_stable(
        &before_analysis.graph,
        &after_analysis.graph,
        &before,
        &params,
    );

    assert_eq!(first.nodes, second.nodes);
    assert_eq!(first.clusters, second.clusters);
    println!("{} positions reproduced exactly", first.node_count());
}
