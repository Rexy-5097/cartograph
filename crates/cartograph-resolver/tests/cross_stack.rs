//! End-to-end cross-stack fixtures: real TypeScript and Python source, parsed,
//! canonicalised, matched, and turned into graph edges.
//!
//! These run the whole pipeline the specification describes — AST extraction →
//! HTTP call sites → route extraction → normalisation → matching → confidence →
//! edge — so they answer the question the milestone exists to answer: *can
//! Cartograph connect a TypeScript call to a Python handler without guessing?*
//!
//! The negative fixtures matter at least as much as the positive ones. A
//! resolver that connects everything is worthless; the value is in what it
//! refuses.

use cartograph_core::{EdgeKind, NodeKind, Provenance};
use cartograph_graph::ArchitectureGraph;
use cartograph_parser::Analyzer;
use cartograph_parser::model::SourceLanguage;
use cartograph_resolver::{
    MatchResult, MatchStatus, RouteIndex, UnsupportedReason, add_edge_for_match, match_client,
    normalize_client_call, normalize_route_declaration,
};

/// Runs the full pipeline over one TypeScript file and one Python file.
struct Fixture {
    results: Vec<MatchResult>,
    graph: ArchitectureGraph,
    edges: usize,
}

impl Fixture {
    fn build(client_src: &str, client_path: &str, server_src: &str, server_path: &str) -> Self {
        let mut analyzer = Analyzer::new().expect("grammars load");

        let client_analysis = analyzer
            .analyze_source(client_path, client_src)
            .expect("client fixture parses");
        let server_analysis = analyzer
            .analyze_source(server_path, server_src)
            .expect("server fixture parses");

        let routes = server_analysis.routes.iter().map(|r| {
            normalize_route_declaration(r, &server_analysis.path, server_analysis.language)
        });
        let index = RouteIndex::build(routes);

        let results: Vec<MatchResult> = client_analysis
            .http_calls
            .iter()
            .map(|c| normalize_client_call(c, &client_analysis.path, client_analysis.language))
            .map(|client| match_client(&client, &index))
            .collect();

        let mut graph = ArchitectureGraph::new();
        let mut edges = 0;
        for result in &results {
            if add_edge_for_match(&mut graph, result, None)
                .expect("graph accepts endpoints it created")
                .is_some()
            {
                edges += 1;
            }
        }

        Self {
            results,
            graph,
            edges,
        }
    }

    fn only(&self) -> &MatchResult {
        assert_eq!(
            self.results.len(),
            1,
            "fixture should produce exactly one client observation"
        );
        &self.results[0]
    }
}

const FASTAPI_ORDERS: &str = r#"
from fastapi import FastAPI

app = FastAPI()


@app.post("/api/orders")
async def create_order(payload: dict):
    return payload
"#;

// ── Fixture A: exact literal match ──────────────────────────────────

#[test]
fn fixture_a_exact_path_and_method_produces_one_edge() {
    let fixture = Fixture::build(
        r#"import axios from "axios";
export async function submit() {
    await axios.post("/api/orders", {});
}"#,
        "web/src/CheckoutButton.tsx",
        FASTAPI_ORDERS,
        "api/orders.py",
    );

    let result = fixture.only();
    assert_eq!(result.status, MatchStatus::Exact);
    assert_eq!(fixture.edges, 1, "exactly one HttpCall edge");

    let candidate = result.accepted().expect("accepted match");
    assert_eq!(
        candidate.confidence,
        cartograph_resolver::priors::EXACT_ROUTE_AND_METHOD
    );
    assert_eq!(
        candidate.route.provenance.handler.as_deref(),
        Some("create_order"),
        "the handler comes from the decorated function, read syntactically"
    );
}

#[test]
fn fixture_a_edge_carries_the_case_for_itself() {
    let fixture = Fixture::build(
        r#"import axios from "axios";
await axios.post("/api/orders", {});"#,
        "web/src/CheckoutButton.tsx",
        FASTAPI_ORDERS,
        "api/orders.py",
    );

    let edge = fixture.graph.edges().next().expect("one edge");
    assert_eq!(edge.kind(), EdgeKind::HttpCall);
    assert_eq!(edge.provenance(), Provenance::RouteMatcher);
    assert!(edge.is_deterministic(), "static matching is not inference");
    assert_eq!(edge.confidence().to_string(), "0.98");

    let evidence = edge.evidence().as_str();
    assert!(
        evidence.contains("POST /api/orders"),
        "canonical route: {evidence}"
    );
    assert!(
        evidence.contains("api/orders.py"),
        "backend file: {evidence}"
    );
    assert!(evidence.contains("create_order"), "handler: {evidence}");
    assert!(
        evidence.contains("exact HTTP method"),
        "method rule: {evidence}"
    );
    assert!(
        evidence.contains("every path segment static and equal"),
        "path rule: {evidence}"
    );

    // The claim was observed at the client call site.
    assert_eq!(edge.location().file(), "web/src/CheckoutButton.tsx");
    assert_eq!(edge.commit(), None, "no Git integration until M07");
}

