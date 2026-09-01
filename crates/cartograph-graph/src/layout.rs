//! Deterministic layout of an architecture graph.
//!
//! # Why this lives in Rust
//!
//! [ADR-0001](../../../docs/adr/ADR-0001-rust-core-is-the-product.md) and
//! [ADR-0006](../../../docs/adr/ADR-0006-sigma-before-custom-renderer.md) both
//! place layout in the core: the frontend receives `{id, x, y, cluster}` and
//! does nothing but draw. That is what keeps a renderer swap contained — if
//! positions were computed in JavaScript, replacing Sigma.js would mean
//! reimplementing layout, and the client would hold analysis logic in
//! violation of RULE 002.
//!
//! This module is therefore the boundary between the analysis core and every
//! future client. It emits coordinates and cluster membership, and nothing
//! else: no names, no evidence, no confidence, no provenance. A client that
//! wants those asks the graph for them.
//!
//! # The determinism guarantee
//!
//! **The same graph and the same [`LayoutParams`] produce the same
//! coordinates, on the same binary.** Concretely:
//!
//! - node ordering is by *semantic identity* — `(kind, name, file:line)` —
//!   never by [`NodeId`], so insertion order cannot move a node;
//! - cluster ids are assigned by sorting cluster labels, so filesystem
//!   enumeration order cannot move a node;
//! - every map used during layout is a [`BTreeMap`], never a `HashMap`, so
//!   hash iteration order cannot move a node;
//! - initial positions come from a small FNV-1a hash written out below rather
//!   than from a random number generator or `DefaultHasher` — there is no
//!   seed to forget to set, and no dependence on a hasher whose output may
//!   change between Rust releases;
//! - the iteration count is fixed by `LayoutParams`, so there is no
//!   convergence test whose result could vary with floating-point noise.
//!
//! Two graphs that are *semantically equal* in the sense of M10's canonical
//! comparison therefore lay out identically, even though their `NodeId`s may
//! differ. That is a consequence of seeding from semantic identity, and it is
//! deliberately **not** stable node identity: nothing here assigns a durable
//! handle, and nothing correlates a node across two analyses.
//!
//! # What this does not promise
//!
//! **Determinism is not stability under change.** Adding one node changes its
//! cluster's population, which moves every node in that cluster, and adding a
//! cluster re-indexes the ones after it. Preserving a reader's mental map
//! across two different graphs is
//! `layout(prev_graph, new_graph, prev_positions)` — a distinct problem that
//! belongs to M13, and one that also requires the cross-run node
//! correspondence [ADR-0014](../../../docs/adr/ADR-0014-m10-scope-reconciliation.md)
//! defers. Nothing here anticipates it.

use std::collections::{BTreeMap, BTreeSet};

use cartograph_core::{Node, NodeId};
use serde::{Deserialize, Serialize};

use crate::ArchitectureGraph;

/// Handle to a cluster within one [`Layout`].
///
/// Dense and assigned by sorting cluster labels, so it is a property of the
/// graph's content rather than of the order nodes were added. Like [`NodeId`]
/// it is local to the layout that produced it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ClusterId(u32);

impl ClusterId {
    /// The raw index.
    #[must_use]
    pub const fn as_u32(self) -> u32 {
        self.0
    }
}

impl std::fmt::Display for ClusterId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "c{}", self.0)
    }
}

/// A group of nodes drawn together.
///
/// The label is the grouping's own name — a repository-relative directory, or
/// a bracketed node kind for artefacts that have no source location. It is
/// carried because a cluster the reader cannot name answers nothing about what
/// a system is, and deriving it again in the client would put a rule about
/// Cartograph's data model into the frontend.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Cluster {
    /// Handle used by [`LayoutNode::cluster`].
    pub id: ClusterId,
    /// Repository-relative directory, or `<Kind>` for unlocated artefacts.
    pub label: String,
}

/// One node's drawing instruction: where it goes and what it belongs to.
///
/// This is the whole per-node payload a client receives. It deliberately
/// carries no name, kind, evidence, confidence or provenance — see the module
/// documentation.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct LayoutNode {
    /// The node this positions, addressing the graph it was laid out from.
    pub id: NodeId,
    /// Horizontal position, within the extent given by [`LayoutParams`].
    pub x: f64,
    /// Vertical position, within the extent given by [`LayoutParams`].
    pub y: f64,
    /// The cluster this node was grouped into.
    pub cluster: ClusterId,
}

/// A complete layout: one record per graph node, plus the cluster table.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Layout {
    /// One entry per node in the source graph, ordered by semantic identity.
    pub nodes: Vec<LayoutNode>,
    /// Every cluster referenced by `nodes`, ordered by [`ClusterId`].
    pub clusters: Vec<Cluster>,
}

