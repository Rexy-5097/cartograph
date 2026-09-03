//! Following one symbol's relationships outward through the graph.
//!
//! # Why this lives here rather than in a client
//!
//! It did not, until M15. The walk was written for `cartograph trace` and lived
//! inside `cartograph-cli`, which is a **binary** crate: nothing can depend on
//! it. That was invisible while the CLI was the only caller, and it stopped
//! being invisible when the MCP server became the third client — the same
//! situation ADR-0016 recorded at M11, when the analysis *sequence* had to
//! leave the CLI before the desktop could call it.
//!
//! A second client had the two options that ADR named, and both are wrong:
//! duplicate the traversal, or reach into the first client. RULE 002 forbids a
//! client holding analysis logic, and a traversal that decides which
//! relationships exist and in what order is analysis. So it moves here, beside
//! [`crate::blast`] and [`crate::diff`], and every client renders what it
//! returns. See ADR-0019.
//!
//! # The path comes out of the graph
//!
//! This module walks edges with [`ArchitectureGraph::edges_from`]. It does not
//! re-derive a chain from match results, from the source, or from anything
//! else. If a rendered path and the graph disagree, the rendering is wrong —
//! there is no second opinion to consult.
//!
//! # What it returns, and what it deliberately does not
//!
//! [`Trace`] carries [`NodeId`] and [`EdgeId`] and nothing else. It does not
//! carry names, confidences, evidence or formatted text: a caller that wants
//! those reads them from the graph it already has, which keeps one copy of
//! every fact. Refusals — observations the *resolver* declined to turn into an
//! edge — are not here either, because they live in `Analysis::results` and
//! reaching them would make this crate depend on `cartograph-resolver`,
//! inverting the dependency direction QG-006 checks.
//!
//! # Ambiguity is not resolved by choosing
//!
//! Two symbols named `User` in different files are two different things.
//! [`resolve_symbol`] returns [`SymbolError::Ambiguous`] with every candidate
//! rather than the one the graph happened to number first. Picking one would
//! make the answer depend on iteration order, which is the class of quiet
//! wrongness the project's refusal rules exist to prevent (RULE 009).

use std::collections::HashSet;

use cartograph_core::{EdgeId, Node, NodeId, SourceLocation};
use serde::Serialize;

use crate::ArchitectureGraph;

/// How deep a trace walks before stopping.
///
/// A frontend → client → route → handler → model → table chain is five hops.
/// Ten leaves room for longer real chains while bounding output on a graph
/// with a long import spine.
pub const DEFAULT_MAX_DEPTH: usize = 10;

/// Why a walk stopped.
///
/// Serialised as a stable kebab-case slug because it reaches the CLI's JSON
/// document, the same way [`cartograph_core::NodeKind`] does.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Stop {
    /// The node has no outgoing edge. The chain genuinely ends here.
    ChainEnd,
    /// The depth limit was reached; more of the chain may exist.
    MaxDepth,
    /// The walk returned to a node already on this path.
    Cycle,
}

/// One hop, and everything reachable through it.
///
/// Identifiers only. The edge's confidence, provenance, evidence and location
/// are on the [`cartograph_core::Edge`] the caller can fetch with
/// [`ArchitectureGraph::edge`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TraceStep {
    /// The relationship traversed.
    pub edge: EdgeId,
    /// Where it led.
    pub to: NodeId,
    /// Further hops from there.
    pub next: Vec<TraceStep>,
    /// Why the walk stopped, when it stopped here.
    pub stop: Option<Stop>,
}

/// A completed walk.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Trace {
    /// The node the walk started from.
    pub from: NodeId,
    /// The hops.
    pub steps: Vec<TraceStep>,
    /// Why the walk stopped, when it stopped immediately.
    pub stop: Option<Stop>,
    /// The depth limit that was in force.
    pub max_depth: usize,
}

/// Why a symbol query did not name exactly one node.
///
/// Both variants carry [`NodeId`]s rather than rendered strings: how a
/// candidate is described to a person is the client's decision, and two
/// clients describe it differently.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SymbolError {
    /// No node carries the name.
    NotFound {
        /// Nodes whose names are near the query, in graph order. Empty when
        /// nothing is close. Never a guess at what was meant — see
        /// [`near_misses`].
        near: Vec<NodeId>,
    },
    /// More than one node carries the name.
    Ambiguous {
        /// Every match, ordered by `(file, line, id)`.
        matches: Vec<NodeId>,
    },
}

