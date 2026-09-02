//! The Cartograph architecture graph.
//!
//! # petgraph does not leak
//!
//! `petgraph` provides algorithms; it is not the domain model. Nothing in this
//! crate's public API names a petgraph type, and `cartograph-core` does not
//! depend on petgraph at all. That keeps the library replaceable: if the graph
//! ever needs a different backing store — an on-disk representation for very
//! large repositories, say — the change is confined to this file rather than
//! spreading through every caller.
//!
//! `StableDiGraph` specifically, rather than `DiGraph`, because indices must
//! survive removals: the incremental engine (M10) removes and re-adds
//! subgraphs as files change, and invalidated indices would silently corrupt
//! every stored reference.
//!
//! See `docs/adr/ADR-0003-cartograph-owns-the-graph.md`.

pub mod blast;
pub mod diff;
pub mod layout;

use std::collections::HashMap;

use cartograph_core::{
    Confidence, Edge, EdgeId, EdgeKind, Evidence, Node, NodeId, NodeKind, Provenance,
    SourceLocation, id::CommitId,
};
use petgraph::{Direction, graph::NodeIndex, stable_graph::StableDiGraph};
use thiserror::Error;
use time::OffsetDateTime;

/// Failure to modify an architecture graph.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum GraphError {
    /// An edge referred to a node that is not in this graph.
    ///
    /// Cartograph does not create placeholder nodes for unresolved targets. An
    /// edge to something the analysis never found would be an assertion the
    /// evidence does not support.
    #[error("no such node in this graph: {id}")]
    UnknownNode {
        /// The node that was referenced.
        id: NodeId,
    },
}

/// Everything needed to assert a relationship.
///
/// Passed by value to [`ArchitectureGraph::add_edge`], which supplies the
/// identifier and timestamp. Every field is required except `commit`; see
/// [`Edge`] for why that is not negotiable.
#[derive(Debug, Clone)]
pub struct EdgeSpec {
    /// The node the relationship starts at.
    pub source: NodeId,
    /// The node the relationship points to.
    pub target: NodeId,
    /// What kind of relationship this is.
    pub kind: EdgeKind,
    /// How much the edge should be trusted.
    pub confidence: Confidence,
    /// Which analysis produced it.
    pub provenance: Provenance,
    /// The observation supporting it.
    pub evidence: Evidence,
    /// Where the claim was observed.
    pub location: SourceLocation,
    /// The revision it was derived from, if known.
    pub commit: Option<CommitId>,
}

/// A directed graph of software artefacts and the evidenced relationships
/// between them.
#[derive(Debug)]
pub struct ArchitectureGraph {
    inner: StableDiGraph<Node, Edge>,
    indices: HashMap<NodeId, NodeIndex>,
    /// Domain identity → node, so two analyses describing one artefact agree.
    identities: HashMap<(NodeKind, String, Option<String>), NodeId>,
    next_node_id: u64,
    next_edge_id: u64,
    clock: fn() -> OffsetDateTime,
}

impl ArchitectureGraph {
    /// Creates an empty graph.
    #[must_use]
    pub fn new() -> Self {
        Self {
            inner: StableDiGraph::new(),
            indices: HashMap::new(),
            identities: HashMap::new(),
            next_node_id: 0,
            next_edge_id: 0,
            clock: OffsetDateTime::now_utc,
        }
    }

    /// Creates an empty graph that stamps edges from `clock` instead of the
    /// system time.
    ///
    /// Golden-fixture tests (M08) compare serialised graphs byte for byte, so
    /// they need a timestamp that does not move.
    #[must_use]
    pub fn with_clock(clock: fn() -> OffsetDateTime) -> Self {
        Self {
            clock,
            ..Self::new()
        }
    }

    /// Adds a node and returns its handle.
    ///
    /// # Errors
    ///
    /// Propagates [`cartograph_core::CoreError`] if the node is invalid — an
    /// empty name, for instance.
    pub fn add_node(
        &mut self,
        kind: NodeKind,
        name: impl Into<String>,
        location: Option<SourceLocation>,
    ) -> Result<NodeId, cartograph_core::CoreError> {
        let id = NodeId::from_raw(self.next_node_id);
        let node = Node::new(id, kind, name, location)?;
        let index = self.inner.add_node(node);
        self.indices.insert(id, index);
        self.next_node_id += 1;
        Ok(id)
    }