impl Layout {
    /// Number of positioned nodes.
    #[must_use]
    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }

    /// Number of clusters.
    #[must_use]
    pub fn cluster_count(&self) -> usize {
        self.clusters.len()
    }

    /// The position assigned to `id`, if it was laid out.
    ///
    /// Linear; a client that needs many lookups should build its own index.
    #[must_use]
    pub fn node(&self, id: NodeId) -> Option<&LayoutNode> {
        self.nodes.iter().find(|n| n.id == id)
    }
}

/// Tunable inputs to [`layout`].
///
/// Every field participates in the determinism guarantee: two runs agree only
/// if these agree.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LayoutParams {
    /// Force-directed relaxation passes within each cluster.
    ///
    /// Fixed rather than convergence-tested, so the result cannot vary with
    /// floating-point noise near a threshold.
    pub iterations: u32,
    /// Half-width of the square the finished layout is scaled into.
    ///
    /// Coordinates are guaranteed to land in `[-extent, extent]`.
    pub extent: f64,
    /// Centre-to-centre distance between clusters, as a multiple of the
    /// largest cluster's half-width.
    ///
    /// Relative rather than absolute so that one large package cannot grow
    /// until it overlaps its neighbours: the spacing grows with it.
    pub cluster_spacing: f64,
}

impl Default for LayoutParams {
    /// Values chosen to be legible at a few thousand nodes, not tuned.
    fn default() -> Self {
        Self {
            iterations: 60,
            extent: 1.0,
            cluster_spacing: 3.0,
        }
    }
}

/// Below this, two positions are treated as coincident rather than divided by.
const EPSILON: f64 = 1e-9;

const FNV_OFFSET_BASIS: u64 = 0xcbf2_9ce4_8422_2325;
const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;

/// FNV-1a, written out so the layout does not depend on a hasher whose output
/// may change between Rust releases.
fn fnv1a(bytes: &[u8]) -> u64 {
    let mut hash = FNV_OFFSET_BASIS;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(FNV_PRIME);
    }
    hash
}

/// A node's semantic identity, used for ordering and for seeding.
///
/// The same shape M10's canonical comparison uses: what the node *claims*, not
/// which slot it occupies. Two graphs that are canonically equal produce the
/// same identities here, which is what makes the layout independent of
/// insertion order.
pub(crate) fn identity(node: &Node) -> String {
    let location = node
        .location()
        .map_or_else(|| "-".to_owned(), |l| format!("{}:{}", l.file(), l.line()));
    format!("{:?}|{}|{}", node.kind(), node.name(), location)
}

/// The cluster a node belongs to.
///
/// Located artefacts group by their directory, which is the structural
/// information the analyser already proves and the one a reader recognises.
/// Artefacts with no source location — a database table, an external service,
/// an environment variable — group by kind, because they are real parts of the
/// system and dropping them into one unnamed bucket would answer nothing.
fn cluster_label(node: &Node) -> String {
    node.location().map_or_else(
        || format!("<{:?}>", node.kind()),
        |location| {
            location
                .file()
                .rsplit_once('/')
                .map_or_else(|| "<root>".to_owned(), |(dir, _)| dir.to_owned())
        },
    )
}

/// Deterministic initial offset within a cluster, in `[-1, 1]` on each axis.
///
/// Derived from the node's semantic identity so that no random number
/// generator, and therefore no un-set seed, can enter the layout.
fn seed_offset(identity: &str) -> (f64, f64) {
    let hash = fnv1a(identity.as_bytes());
    // Two independent 32-bit halves, mapped onto the unit square.
    let low = f64::from(u32::try_from(hash & 0xffff_ffff).unwrap_or(0));
    let high = f64::from(u32::try_from(hash >> 32).unwrap_or(0));
    let scale = f64::from(u32::MAX);
    (low / scale * 2.0 - 1.0, high / scale * 2.0 - 1.0)
}

/// Half-width of the frame one cluster is relaxed inside.
///
/// Proportional to the square root of the population, so every cluster is
/// drawn at the same density rather than every cluster at the same size.
#[allow(clippy::cast_precision_loss)] // cluster sizes are far below 2^53
fn frame_half(count: usize) -> f64 {
    (count as f64).sqrt().max(1.0) / 2.0
}

/// Cluster centres on a phyllotaxis spiral.
///
/// Even spacing without a packing search, and a pure function of the cluster
/// index, so it cannot drift between runs.
#[allow(clippy::cast_precision_loss)] // cluster counts are far below 2^53
fn cluster_centre(index: usize, spacing: f64) -> (f64, f64) {
    // The golden angle, which is what makes successive points avoid each other.
    const GOLDEN_ANGLE: f64 = 2.399_963_229_728_653;
    let i = index as f64;
    let radius = spacing * i.sqrt();
    let angle = i * GOLDEN_ANGLE;
    (radius * angle.cos(), radius * angle.sin())
}