/// Finds the single node a query names.
///
/// A bare name matches on the node's name. A query containing `:` is read as
/// `file:name` — `orders.py:create_order` — which is how a user disambiguates
/// without needing to know node ids.
///
/// # Errors
///
/// [`SymbolError::NotFound`] when no node carries the name, and
/// [`SymbolError::Ambiguous`] when more than one does.
pub fn resolve_symbol(graph: &ArchitectureGraph, query: &str) -> Result<NodeId, SymbolError> {
    let (file_hint, name) = split_query(query);

    let mut matches: Vec<&Node> = graph
        .nodes()
        .filter(|node| node.name() == name)
        .filter(|node| match file_hint {
            None => true,
            Some(hint) => node
                .location()
                .is_some_and(|location| path_matches(location.file(), hint)),
        })
        .collect();

    // Deterministic ordering: the candidate list a user sees must not depend
    // on graph iteration order.
    matches.sort_by(|a, b| {
        let key = |n: &Node| {
            (
                n.location()
                    .map(|l| l.file().to_owned())
                    .unwrap_or_default(),
                n.location().map_or(0, SourceLocation::line),
                n.id().as_u64(),
            )
        };
        key(a).cmp(&key(b))
    });

    match matches.as_slice() {
        [] => Err(SymbolError::NotFound {
            near: near_misses(graph, name),
        }),
        [only] => Ok(only.id()),
        many => Err(SymbolError::Ambiguous {
            matches: many.iter().map(|n| n.id()).collect(),
        }),
    }
}

/// Splits `file:name` into its parts, or reports the whole query as a name.
///
/// Public because a client rendering a not-found message needs the same
/// reading of the query that produced the failure, and re-splitting it by hand
/// is how the two drift apart.
#[must_use]
pub fn split_query(query: &str) -> (Option<&str>, &str) {
    match query.rsplit_once(':') {
        Some((file, name)) if !file.is_empty() && !name.is_empty() => (Some(file), name),
        _ => (None, query),
    }
}

/// Whether a node's repository-relative path satisfies a user's file hint.
///
/// Suffix matching on path segments: `orders.py` matches `app/api/orders.py`,
/// and `api/orders.py` matches it too, but `ders.py` does not.
#[must_use]
pub fn path_matches(file: &str, hint: &str) -> bool {
    let hint = hint.trim_start_matches("./").replace('\\', "/");
    if file == hint {
        return true;
    }
    file.strip_suffix(hint.as_str())
        .is_some_and(|prefix| prefix.is_empty() || prefix.ends_with('/'))
}

/// Nodes whose names are near `name`, in graph order.
///
/// Case-insensitive equality first, then substring: the two mistakes a person
/// actually makes. Not a general edit-distance search — suggesting `Order` for
/// `Odrer` is guessing, and this does not guess (RULE 010).
///
/// Ordering, de-duplication and truncation are the caller's, because they
/// happen on the *rendered* description and two clients render differently.
#[must_use]
pub fn near_misses(graph: &ArchitectureGraph, name: &str) -> Vec<NodeId> {
    graph
        .nodes()
        .filter(|node| {
            node.name().eq_ignore_ascii_case(name)
                || (name.len() >= 3 && node.name().contains(name))
        })
        .map(Node::id)
        .collect()
}

/// Walks outward from `from`, depth-first, following graph edges only.
///
/// `max_depth` bounds the number of hops. A node is not revisited while it is
/// on the current path, so a cycle terminates as [`Stop::Cycle`] rather than
/// recursing forever; the same node reached by two different branches is
/// reported on both, because two routes to one artefact are two facts.
#[must_use]
pub fn trace(graph: &ArchitectureGraph, from: NodeId, max_depth: usize) -> Trace {
    let mut visited = HashSet::new();
    visited.insert(from);
    let (steps, stop) = walk(graph, from, max_depth, &mut visited);

    Trace {
        from,
        steps,
        stop,
        max_depth,
    }
}