    /// Returns the handle for an artefact, creating it only if absent.
    ///
    /// Two analyses describing the same artefact must reach the same node, or
    /// the graph silently splits into disconnected fragments. M06 exposed
    /// this: the route matcher created a `Function` node for `create_order`,
    /// the ORM resolver created a second one, and the cross-stack chain broke
    /// between them — every edge correct, the path unwalkable.
    ///
    /// Identity is the triple **(kind, name, file)**. The file distinguishes
    /// same-named artefacts in different modules; the line does not
    /// participate, because two analyses legitimately observe one artefact at
    /// different lines — a decorator and a `def`, a declaration and a call
    /// site — and treating those as different nodes is the bug this prevents.
    ///
    /// This is deduplication *within* one graph; see
    /// `docs/adr/ADR-0011-node-identity-within-a-graph.md`.
    ///
    /// Stable content-addressed identity *across* graphs was ADR-0011's
    /// deferral to M10, and M10 deferred it again — its cache never keys on a
    /// handle. ADR-0014 records that and names M13 (structural diff) as the
    /// milestone that cannot proceed without it. Note that this triple is
    /// line-independent, so it is the closer of the two identity notions in
    /// the tree; it still cannot express a moved file or a renamed artefact as
    /// the same node, which is the design content of that future slice.
    ///
    /// # Errors
    ///
    /// Propagates [`cartograph_core::CoreError`] if the node is invalid.
    pub fn node_for(
        &mut self,
        kind: NodeKind,
        name: impl Into<String>,
        location: Option<SourceLocation>,
    ) -> Result<NodeId, cartograph_core::CoreError> {
        let name = name.into();
        let file = location.as_ref().map(|l| l.file().to_owned());
        if let Some(existing) = self.identities.get(&(kind, name.clone(), file.clone())) {
            return Ok(*existing);
        }
        let id = self.add_node(kind, name.clone(), location)?;
        self.identities.insert((kind, name, file), id);
        Ok(id)
    }

    /// Asserts a relationship and returns its handle.
    ///
    /// # Errors
    ///
    /// Returns [`GraphError::UnknownNode`] if either endpoint is not in this
    /// graph.
    pub fn add_edge(&mut self, spec: EdgeSpec) -> Result<EdgeId, GraphError> {
        let source_index = self.index_of(spec.source)?;
        let target_index = self.index_of(spec.target)?;

        let id = EdgeId::from_raw(self.next_edge_id);
        let edge = Edge::new(
            id,
            spec.source,
            spec.target,
            spec.kind,
            spec.confidence,
            spec.provenance,
            spec.evidence,
            spec.location,
            spec.commit,
            (self.clock)(),
        );
        self.inner.add_edge(source_index, target_index, edge);
        self.next_edge_id += 1;
        Ok(id)
    }

    /// Looks up a node.
    #[must_use]
    pub fn node(&self, id: NodeId) -> Option<&Node> {
        self.indices
            .get(&id)
            .and_then(|i| self.inner.node_weight(*i))
    }

    /// Looks up an edge.
    ///
    /// Linear in the number of edges. An index is not worth building until
    /// there is a caller that needs one; the incremental engine (M10) is the
    /// first plausible candidate.
    #[must_use]
    pub fn edge(&self, id: EdgeId) -> Option<&Edge> {
        self.edges().find(|edge| edge.id() == id)
    }

    /// Every node, in unspecified order.
    pub fn nodes(&self) -> impl Iterator<Item = &Node> {
        self.inner.node_weights()
    }

    /// Every edge, in unspecified order.
    pub fn edges(&self) -> impl Iterator<Item = &Edge> {
        self.inner.edge_weights()
    }

    /// Edges leaving `id`.
    ///
    /// Returns an empty iterator for a node that is not in this graph.
    pub fn edges_from(&self, id: NodeId) -> impl Iterator<Item = &Edge> {
        self.directed_edges(id, Direction::Outgoing)
    }

    /// Edges arriving at `id`.
    ///
    /// Returns an empty iterator for a node that is not in this graph. This is
    /// the primitive blast radius (M12) is built on.
    pub fn edges_to(&self, id: NodeId) -> impl Iterator<Item = &Edge> {
        self.directed_edges(id, Direction::Incoming)
    }

    /// How many nodes the graph holds.
    #[must_use]
    pub fn node_count(&self) -> usize {
        self.inner.node_count()
    }

    /// How many edges the graph holds.
    #[must_use]
    pub fn edge_count(&self) -> usize {
        self.inner.edge_count()
    }

    fn directed_edges(&self, id: NodeId, direction: Direction) -> impl Iterator<Item = &Edge> {
        self.indices
            .get(&id)
            .into_iter()
            .flat_map(move |index| self.inner.edges_directed(*index, direction))
            .map(|edge| edge.weight())
    }

    fn index_of(&self, id: NodeId) -> Result<NodeIndex, GraphError> {
        self.indices
            .get(&id)
            .copied()
            .ok_or(GraphError::UnknownNode { id })
    }
}

impl Default for ArchitectureGraph {
    fn default() -> Self {
        Self::new()
    }
}
