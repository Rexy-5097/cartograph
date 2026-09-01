//! Blast radius: what depends on a given artefact.
//!
//! # The direction, and why it is the reverse one
//!
//! Every edge Cartograph builds points from **dependent to dependency**. That
//! is not an assumption; it is what the resolver produces, and it holds across
//! all four kinds it constructs:
//!
//! ```text
//! ensureUseAssetService --Call--> getAssets --HttpCall--> get_assets
//!                                                             |
//!                                            post_connection --OrmAccess--> Connection
//!                                                        Action --Queries--> ab_permission
//! ```
//!
//! So "what breaks if I change X?" is answered by walking **against** the
//! arrows from X: the transitive closure of [`ArchitectureGraph::edges_to`],
//! whose own documentation names this as the primitive it exists for.
//!
//! The target is **not** in its own blast radius. Changing a thing does not
//! impact the thing; it impacts what depends on it.
//!
//! # Confidence
//!
//! A path is worth its **weakest** edge, and a node is worth its **best**
//! path — see
//! [ADR-0018](../../../docs/adr/ADR-0018-blast-radius-confidence.md). The rule
//! is `min` along a path and `max` across paths, never a product: Cartograph's
//! confidence is an uncalibrated prior (M08), and multiplying priors both
//! asserts a probability semantics they do not have and makes long chains look
//! doubtful for being long — burying exactly the cross-stack paths this
//! feature exists to surface.
//!
//! Because `min` and `max` are monotone this is the classic bottleneck
//! shortest-path problem, so a Dijkstra-style relaxation is correct.
//!
//! # Which edges count
//!
//! An explicit **allow-list**, never "every variant". `EdgeKind` is
//! `#[non_exhaustive]`, so a future kind must not silently widen what
//! "depends on" means — [`is_dependency`] fails closed and
//! `an_unclassified_edge_kind_must_not_be_traversed_by_default` is there to
//! break loudly if one appears.
//!
//! # What this module deliberately is not
//!
//! No caching, no persistence, no cross-run identity. It reads one graph from
//! one analysis and returns a value. `NodeId` is graph-local and that is
//! sufficient here (ADR-0014); nothing in this file is a step toward M13.

use std::cmp::Ordering;
use std::collections::{BTreeMap, BinaryHeap};

use cartograph_core::{EdgeId, EdgeKind, NodeId};

use crate::ArchitectureGraph;

/// Whether an edge kind means "the source depends on the target".
///
/// The allow-list is exhaustive over the variants that exist, with no wildcard
/// arm, so adding a kind to `EdgeKind` is a compile-time decision here rather
/// than a silent behaviour change. The `#[non_exhaustive]` fallback is `false`:
/// an unclassified relationship is **not** treated as a dependency, because
/// over-reporting impact is the more damaging error — a blast radius that
/// quietly grows is one a reviewer stops trusting.
#[must_use]
#[allow(
    clippy::match_same_arms,
    reason = "the excluded kinds are listed separately from the non_exhaustive               fallback on purpose: the list records which relationships were               considered and rejected, which a merged arm would erase"
)]
pub fn is_dependency(kind: EdgeKind) -> bool {
    match kind {
        // The four the resolver actually constructs.
        //
        // `Call`      — a caller depends on its callee.
        // `HttpCall`  — a client depends on the handler it calls; this is the
        //               language boundary the milestone is about.
        // `OrmAccess` — code depends on the model it reads or writes.
        // `Queries`   — a model depends on the table behind it.
        EdgeKind::Call | EdgeKind::HttpCall | EdgeKind::OrmAccess | EdgeKind::Queries => true,

        // Declared but never produced today, and excluded on meaning rather
        // than on absence. `Import` is a file-level fact, not a claim that a
        // symbol is used; `Inherits`/`Implements` describe shape; `References`
        // is unspecified in strength. Including any of them would widen impact
        // on a relationship nobody has argued is a dependency.
        EdgeKind::Import | EdgeKind::Inherits | EdgeKind::Implements | EdgeKind::References => {
            false
        }

        // Fail closed. See the module documentation.
        _ => false,
    }
}

/// One artefact that depends on the target.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Reached {
    /// The dependent artefact.
    pub node: NodeId,
    /// Confidence of the best-supported route from this node to the target.
    ///
    /// The minimum confidence along that route, maximised over routes
    /// (ADR-0018). **Not a probability**; it is one of the uncalibrated prior
    /// values already present on some edge of that route.
    pub confidence: f32,
    /// Hops along the route the confidence was taken from.
    ///
    /// Reported separately precisely *because* confidence does not decay with
    /// distance under this rule — depth is how a reader tells a direct
    /// dependent from a five-hop one.
    pub depth: u32,
    /// The edge this node reaches the target through, on that route.
    ///
    /// Carries the evidence: the edge already holds provenance, the
    /// observation and a source location, so the impact set is evidenced
    /// without this module copying any of it.
    pub via: EdgeId,
}

/// Everything that depends on one artefact.
#[derive(Debug, Clone, PartialEq)]
pub struct BlastRadius {
    /// The artefact the query was about. Never present in `reached`.
    pub target: NodeId,
    /// The dependents, in a deterministic semantic order.
    pub reached: Vec<Reached>,
}

impl BlastRadius {
    /// How many artefacts depend on the target.
    #[must_use]
    pub fn len(&self) -> usize {
        self.reached.len()
    }