fn walk(
    graph: &ArchitectureGraph,
    from: NodeId,
    remaining: usize,
    visited: &mut HashSet<NodeId>,
) -> (Vec<TraceStep>, Option<Stop>) {
    let mut outgoing: Vec<&cartograph_core::Edge> = graph.edges_from(from).collect();

    // Order matters: a leaf at the depth limit has ended, not been cut short.
    if outgoing.is_empty() {
        return (Vec::new(), Some(Stop::ChainEnd));
    }
    if remaining == 0 {
        return (Vec::new(), Some(Stop::MaxDepth));
    }

    // Deterministic branch order, independent of insertion order.
    outgoing.sort_by_key(|edge| {
        (
            edge.kind() as u8,
            edge.target().as_u64(),
            edge.id().as_u64(),
        )
    });

    let mut steps = Vec::with_capacity(outgoing.len());
    for edge in outgoing {
        let target = edge.target();
        let step = if visited.contains(&target) {
            TraceStep {
                edge: edge.id(),
                to: target,
                next: Vec::new(),
                stop: Some(Stop::Cycle),
            }
        } else {
            visited.insert(target);
            let (next, stop) = walk(graph, target, remaining - 1, visited);
            visited.remove(&target);

            TraceStep {
                edge: edge.id(),
                to: target,
                next,
                stop,
            }
        };
        steps.push(step);
    }

    (steps, None)
}

#[cfg(test)]
mod tests {
    use super::*;

    use cartograph_core::{Confidence, EdgeKind, Evidence, NodeKind, Provenance, SourceLocation};

    use crate::EdgeSpec;

    /// Builds a graph from `(from, to)` name pairs, adding each node once.
    fn node(graph: &mut ArchitectureGraph, name: &str) -> NodeId {
        graph
            .node_for(
                NodeKind::Function,
                name,
                Some(SourceLocation::new("app.py", 1).expect("location")),
            )
            .expect("node")
    }

    fn graph_of(edges: &[(&str, &str)]) -> ArchitectureGraph {
        let mut graph = ArchitectureGraph::new();

        for (from, to) in edges {
            let source = node(&mut graph, from);
            let target = node(&mut graph, to);
            graph
                .add_edge(EdgeSpec {
                    source,
                    target,
                    kind: EdgeKind::Call,
                    confidence: Confidence::new(0.9).expect("confidence"),
                    provenance: Provenance::StaticImportResolution,
                    evidence: Evidence::new(format!("{from} calls {to}")).expect("evidence"),
                    location: SourceLocation::new("app.py", 1).expect("location"),
                    commit: None,
                })
                .expect("edge");
        }

        graph
    }

    fn named(graph: &ArchitectureGraph, name: &str) -> NodeId {
        resolve_symbol(graph, name).expect("one match")
    }

    // ── traversal ───────────────────────────────────────────────────

    #[test]
    fn a_single_chain_walks_to_its_end() {
        let graph = graph_of(&[("a", "b"), ("b", "c")]);
        let result = trace(&graph, named(&graph, "a"), DEFAULT_MAX_DEPTH);

        assert_eq!(result.steps.len(), 1);
        let b = &result.steps[0];
        assert_eq!(b.to, named(&graph, "b"));
        assert_eq!(b.stop, None, "b has an outgoing edge");
        assert_eq!(b.next.len(), 1);

        let c = &b.next[0];
        assert_eq!(c.to, named(&graph, "c"));
        assert_eq!(
            c.stop,
            Some(Stop::ChainEnd),
            "c has no outgoing edge, so the chain genuinely ends"
        );
        assert!(c.next.is_empty());
    }

    #[test]
    fn a_node_with_no_outgoing_edge_stops_immediately() {
        let graph = graph_of(&[("a", "b")]);
        let result = trace(&graph, named(&graph, "b"), DEFAULT_MAX_DEPTH);

        assert!(result.steps.is_empty());
        assert_eq!(result.stop, Some(Stop::ChainEnd));
    }

    #[test]
    fn the_depth_limit_stops_the_walk_and_says_so() {
        let graph = graph_of(&[("a", "b"), ("b", "c"), ("c", "d")]);
        let result = trace(&graph, named(&graph, "a"), 2);

        let b = &result.steps[0];
        let c = &b.next[0];
        assert_eq!(
            c.stop,
            Some(Stop::MaxDepth),
            "the chain continues past the limit, and the reader must be told"
        );
        assert!(c.next.is_empty(), "nothing beyond the limit is reported");
    }

    #[test]
    fn a_depth_of_zero_reports_the_limit_not_an_ending() {
        let graph = graph_of(&[("a", "b")]);
        let result = trace(&graph, named(&graph, "a"), 0);

        assert!(result.steps.is_empty());
        assert_eq!(result.stop, Some(Stop::MaxDepth));
    }

