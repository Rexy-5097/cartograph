//! Integration tests for the architecture graph.
//!
//! These exercise the graph through its public API only. If a test here needs
//! a petgraph type to compile, the abstraction has leaked and ADR-0003 has been
//! violated — that is the point of testing from outside the crate.

use cartograph_core::{Confidence, EdgeKind, Evidence, NodeKind, Provenance, SourceLocation};
use cartograph_graph::{ArchitectureGraph, EdgeSpec, GraphError};
use cartograph_testkit::{cross_stack_chain, fixed_clock};

fn location(file: &str, line: u32) -> SourceLocation {
    SourceLocation::new(file, line).expect("repository-relative path")
}

fn evidence(text: &str) -> Evidence {
    Evidence::new(text).expect("non-empty evidence")
}

#[test]
fn a_new_graph_is_empty() {
    let graph = ArchitectureGraph::new();
    assert_eq!(graph.node_count(), 0);
    assert_eq!(graph.edge_count(), 0);
}

#[test]
fn nodes_are_retrievable_by_the_handle_they_were_given() {
    let mut graph = ArchitectureGraph::new();

    let handler = graph
        .add_node(
            NodeKind::Function,
            "create_order",
            Some(location("api/orders.py", 12)),
        )
        .expect("valid node");

    let node = graph.node(handler).expect("node was just added");
    assert_eq!(node.name(), "create_order");
    assert_eq!(node.kind(), NodeKind::Function);
    assert_eq!(
        node.location().map(ToString::to_string).as_deref(),
        Some("api/orders.py:12")
    );
    assert_eq!(graph.node_count(), 1);
}

#[test]
fn every_edge_carries_the_case_for_itself() {
    let mut graph = ArchitectureGraph::with_clock(fixed_clock);

    let button = graph
        .add_node(
            NodeKind::Function,
            "onSubmit",
            Some(location("web/src/CheckoutButton.tsx", 34)),
        )
        .expect("valid node");
    let handler = graph
        .add_node(
            NodeKind::Function,
            "create_order",
            Some(location("api/orders.py", 12)),
        )
        .expect("valid node");

    let id = graph
        .add_edge(EdgeSpec {
            source: button,
            target: handler,
            kind: EdgeKind::HttpCall,
            confidence: Confidence::new(0.94).expect("in the unit interval"),
            provenance: Provenance::RouteMatcher,
            evidence: evidence("POST /api/orders"),
            location: location("web/src/CheckoutButton.tsx", 34),
            commit: None,
        })
        .expect("both endpoints exist");

    let edge = graph.edge(id).expect("edge was just added");
    assert_eq!(edge.kind(), EdgeKind::HttpCall);
    assert_eq!(edge.confidence().to_string(), "0.94");
    assert_eq!(edge.provenance(), Provenance::RouteMatcher);
    assert_eq!(edge.evidence().as_str(), "POST /api/orders");
    assert_eq!(edge.location().to_string(), "web/src/CheckoutButton.tsx:34");
    assert_eq!(edge.commit(), None, "no Git integration until M07");
}

#[test]
fn an_edge_to_a_node_that_was_never_found_is_refused() {
    let mut graph = ArchitectureGraph::new();

    let known = graph
        .add_node(
            NodeKind::File,
            "app.ts",
            Some(location("web/src/app.ts", 1)),
        )
        .expect("valid node");

    // A handle this graph never issued. This is what an import that resolved
    // to nothing looks like by the time it reaches the graph.
    let never_added = cartograph_core::NodeId::from_raw(u64::MAX);

    let result = graph.add_edge(EdgeSpec {
        source: known,
        target: never_added,
        kind: EdgeKind::Import,
        confidence: Confidence::STATIC_IMPORT,
        provenance: Provenance::StaticImportResolution,
        evidence: evidence("import './elsewhere'"),
        location: location("web/src/app.ts", 1),
        commit: None,
    });

    assert!(
        matches!(result, Err(GraphError::UnknownNode { .. })),
        "an unresolved target must be refused, not invented as a placeholder"
    );
    assert_eq!(graph.edge_count(), 0);
}

#[test]
fn traversal_distinguishes_direction() {
    let mut graph = ArchitectureGraph::with_clock(fixed_clock);

    let checkout = graph
        .add_node(NodeKind::Function, "checkout", None)
        .expect("valid");
    let create_order = graph
        .add_node(NodeKind::Function, "create_order", None)
        .expect("valid");

    graph
        .add_edge(EdgeSpec {
            source: checkout,
            target: create_order,
            kind: EdgeKind::Call,
            confidence: Confidence::LSP_EXACT_SYMBOL,
            provenance: Provenance::LspSymbolResolution,
            evidence: evidence("create_order()"),
            location: location("api/orders.py", 30),
            commit: None,
        })
        .expect("both endpoints exist");

    assert_eq!(graph.edges_from(checkout).count(), 1);
    assert_eq!(graph.edges_to(checkout).count(), 0);
    assert_eq!(graph.edges_from(create_order).count(), 0);
    assert_eq!(
        graph.edges_to(create_order).count(),
        1,
        "edges_to is the primitive blast radius is built on"
    );
}

#[test]
fn traversal_from_an_unknown_node_is_empty_rather_than_a_panic() {
    let graph = ArchitectureGraph::new();
    let never_added = cartograph_core::NodeId::from_raw(999);

    assert_eq!(graph.edges_from(never_added).count(), 0);
    assert_eq!(graph.edges_to(never_added).count(), 0);
    assert!(graph.node(never_added).is_none());
}

#[test]
fn the_cross_stack_chain_survives_a_round_trip_through_the_graph() {
    let mut graph = ArchitectureGraph::with_clock(fixed_clock);
    let chain = cross_stack_chain();

    // Add each distinct artefact once, in chain order.
    let mut handles = Vec::new();
    handles.push(
        graph
            .add_node(
                chain[0].source.kind(),
                chain[0].source.name(),
                chain[0].source.location().cloned(),
            )
            .expect("valid node"),
    );
    for hop in &chain {
        handles.push(
            graph
                .add_node(
                    hop.target.kind(),
                    hop.target.name(),
                    hop.target.location().cloned(),
                )
                .expect("valid node"),
        );
    }

    for (i, hop) in chain.iter().enumerate() {
        graph
            .add_edge(EdgeSpec {
                source: handles[i],
                target: handles[i + 1],
                kind: hop.edge.kind(),
                confidence: hop.edge.confidence(),
                provenance: hop.edge.provenance(),
                evidence: hop.edge.evidence().clone(),
                location: hop.edge.location().clone(),
                commit: hop.edge.commit().cloned(),
            })
            .expect("both endpoints exist");
    }

    assert_eq!(graph.node_count(), 5, "four hops touch five artefacts");
    assert_eq!(graph.edge_count(), 4);

    // Walk the chain the way `cartograph trace` eventually will.
    let mut current = handles[0];
    let mut walked = 0;
    while let Some(edge) = graph.edges_from(current).next() {
        walked += 1;
        current = edge.target();
    }
    assert_eq!(walked, 4, "the chain is walkable end to end");
    assert_eq!(
        graph.node(current).expect("terminal node").kind(),
        NodeKind::Table,
        "the chain ends at a database table"
    );
}
