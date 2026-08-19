//! The complete chain the product exists to produce:
//!
//! ```text
//! CheckoutButton.tsx → POST /api/orders → create_order() → Order → orders
//! ```
//!
//! Four artefacts, three languages, one causal path — assembled from M04's
//! route matching and M06's ORM resolution over the same graph.

use cartograph_core::{EdgeKind, NodeKind, Provenance};
use cartograph_graph::ArchitectureGraph;
use cartograph_parser::Analyzer;
use cartograph_parser::model::FileAnalysis;
use cartograph_resolver::{
    MatchStatus, OrmAnalysis, RouteIndex, add_edge_for_match, add_orm_edges,
    collect_exported_constants, match_client, normalize_client_call, normalize_route_declaration,
    resolve_url, scope_for_file, with_resolved_url,
};

/// Runs every stage — extraction, dynamic URLs, routes, matching, ORM — into
/// one graph, exactly as the CLI does.
fn build(files: &[(&str, &str)]) -> ArchitectureGraph {
    let mut analyzer = Analyzer::new().expect("grammars load");
    let analyses: Vec<FileAnalysis> = files
        .iter()
        .map(|(p, s)| analyzer.analyze_source(p, s).expect("fixture parses"))
        .collect();

    let mut graph = ArchitectureGraph::new();

    // M03/M04/M05: client → handler.
    let exported = collect_exported_constants(&analyses);
    let index = RouteIndex::build(analyses.iter().flat_map(|f| {
        f.routes
            .iter()
            .map(move |r| normalize_route_declaration(r, &f.path, f.language))
    }));
    for file in &analyses {
        let scope = scope_for_file(file, &exported);
        for call in &file.http_calls {
            let effective = resolve_url(call, file, &scope)
                .as_ref()
                .map_or_else(|| call.clone(), |r| with_resolved_url(call, r));
            let canonical = normalize_client_call(&effective, &file.path, file.language);
            let result = match_client(&canonical, &index);
            assert_ne!(
                result.status,
                MatchStatus::Ambiguous,
                "fixture should be unambiguous"
            );
            add_edge_for_match(&mut graph, &result, None).expect("endpoints exist");
        }
    }

    // M06: handler → model → table.
    add_orm_edges(&mut graph, &OrmAnalysis::build(&analyses), None).expect("endpoints exist");
    graph
}

const CHECKOUT_TSX: &str = r#"import axios from "axios";

export function CheckoutButton({ orderId }: { orderId: string }) {
    async function onSubmit() {
        await axios.post("/api/orders", { orderId });
    }
    return onSubmit;
}"#;

/// Walks a named node's single outgoing edge, returning the target's name.
fn hop<'a>(graph: &'a ArchitectureGraph, from: &str) -> (&'a str, EdgeKind) {
    let node = graph
        .nodes()
        .find(|n| n.name() == from)
        .unwrap_or_else(|| panic!("no node named `{from}`"));
    let edge = graph
        .edges_from(node.id())
        .next()
        .unwrap_or_else(|| panic!("`{from}` has no outgoing edge"));
    let target = graph.node(edge.target()).expect("edge target exists");
    (target.name(), edge.kind())
}

#[test]
fn the_sqlalchemy_chain_is_walkable_end_to_end() {
    let graph = build(&[
        ("web/src/CheckoutButton.tsx", CHECKOUT_TSX),
        (
            "api/models.py",
            r#"
from sqlalchemy.orm import DeclarativeBase

class Base(DeclarativeBase):
    pass

class Order(Base):
    __tablename__ = "orders"
"#,
        ),
        (
            "api/orders.py",
            r#"
from fastapi import FastAPI
from .models import Order

app = FastAPI()


@app.post("/api/orders")
async def create_order(payload: dict):
    return Order(**payload)
"#,
        ),
    ]);

    // CheckoutButton.tsx → create_order → Order → orders
    let (handler, kind) = hop(&graph, "web/src/CheckoutButton.tsx");
    assert_eq!((handler, kind), ("create_order", EdgeKind::HttpCall));

    let (model, kind) = hop(&graph, "create_order");
    assert_eq!((model, kind), ("Order", EdgeKind::OrmAccess));

    let (table, kind) = hop(&graph, "Order");
    assert_eq!((table, kind), ("orders", EdgeKind::Queries));

    assert_eq!(
        graph.nodes().find(|n| n.name() == "orders").unwrap().kind(),
        NodeKind::Table
    );
}

