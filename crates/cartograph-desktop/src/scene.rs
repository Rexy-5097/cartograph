//! The render scene: what a renderer needs, and nothing more.
//!
//! # Why this is a separate type from `Layout`
//!
//! [ADR-0015](../../../docs/adr/ADR-0015-layout-contract.md) fixes
//! `layout::Layout` at `{id, x, y, cluster}` and keeps it free of names, kinds
//! and evidence. That is the right shape for a *layout* — an algorithm that
//! places points does not need to know what they are called — but it is not
//! enough to draw a map a person can read. A screen of unlabelled dots answers
//! nothing about what a system is.
//!
//! So the scene is composed here, in the client, rather than by widening the
//! layout contract. `layout::Layout` is untouched; this module joins it to the
//! graph the same analysis produced.
//!
//! # What earns a place, and what does not
//!
//! Every field answers a concrete rendering question. The test
//! `the_scene_carries_no_analysis_payload` pins the shape, so adding a field
//! is a deliberate act rather than a drift.
//!
//! | Field | Why a renderer needs it |
//! |---|---|
//! | `id` | addresses the node for selection and for edge endpoints |
//! | `x`, `y` | **taken verbatim from Rust**; the frontend never computes these |
//! | `cluster` | colours and groups; also taken verbatim |
//! | `label` | a map of unnamed dots does not answer "what is this system?" |
//! | `kind` | a route, a table and a function are not the same thing, and the reader must see that at a glance |
//!
//! Edges carry their endpoints and their kind for the same reason: an HTTP
//! call crossing from TypeScript to Python is the claim the product exists to
//! make, and it must be visually distinguishable from an import.
//!
//! **Deliberately absent: `confidence`, `provenance`, `evidence` and
//! `location`.** Those are the evidence record, and opening it on an edge click
//! is M11's defining interaction — Slice 4. Shipping them here would mean the
//! renderer slice quietly carried the next slice's payload, and every node and
//! edge would pay the transport cost for data nothing draws.
//!
//! # The scene is derived, never authored
//!
//! Positions and clusters are copied out of `Layout` without arithmetic.
//! `a_scene_copies_layout_coordinates_verbatim` asserts bit-equality rather
//! than approximate equality, because "close enough" is exactly how a
//! frontend-side transform would slip in unnoticed.

use std::collections::BTreeSet;

use cartograph_core::{EdgeId, EdgeKind, NodeId, NodeKind};
use cartograph_graph::ArchitectureGraph;
use cartograph_graph::layout::{Cluster, ClusterId, Layout};
use serde::{Deserialize, Serialize};

/// One node, ready to draw.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SceneNode {
    /// Graph-local handle; also how edges name their endpoints.
    pub id: NodeId,
    /// Horizontal position, copied verbatim from the Rust layout.
    pub x: f64,
    /// Vertical position, copied verbatim from the Rust layout.
    pub y: f64,
    /// Cluster membership, copied verbatim from the Rust layout.
    pub cluster: ClusterId,
    /// The artefact's name, for the on-screen label.
    pub label: String,
    /// What the artefact is, for colour and size.
    pub kind: NodeKind,
}

/// One edge, ready to draw.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SceneEdge {
    /// Graph-local handle, so a renderer can address one edge.
    pub id: EdgeId,
    /// The node the relationship starts at.
    pub source: NodeId,
    /// The node the relationship points to.
    pub target: NodeId,
    /// What kind of relationship, for colour.
    pub kind: EdgeKind,
}

/// A complete drawable scene.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Scene {
    /// One entry per graph node, in the layout's order.
    pub nodes: Vec<SceneNode>,
    /// Every edge whose endpoints are both present.
    pub edges: Vec<SceneEdge>,
    /// The cluster table, carried through from the layout.
    pub clusters: Vec<Cluster>,
}

impl Scene {
    /// Number of drawable nodes.
    #[must_use]
    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }

    /// Number of drawable edges.
    #[must_use]
    pub fn edge_count(&self) -> usize {
        self.edges.len()
    }
}

/// Joins a graph to its layout.
///
/// # Endpoints
///
/// An edge is emitted only when **both** endpoints appear in the layout. For a
/// layout produced from this same graph that is every edge, because
/// `layout::layout` is total over the graph's nodes — a property its own suite
/// asserts. The check exists for the case where the two arguments do not
/// belong together, where silently emitting an edge to a node the renderer
/// never received would produce a dangling reference in the frontend rather
/// than a diagnosable error here.
///
/// A node in the layout that is not in the graph is likewise skipped; the same
/// reasoning applies in reverse.
#[must_use]
pub fn compose(graph: &ArchitectureGraph, layout: &Layout) -> Scene {
    let mut nodes = Vec::with_capacity(layout.nodes.len());
    let mut placed: BTreeSet<NodeId> = BTreeSet::new();

    for positioned in &layout.nodes {
        let Some(node) = graph.node(positioned.id) else {
            continue;
        };
        placed.insert(positioned.id);
        nodes.push(SceneNode {
            id: positioned.id,
            // Copied, not computed. See the module documentation.
            x: positioned.x,
            y: positioned.y,
            cluster: positioned.cluster,
            label: node.name().to_owned(),
            kind: node.kind(),
        });
    }

    let edges = graph
        .edges()
        .filter(|edge| placed.contains(&edge.source()) && placed.contains(&edge.target()))
        .map(|edge| SceneEdge {
            id: edge.id(),
            source: edge.source(),
            target: edge.target(),
            kind: edge.kind(),
        })
        .collect();

    Scene {
        nodes,
        edges,
        clusters: layout.clusters.clone(),
    }
}