    /// Whether nothing depends on the target.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.reached.is_empty()
    }

    /// The entry for `node`, if it is in the blast radius.
    ///
    /// Linear; a caller needing many lookups should build its own index.
    #[must_use]
    pub fn get(&self, node: NodeId) -> Option<&Reached> {
        self.reached.iter().find(|r| r.node == node)
    }

    /// The greatest number of hops any dependent is from the target.
    #[must_use]
    pub fn max_depth(&self) -> u32 {
        self.reached.iter().map(|r| r.depth).max().unwrap_or(0)
    }
}

/// Priority-queue key: best confidence first, then fewest hops.
///
/// A separate type because `f32` is not `Ord`. `total_cmp` gives a total order
/// without the partial-comparison pitfalls, and confidences here are always
/// finite — `Confidence` validates that at construction — so no NaN can reach
/// it. Ordering is reversed on depth so that, at equal confidence, the shorter
/// route is explored first.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Key {
    confidence: f32,
    depth: u32,
    node: NodeId,
}

impl Eq for Key {}

impl Ord for Key {
    fn cmp(&self, other: &Self) -> Ordering {
        self.confidence
            .total_cmp(&other.confidence)
            // `BinaryHeap` is a max-heap, so a *smaller* depth must compare
            // greater to be popped first.
            .then_with(|| other.depth.cmp(&self.depth))
            // Total order, so the heap is deterministic even on full ties.
            .then_with(|| other.node.cmp(&self.node))
    }
}

impl PartialOrd for Key {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

/// Everything that depends on `target`, transitively.
///
/// Walks [`ArchitectureGraph::edges_to`] outward from the target, following
/// only [`is_dependency`] edges, and reports each artefact reached with the
/// confidence of its best-supported route (ADR-0018).
///
/// # Behaviour at the edges of the problem
///
/// - **The target is excluded** from its own blast radius, even when a cycle
///   or a self-loop leads back to it.
/// - **A node absent from the graph** yields an empty radius rather than an
///   error: "nothing depends on a thing that is not here" is a true answer, and
///   the caller already knows whether it holds a valid handle.
/// - **Cycles terminate.** Relaxation only ever improves a node's confidence,
///   which is bounded above by 1.0, so the queue drains.
/// - **Duplicates collapse.** Each artefact appears once, carrying its best
///   route.
///
/// The traversal is iterative. Deep graphs cannot overflow the stack, which a
/// recursive formulation would risk on a long dependency chain.
#[must_use]
pub fn blast_radius(graph: &ArchitectureGraph, target: NodeId) -> BlastRadius {
    // `BTreeMap`, never `HashMap`: iteration order participates in nothing
    // below, but making that impossible is cheaper than proving it.
    let mut best: BTreeMap<NodeId, Reached> = BTreeMap::new();
    let mut queue: BinaryHeap<Key> = BinaryHeap::new();

    if graph.node(target).is_none() {
        return BlastRadius {
            target,
            reached: Vec::new(),
        };
    }

    // The target is the origin. Its notional confidence is 1.0 — the identity
    // for `min` — so the first edge traversed sets the bottleneck rather than
    // being capped by an arbitrary starting value.
    queue.push(Key {
        confidence: 1.0,
        depth: 0,
        node: target,
    });

    while let Some(current) = queue.pop() {
        // A stale queue entry: this node has since been reached better.
        if current.node != target {
            match best.get(&current.node) {
                None => continue,
                Some(found) => match found.confidence.total_cmp(&current.confidence) {
                    Ordering::Greater => continue,
                    Ordering::Equal if found.depth < current.depth => continue,
                    _ => {}
                },
            }
        }

        for edge in graph.edges_to(current.node) {
            if !is_dependency(edge.kind()) {
                continue;
            }
            let source = edge.source();
            // A self-loop, or a cycle closing back on the origin. The target is
            // never part of its own radius.
            if source == target {
                continue;
            }

            // The weakest link along this route.
            let confidence = current.confidence.min(edge.confidence().get());
            let depth = current.depth + 1;

            let improved = match best.get(&source) {
                None => true,
                Some(found) => match confidence.total_cmp(&found.confidence) {
                    Ordering::Greater => true,
                    Ordering::Equal => depth < found.depth,
                    Ordering::Less => false,
                },
            };
            if !improved {
                continue;
            }

            best.insert(
                source,
                Reached {
                    node: source,
                    confidence,
                    depth,
                    via: edge.id(),
                },
            );
            queue.push(Key {
                confidence,
                depth,
                node: source,
            });
        }
    }

    // Deterministic output order: semantic identity, never `NodeId` and never
    // discovery order. Two graphs describing one architecture in different
    // insertion orders must report the same list.
    let mut reached: Vec<Reached> = best.into_values().collect();
    reached.sort_by(|a, b| {
        identity_of(graph, a.node)
            .cmp(&identity_of(graph, b.node))
            .then_with(|| a.node.cmp(&b.node))
    });

    BlastRadius { target, reached }
}

/// A node's semantic identity, for ordering.
///
/// Shared with the layout contract rather than redefined, so the two cannot
/// drift into disagreeing about what identifies a node.
fn identity_of(graph: &ArchitectureGraph, node: NodeId) -> String {
    graph
        .node(node)
        .map_or_else(String::new, crate::layout::identity)
}