    /// A leaf at the limit has arrived, not been cut short. The order of the
    /// two checks in `walk` is what makes this true.
    #[test]
    fn a_leaf_reached_at_the_limit_is_a_chain_end_not_a_depth_stop() {
        let graph = graph_of(&[("a", "b")]);
        let result = trace(&graph, named(&graph, "b"), 0);

        assert_eq!(result.stop, Some(Stop::ChainEnd));
    }

    #[test]
    fn a_cycle_terminates_rather_than_recursing() {
        let graph = graph_of(&[("a", "b"), ("b", "c"), ("c", "a")]);
        let result = trace(&graph, named(&graph, "a"), DEFAULT_MAX_DEPTH);

        let back = &result.steps[0].next[0].next[0];
        assert_eq!(back.to, named(&graph, "a"));
        assert_eq!(back.stop, Some(Stop::Cycle));
        assert!(back.next.is_empty());
    }

    #[test]
    fn a_self_loop_is_a_cycle() {
        let graph = graph_of(&[("a", "a")]);
        let result = trace(&graph, named(&graph, "a"), DEFAULT_MAX_DEPTH);

        assert_eq!(result.steps.len(), 1);
        assert_eq!(result.steps[0].stop, Some(Stop::Cycle));
    }

    /// Two routes to one artefact are two facts. The visited set is scoped to
    /// the current path, so a diamond reports the shared node on both branches
    /// rather than suppressing the second.
    #[test]
    fn a_node_reachable_by_two_branches_is_reported_on_both() {
        let graph = graph_of(&[("a", "b"), ("a", "c"), ("b", "d"), ("c", "d")]);
        let result = trace(&graph, named(&graph, "a"), DEFAULT_MAX_DEPTH);

        assert_eq!(result.steps.len(), 2);
        for branch in &result.steps {
            assert_eq!(branch.next.len(), 1, "each branch reaches d");
            assert_eq!(branch.next[0].to, named(&graph, "d"));
            assert_eq!(branch.next[0].stop, Some(Stop::ChainEnd));
        }
    }

    #[test]
    fn branch_order_is_deterministic_and_independent_of_insertion_order() {
        let forward = graph_of(&[("a", "b"), ("a", "c"), ("a", "d")]);
        let reverse = graph_of(&[("a", "d"), ("a", "c"), ("a", "b")]);

        let key = |graph: &ArchitectureGraph| {
            trace(graph, named(graph, "a"), DEFAULT_MAX_DEPTH)
                .steps
                .iter()
                .map(|step| {
                    let edge = graph.edge(step.edge).expect("edge");
                    (edge.kind() as u8, step.to.as_u64(), step.edge.as_u64())
                })
                .collect::<Vec<_>>()
        };

        for graph in [&forward, &reverse] {
            let ordered = key(graph);
            let mut sorted = ordered.clone();
            sorted.sort_unstable();
            assert_eq!(ordered, sorted, "branches are not in sorted order");
        }
    }

    #[test]
    fn repeated_walks_of_one_graph_are_identical() {
        let graph = graph_of(&[("a", "b"), ("a", "c"), ("b", "d"), ("c", "a")]);
        let first = trace(&graph, named(&graph, "a"), DEFAULT_MAX_DEPTH);
        for _ in 0..3 {
            assert_eq!(first, trace(&graph, named(&graph, "a"), DEFAULT_MAX_DEPTH));
        }
    }

    // ── symbol resolution ───────────────────────────────────────────

    #[test]
    fn an_unknown_symbol_is_not_found() {
        let graph = graph_of(&[("a", "b")]);
        match resolve_symbol(&graph, "zzz") {
            Err(SymbolError::NotFound { near }) => assert!(near.is_empty()),
            other => panic!("expected NotFound, got {other:?}"),
        }
    }

    #[test]
    fn a_near_miss_is_offered_but_never_guessed_at() {
        let graph = graph_of(&[("create_order", "b")]);

        match resolve_symbol(&graph, "CREATE_ORDER") {
            Err(SymbolError::NotFound { near }) => {
                assert_eq!(near.len(), 1, "case-insensitive equality is a near miss");
            }
            other => panic!("expected NotFound, got {other:?}"),
        }

        match resolve_symbol(&graph, "Odrer") {
            Err(SymbolError::NotFound { near }) => {
                assert!(near.is_empty(), "a transposition is not a near miss");
            }
            other => panic!("expected NotFound, got {other:?}"),
        }
    }