/// Positions every node in `graph`.
///
/// One [`LayoutNode`] per graph node, no more and no fewer. Coordinates are
/// always finite and always inside `[-params.extent, params.extent]`; see the
/// module documentation for the determinism guarantee.
///
/// An empty graph produces an empty layout rather than an error — nothing to
/// draw is a legitimate answer, not a failure.
#[must_use]
#[allow(clippy::cast_precision_loss)] // node counts are far below 2^53
pub fn layout(graph: &ArchitectureGraph, params: &LayoutParams) -> Layout {
    // 1. Order nodes by semantic identity. `NodeId` breaks ties only between
    //    nodes that are semantically indistinguishable, whose seeds — and so
    //    whose initial positions — are identical anyway.
    let mut ordered: Vec<(String, NodeId)> = graph.nodes().map(|n| (identity(n), n.id())).collect();
    ordered.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| a.1.cmp(&b.1)));

    if ordered.is_empty() {
        return Layout {
            nodes: Vec::new(),
            clusters: Vec::new(),
        };
    }

    // 2. Assign clusters. Sorting the labels is what makes the ids a property
    //    of the content rather than of the traversal.
    let mut labels: BTreeSet<String> = BTreeSet::new();
    let mut label_of: BTreeMap<NodeId, String> = BTreeMap::new();
    for node in graph.nodes() {
        let label = cluster_label(node);
        labels.insert(label.clone());
        label_of.insert(node.id(), label);
    }
    let clusters: Vec<Cluster> = labels
        .iter()
        .enumerate()
        .map(|(i, label)| Cluster {
            // A graph with more than u32::MAX clusters is not a thing that
            // exists; saturating keeps the function total rather than panicking.
            id: ClusterId(u32::try_from(i).unwrap_or(u32::MAX)),
            label: label.clone(),
        })
        .collect();
    let cluster_index: BTreeMap<&str, ClusterId> =
        clusters.iter().map(|c| (c.label.as_str(), c.id)).collect();

    // 3. Group nodes by cluster, preserving the semantic ordering within each.
    let mut members: BTreeMap<ClusterId, Vec<(String, NodeId)>> = BTreeMap::new();
    for (ident, id) in ordered {
        let label = &label_of[&id];
        let cluster = cluster_index[label.as_str()];
        members.entry(cluster).or_default().push((ident, id));
    }

    // 4. Relax each cluster independently, then place it at its centre.
    //
    //    Only intra-cluster edges participate. Cross-cluster edges pull on
    //    nothing, which is a real limitation recorded in the module docs: it
    //    keeps the cost at the sum of squared cluster sizes rather than the
    //    square of the node count, and cluster placement already carries the
    //    coarse structure.
    //
    //    Centre spacing is driven by the *largest* cluster, so a package big
    //    enough to fill its frame still cannot reach its neighbour's.
    let widest = members
        .values()
        .map(|group| frame_half(group.len()))
        .fold(1.0_f64, f64::max);
    let spacing = params.cluster_spacing * widest;

    let mut positions: BTreeMap<NodeId, (f64, f64)> = BTreeMap::new();
    for (cluster, group) in &members {
        let relaxed = relax(graph, group, params.iterations);
        let (cx, cy) = cluster_centre(cluster.as_u32() as usize, spacing);
        for (id, (x, y)) in relaxed {
            positions.insert(id, (cx + x, cy + y));
        }
    }

    // 5. Normalise into the requested extent. This is also what guarantees
    //    finite, bounded output: a degenerate span collapses to the centre
    //    instead of dividing by zero.
    let nodes = normalise(&members, &positions, params.extent);

    Layout { nodes, clusters }
}