#[test]
fn fixture_a_endpoints_are_the_client_file_and_the_handler() {
    let fixture = Fixture::build(
        r#"await axios.post("/api/orders", {});"#,
        "web/src/CheckoutButton.tsx",
        FASTAPI_ORDERS,
        "api/orders.py",
    );

    let edge = fixture.graph.edges().next().unwrap();
    let source = fixture.graph.node(edge.source()).unwrap();
    let target = fixture.graph.node(edge.target()).unwrap();

    assert_eq!(source.kind(), NodeKind::File);
    assert_eq!(source.name(), "web/src/CheckoutButton.tsx");
    assert_eq!(target.kind(), NodeKind::Function);
    assert_eq!(target.name(), "create_order");
    assert_eq!(
        target.location().map(cartograph_core::SourceLocation::file),
        Some("api/orders.py")
    );
}

// ── Fixture B: concrete value into a declared parameter ─────────────

#[test]
fn fixture_b_concrete_value_matches_a_declared_parameter() {
    let fixture = Fixture::build(
        r#"await axios.post("/api/orders/123", {});"#,
        "web/src/Order.tsx",
        r#"
from fastapi import FastAPI
app = FastAPI()

@app.post("/api/orders/{id}")
async def replace_order(id: str):
    pass
"#,
        "api/orders.py",
    );

    let result = fixture.only();
    assert_eq!(
        result.status,
        MatchStatus::Strong,
        "a bound parameter is compatible but not fully determined"
    );
    assert_eq!(fixture.edges, 1);

    let evidence = fixture.graph.edges().next().unwrap().evidence().as_str();
    assert!(
        evidence.contains("bound to concrete values"),
        "the binding must be visible in the evidence: {evidence}"
    );
}

// ── Fixture C: method mismatch ──────────────────────────────────────

#[test]
fn fixture_c_method_mismatch_produces_no_edge() {
    let fixture = Fixture::build(
        r#"await axios.get("/api/orders");"#,
        "web/src/Orders.tsx",
        FASTAPI_ORDERS,
        "api/orders.py",
    );

    assert_eq!(fixture.only().status, MatchStatus::NoMatch);
    assert_eq!(fixture.edges, 0, "GET must not reach a POST-only route");
    assert_eq!(fixture.graph.edge_count(), 0);
}

// ── Fixture D: different static path ────────────────────────────────

#[test]
fn fixture_d_different_resource_produces_no_edge() {
    let fixture = Fixture::build(
        r#"await axios.post("/api/users/123", {});"#,
        "web/src/User.tsx",
        r#"
from fastapi import FastAPI
app = FastAPI()

@app.post("/api/orders/{id}")
async def replace_order(id: str):
    pass
"#,
        "api/orders.py",
    );

    assert_eq!(fixture.only().status, MatchStatus::NoMatch);
    assert_eq!(fixture.edges, 0, "users is not orders");
}

// ── Fixture E: unresolved template prefix ───────────────────────────

#[test]
fn fixture_e_unknown_prefix_is_refused_rather_than_aligned() {
    let fixture = Fixture::build(
        r#"const BASE = "/api";
await axios.post(`${BASE}/orders/${id}`, {});"#,
        "web/src/Order.tsx",
        r#"
from fastapi import FastAPI
app = FastAPI()

@app.post("/orders/{id}")
async def replace_order(id: str):
    pass
"#,
        "api/orders.py",
    );

    let result = fixture.only();
    assert_eq!(
        result.status,
        MatchStatus::Unsupported,
        "an unknown prefix means an unknown path length; aligning would be a guess"
    );
    assert_eq!(
        result.unsupported_reason,
        Some(UnsupportedReason::ClientPrefixUnresolved)
    );
    assert_eq!(fixture.edges, 0);
    // M05's constant propagation is what will resolve BASE and unlock this.
}

// ── Fixture F: genuine ambiguity ────────────────────────────────────

#[test]
fn fixture_f_two_equally_plausible_routes_are_ambiguous_and_produce_no_edge() {
    let fixture = Fixture::build(
        r#"await axios.post("/api/orders/123", {});"#,
        "web/src/Order.tsx",
        r#"
from fastapi import FastAPI
app = FastAPI()

@app.post("/api/orders/{id}")
async def by_id(id: str):
    pass

@app.post("/api/orders/{slug}")
async def by_slug(slug: str):
    pass
"#,
        "api/orders.py",
    );

    let result = fixture.only();
    assert_eq!(result.status, MatchStatus::Ambiguous);
    assert_eq!(result.candidates.len(), 2, "both candidates are retained");
    assert_eq!(
        fixture.edges, 0,
        "picking a winner would make every downstream confidence number a guess"
    );
    assert!(
        result.accepted().is_none(),
        "an ambiguous result exposes no accepted candidate"
    );
}