    #[test]
    fn two_symbols_of_one_name_are_refused_with_both_candidates() {
        let mut graph = ArchitectureGraph::new();
        let mut ids = Vec::new();
        for file in ["a.py", "b.py"] {
            ids.push(
                graph
                    .node_for(
                        NodeKind::Class,
                        "User",
                        Some(SourceLocation::new(file, 1).expect("location")),
                    )
                    .expect("node"),
            );
        }

        match resolve_symbol(&graph, "User") {
            Err(SymbolError::Ambiguous { matches }) => {
                assert_eq!(matches.len(), 2);
                // Ordered by (file, line, id): a.py before b.py.
                let files: Vec<&str> = matches
                    .iter()
                    .map(|id| {
                        graph
                            .node(*id)
                            .and_then(Node::location)
                            .expect("location")
                            .file()
                    })
                    .collect();
                assert_eq!(files, ["a.py", "b.py"]);
            }
            other => panic!("expected Ambiguous, got {other:?}"),
        }
    }

    #[test]
    fn a_file_hint_disambiguates() {
        let mut graph = ArchitectureGraph::new();
        for file in ["app/a.py", "app/b.py"] {
            graph
                .node_for(
                    NodeKind::Class,
                    "User",
                    Some(SourceLocation::new(file, 1).expect("location")),
                )
                .expect("node");
        }

        let id = resolve_symbol(&graph, "app/b.py:User").expect("one match");
        assert_eq!(
            graph
                .node(id)
                .and_then(Node::location)
                .expect("location")
                .file(),
            "app/b.py"
        );
    }

    #[test]
    fn a_file_hint_matches_on_whole_path_segments() {
        assert!(path_matches("app/api/orders.py", "orders.py"));
        assert!(path_matches("app/api/orders.py", "api/orders.py"));
        assert!(path_matches("app/api/orders.py", "app/api/orders.py"));
        assert!(path_matches("orders.py", "orders.py"));
        assert!(path_matches("app/api/orders.py", "./orders.py"));
    }

    #[test]
    fn a_file_hint_does_not_match_a_partial_segment() {
        // `ders.py` must not match `orders.py`: a hint that matched half a
        // filename would silently pick the wrong symbol.
        assert!(!path_matches("app/api/orders.py", "ders.py"));
        assert!(!path_matches("app/api/orders.py", "reorders.py"));
        assert!(!path_matches("app/api/orders.py", "other.py"));
    }

    #[test]
    fn a_query_without_a_usable_colon_is_read_as_a_bare_name() {
        assert_eq!(split_query("create_order"), (None, "create_order"));
        assert_eq!(
            split_query("orders.py:create_order"),
            (Some("orders.py"), "create_order")
        );
        assert_eq!(split_query(":name"), (None, ":name"));
        assert_eq!(split_query("file:"), (None, "file:"));
    }

    // ── the serialised contract ─────────────────────────────────────

    #[test]
    fn stop_reasons_serialise_as_stable_slugs() {
        assert_eq!(
            serde_json::to_string(&Stop::ChainEnd).expect("json"),
            "\"chain-end\""
        );
        assert_eq!(
            serde_json::to_string(&Stop::MaxDepth).expect("json"),
            "\"max-depth\""
        );
        assert_eq!(
            serde_json::to_string(&Stop::Cycle).expect("json"),
            "\"cycle\""
        );
    }

    /// The walk reports identifiers, and every one of them resolves against
    /// the graph that produced it. Carrying anything richer would be a second
    /// copy of a fact the graph already holds.
    /// Every identifier resolves, and an edge's target agrees with the step's.
    fn assert_resolvable(graph: &ArchitectureGraph, steps: &[TraceStep]) {
        for step in steps {
            let edge = graph.edge(step.edge).expect("the edge resolves");
            assert_eq!(
                edge.target(),
                step.to,
                "the step's target must be the edge's target, not a second opinion"
            );
            assert!(graph.node(step.to).is_some(), "the node resolves");
            assert_resolvable(graph, &step.next);
        }
    }

    #[test]
    fn every_identifier_a_step_reports_resolves_in_the_graph() {
        let graph = graph_of(&[("a", "b"), ("b", "c"), ("a", "c")]);
        let result = trace(&graph, named(&graph, "a"), DEFAULT_MAX_DEPTH);
        assert_resolvable(&graph, &result.steps);
    }
}