/// Fruchterman–Reingold relaxation of one cluster.
///
/// Returns positions relative to the cluster's own origin.
#[allow(clippy::cast_precision_loss)] // cluster sizes are far below 2^53
fn relax(
    graph: &ArchitectureGraph,
    group: &[(String, NodeId)],
    iterations: u32,
) -> Vec<(NodeId, (f64, f64))> {
    let count = group.len();
    // Constant density: a cluster of n nodes occupies a frame of area n, so a
    // large package is drawn larger rather than more densely.
    let half = frame_half(count);
    let mut points: Vec<(f64, f64)> = group
        .iter()
        .map(|(ident, _)| {
            let (x, y) = seed_offset(ident);
            (x * half, y * half)
        })
        .collect();

    if count < 2 {
        return group
            .iter()
            .zip(points)
            .map(|((_, id), p)| (*id, p))
            .collect();
    }

    // Index within this cluster, so edge lookup does not need a search.
    let slot: BTreeMap<NodeId, usize> = group
        .iter()
        .enumerate()
        .map(|(i, (_, id))| (*id, i))
        .collect();

    // Intra-cluster edges only, de-duplicated and sorted: a pair repeated by
    // several edges must not pull harder than the same pair joined once, or
    // edge insertion order would leak into the result.
    let mut pairs: BTreeSet<(usize, usize)> = BTreeSet::new();
    for edge in graph.edges() {
        if let (Some(&a), Some(&b)) = (slot.get(&edge.source()), slot.get(&edge.target())) {
            if a != b {
                pairs.insert((a.min(b), a.max(b)));
            }
        }
    }

    // The classic FR constants: `k` is the ideal spacing for `count` nodes in
    // a frame of side `2 * half`.
    let area = (2.0 * half) * (2.0 * half);
    let k = (area / count as f64).sqrt();
    let mut temperature = half / 2.0;
    let cooling = if iterations == 0 {
        0.0
    } else {
        temperature / f64::from(iterations)
    };

    for _ in 0..iterations {
        let mut disp = vec![(0.0_f64, 0.0_f64); count];

        // Repulsion between every pair in the cluster.
        for i in 0..count {
            for j in (i + 1)..count {
                let (dx, dy) = (points[i].0 - points[j].0, points[i].1 - points[j].1);
                let distance = (dx * dx + dy * dy).sqrt().max(EPSILON);
                let force = k * k / distance;
                let (ux, uy) = (dx / distance, dy / distance);
                disp[i].0 += ux * force;
                disp[i].1 += uy * force;
                disp[j].0 -= ux * force;
                disp[j].1 -= uy * force;
            }
        }

        // Attraction along edges.
        for &(a, b) in &pairs {
            let (dx, dy) = (points[a].0 - points[b].0, points[a].1 - points[b].1);
            let distance = (dx * dx + dy * dy).sqrt().max(EPSILON);
            let force = distance * distance / k;
            let (ux, uy) = (dx / distance, dy / distance);
            disp[a].0 -= ux * force;
            disp[a].1 -= uy * force;
            disp[b].0 += ux * force;
            disp[b].1 += uy * force;
        }

        // Move, capped by the current temperature, then confine to the frame.
        //
        // The frame is not decoration. Without it, two nodes in a cluster that
        // share no edge repel every iteration and drift apart without bound,
        // so a cluster stops being a place on the drawing and starts
        // overlapping its neighbours. This is the constraint the original
        // Fruchterman-Reingold formulation carries for exactly that reason.
        for i in 0..count {
            let (dx, dy) = disp[i];
            let magnitude = (dx * dx + dy * dy).sqrt();
            if magnitude > EPSILON {
                let step = magnitude.min(temperature);
                points[i].0 += dx / magnitude * step;
                points[i].1 += dy / magnitude * step;
            }
            points[i].0 = points[i].0.clamp(-half, half);
            points[i].1 = points[i].1.clamp(-half, half);
        }
        temperature -= cooling;
    }

    group
        .iter()
        .zip(points)
        .map(|((_, id), p)| (*id, p))
        .collect()
}

/// Scales every position into `[-extent, extent]` on both axes.
///
/// The single place coordinates can become unbounded or non-finite, and
/// therefore the single place that has to guard against it: a span at or below
/// [`EPSILON`] — one node, or many coincident ones — collapses to the centre
/// rather than dividing by it.
fn normalise(
    members: &BTreeMap<ClusterId, Vec<(String, NodeId)>>,
    positions: &BTreeMap<NodeId, (f64, f64)>,
    extent: f64,
) -> Vec<LayoutNode> {
    let (mut min_x, mut max_x) = (f64::INFINITY, f64::NEG_INFINITY);
    let (mut min_y, mut max_y) = (f64::INFINITY, f64::NEG_INFINITY);
    for (x, y) in positions.values() {
        min_x = min_x.min(*x);
        max_x = max_x.max(*x);
        min_y = min_y.min(*y);
        max_y = max_y.max(*y);
    }

    // One scale for both axes, so the drawing is not stretched.
    let span = (max_x - min_x).max(max_y - min_y);
    let scale = if span > EPSILON {
        2.0 * extent / span
    } else {
        0.0
    };
    let centre_x = f64::midpoint(min_x, max_x);
    let centre_y = f64::midpoint(min_y, max_y);

    let mut out = Vec::new();
    for (cluster, group) in members {
        for (_, id) in group {
            let (x, y) = positions[id];
            out.push(LayoutNode {
                id: *id,
                x: (x - centre_x) * scale,
                y: (y - centre_y) * scale,
                cluster: *cluster,
            });
        }
    }
    out
}