// ── Fixture G: undeclared client method ─────────────────────────────

#[test]
fn fixture_g_unknown_client_method_is_not_an_exact_match() {
    // `fetch("/api/orders")` with no options declares no method.
    let fixture = Fixture::build(
        r#"await fetch("/api/orders");"#,
        "web/src/Orders.tsx",
        FASTAPI_ORDERS,
        "api/orders.py",
    );

    let result = fixture.only();
    assert_eq!(
        result.status,
        MatchStatus::Strong,
        "compatible, but the method was never declared"
    );
    let candidate = result.accepted().unwrap();
    assert!(
        candidate.confidence.get() < cartograph_resolver::priors::EXACT_ROUTE_AND_METHOD.get(),
        "an undeclared method must weaken the claim"
    );
    let evidence = fixture.graph.edges().next().unwrap().evidence().as_str();
    assert!(
        evidence.contains("client declared no method"),
        "the uncertainty must be stated: {evidence}"
    );
}

// ── Fixture H: unsupported backend route ────────────────────────────

#[test]
fn fixture_h_regex_route_is_never_matched() {
    let fixture = Fixture::build(
        r#"await axios.get("/archive/2024");"#,
        "web/src/Archive.tsx",
        r#"
from django.urls import re_path
from . import views

urlpatterns = [
    re_path(r"^archive/(?P<year>[0-9]{4})$", views.archive),
]
"#,
        "api/urls.py",
    );

    assert_eq!(
        fixture.only().status,
        MatchStatus::NoMatch,
        "an uninterpreted regex offers no structure to match against"
    );
    assert_eq!(fixture.edges, 0);
}

// ── Python client against Python routes ─────────────────────────────

#[test]
fn a_python_client_call_matches_a_python_route() {
    // Both sides are Python here: matching joins observations, not languages.
    let mut analyzer = Analyzer::new().unwrap();
    let client = analyzer
        .analyze_source(
            "worker/tasks.py",
            r#"
import requests

def sync():
    requests.post("/api/orders")
"#,
        )
        .unwrap();
    let server = analyzer
        .analyze_source("api/orders.py", FASTAPI_ORDERS)
        .unwrap();

    let index = RouteIndex::build(
        server
            .routes
            .iter()
            .map(|r| normalize_route_declaration(r, &server.path, SourceLanguage::Python)),
    );
    let call = normalize_client_call(&client.http_calls[0], &client.path, SourceLanguage::Python);
    let result = match_client(&call, &index);

    assert_eq!(result.status, MatchStatus::Exact);
    assert_eq!(
        result.client.provenance.language,
        SourceLanguage::Python,
        "the client language travels with the evidence"
    );
}

// ── The specification's worked chain ────────────────────────────────

#[test]
fn the_specifications_checkout_chain_resolves_end_to_end() {
    // CheckoutButton.tsx → POST /api/orders → orders.py create_order
    let fixture = Fixture::build(
        r#"import axios from "axios";

export function CheckoutButton({ orderId }: { orderId: string }) {
    async function onSubmit() {
        await axios.post("/api/orders", { orderId });
    }
    return onSubmit;
}"#,
        "web/src/CheckoutButton.tsx",
        r#"
from fastapi import FastAPI

app = FastAPI()


@app.post("/api/orders")
async def create_order(payload: dict):
    return payload
"#,
        "api/orders.py",
    );

    assert_eq!(fixture.only().status, MatchStatus::Exact);
    assert_eq!(fixture.edges, 1);

    let edge = fixture.graph.edges().next().unwrap();
    let source = fixture.graph.node(edge.source()).unwrap();
    let target = fixture.graph.node(edge.target()).unwrap();

    // The hop the whole product exists to produce.
    //
    // The source is the function that issues the request, not the file that
    // contains it. M04 named the file because the enclosing scope of a call
    // site was not resolved; it is now, and a generated client's wrapper
    // function has to be a node in its own right or a component cannot be
    // linked through it to a backend handler.
    assert_eq!(source.name(), "onSubmit");
    assert_eq!(
        source.location().map(cartograph_core::SourceLocation::file),
        Some("web/src/CheckoutButton.tsx"),
        "the calling file is still recorded, on the node's location"
    );
    assert_eq!(target.name(), "create_order");
    assert_eq!(edge.kind(), EdgeKind::HttpCall);
    assert_eq!(edge.provenance(), Provenance::RouteMatcher);
    assert_eq!(edge.confidence().to_string(), "0.98");
}