#[test]
fn the_django_chain_is_walkable_end_to_end() {
    let graph = build(&[
        (
            "web/src/Invoices.tsx",
            r#"import axios from "axios";
await axios.get("/api/invoices");"#,
        ),
        (
            "api/models.py",
            r#"
from django.db import models

class Invoice(models.Model):
    class Meta:
        db_table = "invoice_records"
"#,
        ),
        (
            "api/views.py",
            r#"
from fastapi import FastAPI
from .models import Invoice

app = FastAPI()


@app.get("/api/invoices")
async def list_invoices():
    return Invoice.objects.filter(paid=True)
"#,
        ),
    ]);

    assert_eq!(
        hop(&graph, "web/src/Invoices.tsx"),
        ("list_invoices", EdgeKind::HttpCall)
    );
    assert_eq!(
        hop(&graph, "list_invoices"),
        ("Invoice", EdgeKind::OrmAccess)
    );
    assert_eq!(
        hop(&graph, "Invoice"),
        ("invoice_records", EdgeKind::Queries)
    );
}

#[test]
fn the_handler_is_exactly_one_node_across_both_analyses() {
    // The ADR-0011 defect, reproduced through the real pipeline rather than a
    // synthetic graph: M04 creates a Function node for the route's handler and
    // M06 creates one for the handler that touches the model. If they diverge,
    // the count is two and the chain silently breaks.
    let graph = build(&[
        ("web/src/CheckoutButton.tsx", CHECKOUT_TSX),
        (
            "api/models.py",
            "from sqlalchemy.orm import DeclarativeBase\nclass Base(DeclarativeBase):\n    pass\nclass Order(Base):\n    __tablename__ = \"orders\"\n",
        ),
        (
            "api/orders.py",
            "from fastapi import FastAPI\napp = FastAPI()\n\n@app.post(\"/api/orders\")\nasync def create_order(payload: dict):\n    return Order(**payload)\n",
        ),
    ]);

    let handlers: Vec<_> = graph
        .nodes()
        .filter(|n| n.name() == "create_order")
        .collect();
    assert_eq!(
        handlers.len(),
        1,
        "two analyses described one handler; it must be one node, found {}",
        handlers.len()
    );

    let handler = handlers[0];
    assert_eq!(
        graph.edges_to(handler.id()).count(),
        1,
        "the route matcher's edge arrives here"
    );
    assert_eq!(
        graph.edges_from(handler.id()).count(),
        1,
        "and the ORM edge leaves from the same node"
    );

    // One node per artefact throughout: client file, handler, model, table.
    assert_eq!(graph.node_count(), 4, "no duplicated artefacts");
}

#[test]
fn every_edge_in_the_chain_explains_itself() {
    let graph = build(&[
        ("web/src/CheckoutButton.tsx", CHECKOUT_TSX),
        (
            "api/models.py",
            "from sqlalchemy.orm import DeclarativeBase\nclass Base(DeclarativeBase):\n    pass\nclass Order(Base):\n    __tablename__ = \"orders\"\n",
        ),
        (
            "api/orders.py",
            "from fastapi import FastAPI\napp = FastAPI()\n\n@app.post(\"/api/orders\")\nasync def create_order(payload: dict):\n    return Order(**payload)\n",
        ),
    ]);

    for edge in graph.edges() {
        assert!(
            !edge.evidence().as_str().is_empty(),
            "an edge without evidence"
        );
        assert!(!edge.location().file().is_empty());
        // Each hop names the analysis that produced it, and no hop is inferred
        // by a model (RULE 007).
        assert!(matches!(
            edge.provenance(),
            Provenance::RouteMatcher | Provenance::OrmResolution
        ));
    }

    let orm_edge = graph
        .edges()
        .find(|e| e.kind() == EdgeKind::Queries)
        .expect("model→table edge");
    assert!(orm_edge.evidence().as_str().contains("orders"));
}

#[test]
fn a_broken_link_leaves_the_chain_incomplete_rather_than_inventing_one() {
    // The handler exists and matches, but the model declares no table.
    let graph = build(&[
        ("web/src/CheckoutButton.tsx", CHECKOUT_TSX),
        (
            "api/models.py",
            "from sqlalchemy.orm import DeclarativeBase\nclass Base(DeclarativeBase):\n    pass\nclass Order(Base):\n    pass\n",
        ),
        (
            "api/orders.py",
            "from fastapi import FastAPI\napp = FastAPI()\n\n@app.post(\"/api/orders\")\nasync def create_order(payload: dict):\n    return Order(**payload)\n",
        ),
    ]);

    assert_eq!(hop(&graph, "create_order"), ("Order", EdgeKind::OrmAccess));
    assert_eq!(
        graph
            .edges()
            .filter(|e| e.kind() == EdgeKind::Queries)
            .count(),
        0,
        "no table was declared, so no table edge — the chain stops honestly"
    );
    assert!(graph.nodes().all(|n| n.kind() != NodeKind::Table));
}
