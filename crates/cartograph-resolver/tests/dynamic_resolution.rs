//! End-to-end dynamic URL resolution: source → symbolic value → canonical
//! route → match decision or refusal.
//!
//! Slices 3, 7 and 8. The refusal cases are the load-bearing half: M05's
//! obvious failure mode is that resolving more URLs produces more matches,
//! including wrong ones.

use cartograph_core::Provenance;
use cartograph_graph::ArchitectureGraph;
use cartograph_parser::Analyzer;
use cartograph_parser::model::{FileAnalysis, SourceLanguage};
use cartograph_resolver::{
    MatchStatus, RouteIndex, UnsupportedReason, add_edge_for_match, collect_exported_constants,
    match_client, normalize_client_call, normalize_route_declaration, resolve_url, scope_for_file,
    with_resolved_url,
};

/// One client call's outcome: status, refusal reason, and the derived URL.
type Decision = (MatchStatus, Option<UnsupportedReason>, String);

/// A whole project: several files, analysed, resolved and matched.
struct Project {
    files: Vec<FileAnalysis>,
}

impl Project {
    fn new(files: &[(&str, &str)]) -> Self {
        let mut analyzer = Analyzer::new().expect("grammars load");
        Self {
            files: files
                .iter()
                .map(|(path, src)| analyzer.analyze_source(path, src).expect("fixture parses"))
                .collect(),
        }
    }

    /// Runs the full M05 → M03 → M04 pipeline, returning decisions and edges.
    fn run(&self) -> (Vec<Decision>, usize, ArchitectureGraph) {
        let exported = collect_exported_constants(&self.files);
        let routes = self.files.iter().flat_map(|f| {
            f.routes
                .iter()
                .map(move |r| normalize_route_declaration(r, &f.path, f.language))
        });
        let index = RouteIndex::build(routes);

        let mut decisions = Vec::new();
        let mut graph = ArchitectureGraph::new();
        let mut edges = 0;

        for file in &self.files {
            let scope = scope_for_file(file, &exported);
            for call in &file.http_calls {
                // M05: resolve the URL if it is not already literal.
                let resolved = resolve_url(call, file, &scope);
                let effective = resolved
                    .as_ref()
                    .map_or_else(|| call.clone(), |r| with_resolved_url(call, r));
                let derived = resolved
                    .as_ref()
                    .map_or_else(|| format!("{:?}", call.url), |r| r.value.to_string());

                // M03/M04 unchanged.
                let canonical = normalize_client_call(&effective, &file.path, file.language);
                let result = match_client(&canonical, &index);
                if add_edge_for_match(&mut graph, &result, None)
                    .expect("graph accepts its own endpoints")
                    .is_some()
                {
                    edges += 1;
                }
                decisions.push((result.status, result.unsupported_reason, derived));
            }
        }
        (decisions, edges, graph)
    }
}

const ORDERS_API: &str = r#"
from fastapi import FastAPI
app = FastAPI()

@app.post("/api/v1/orders")
async def create_order(payload: dict):
    pass

@app.get("/api/v1/orders/{id}")
async def get_order(id: str):
    pass
"#;

// ── Slice 3: cross-file module constants ────────────────────────────

#[test]
fn an_imported_constant_resolves_the_url_and_produces_an_edge() {
    // The milestone's headline case: M04 refused this outright.
    let project = Project::new(&[
        ("web/src/config.ts", r#"export const API_BASE = "/api/v1";"#),
        (
            "web/src/orders.ts",
            r#"import { API_BASE } from "./config";
await axios.post(`${API_BASE}/orders`, {});"#,
        ),
        ("api/orders.py", ORDERS_API),
    ]);

    let (decisions, edges, graph) = project.run();
    assert_eq!(decisions.len(), 1);
    assert_eq!(
        decisions[0].2, "/api/v1/orders",
        "URL fully resolved across files"
    );
    assert_eq!(decisions[0].0, MatchStatus::Exact);
    assert_eq!(edges, 1);

    let edge = graph.edges().next().unwrap();
    assert_eq!(edge.provenance(), Provenance::RouteMatcher);
    assert!(
        graph.node(edge.target()).unwrap().name() == "create_order",
        "the chain reaches the handler"
    );
}

#[test]
fn an_imported_constant_with_an_unknown_suffix_still_matches_a_parameter() {
    let project = Project::new(&[
        ("web/src/config.ts", r#"export const API_BASE = "/api/v1";"#),
        (
            "web/src/orders.ts",
            r#"import { API_BASE } from "./config";
await axios.get(`${API_BASE}/orders/${orderId}`);"#,
        ),
        ("api/orders.py", ORDERS_API),
    ]);

    let (decisions, edges, _) = project.run();
    assert_eq!(decisions[0].2, "/api/v1/orders/{unknown}");
    assert_eq!(
        decisions[0].0,
        MatchStatus::Strong,
        "the id position stays undetermined"
    );
    assert_eq!(edges, 1);
}

#[test]
fn an_aliased_import_binds_under_its_local_name() {
    let project = Project::new(&[
        ("web/src/config.ts", r#"export const API_BASE = "/api/v1";"#),
        (
            "web/src/orders.ts",
            r#"import { API_BASE as ROOT } from "./config";
await axios.post(`${ROOT}/orders`, {});"#,
        ),
        ("api/orders.py", ORDERS_API),
    ]);
    assert_eq!(project.run().0[0].0, MatchStatus::Exact);
}

// ── NEGATIVE: what must not resolve ─────────────────────────────────

#[test]
fn a_constant_that_is_not_exported_is_not_visible_to_importers() {
    let project = Project::new(&[
        ("web/src/config.ts", r#"const API_BASE = "/api/v1";"#), // no export
        (
            "web/src/orders.ts",
            r#"import { API_BASE } from "./config";
await axios.post(`${API_BASE}/orders`, {});"#,
        ),
        ("api/orders.py", ORDERS_API),
    ]);

    let (decisions, edges, _) = project.run();
    assert_eq!(
        decisions[0].1,
        Some(UnsupportedReason::ClientPrefixUnresolved),
        "an unexported constant is not visible; resolving it would assert what the module cannot see"
    );
    assert_eq!(edges, 0, "NO EDGE IS PRODUCED");
}

#[test]
fn a_package_import_is_not_followed() {
    let project = Project::new(&[
        (
            "web/src/orders.ts",
            r#"import { API_BASE } from "some-package";
await axios.post(`${API_BASE}/orders`, {});"#,
        ),
        ("api/orders.py", ORDERS_API),
    ]);

    let (decisions, edges, _) = project.run();
    assert_eq!(
        decisions[0].1,
        Some(UnsupportedReason::ClientPrefixUnresolved)
    );
    assert_eq!(edges, 0, "NO EDGE IS PRODUCED");
}

#[test]
fn an_environment_backed_prefix_is_refused_not_matched() {
    let project = Project::new(&[
        (
            "web/src/orders.ts",
            r"await axios.post(`${process.env.API_URL}/orders`, {});",
        ),
        ("api/orders.py", ORDERS_API),
    ]);

    let (decisions, edges, _) = project.run();
    assert_eq!(
        decisions[0].2, "{env:API_URL}/orders",
        "the variable is named, never read"
    );
    assert_eq!(
        decisions[0].1,
        Some(UnsupportedReason::ClientPrefixUnresolved),
        "an unknown prefix means an unknown segment count"
    );
    assert_eq!(edges, 0, "NO EDGE IS PRODUCED");
}

#[test]
fn an_unsupported_expression_is_refused_not_matched() {
    let project = Project::new(&[
        (
            "web/src/orders.ts",
            r"await axios.post(`${buildBase()}/orders`, {});",
        ),
        ("api/orders.py", ORDERS_API),
    ]);

    let (decisions, edges, _) = project.run();
    assert!(decisions[0].2.contains("{unsupported}"));
    assert_eq!(edges, 0, "NO EDGE IS PRODUCED");
}

#[test]
fn a_resolved_url_to_a_different_resource_still_does_not_match() {
    let project = Project::new(&[
        ("web/src/config.ts", r#"export const API_BASE = "/api/v1";"#),
        (
            "web/src/users.ts",
            r#"import { API_BASE } from "./config";
await axios.post(`${API_BASE}/users`, {});"#,
        ),
        ("api/orders.py", ORDERS_API),
    ]);

    let (decisions, edges, _) = project.run();
    assert_eq!(decisions[0].2, "/api/v1/users", "resolved correctly…");
    assert_eq!(
        decisions[0].0,
        MatchStatus::NoMatch,
        "…and still does not match orders"
    );
    assert_eq!(edges, 0, "NO EDGE IS PRODUCED");
}

#[test]
fn a_resolved_url_with_the_wrong_method_still_does_not_match() {
    let project = Project::new(&[
        ("web/src/config.ts", r#"export const API_BASE = "/api/v1";"#),
        (
            "web/src/orders.ts",
            r#"import { API_BASE } from "./config";
await axios.delete(`${API_BASE}/orders`);"#,
        ),
        ("api/orders.py", ORDERS_API),
    ]);

    let (decisions, edges, _) = project.run();
    assert_eq!(
        decisions[0].0,
        MatchStatus::NoMatch,
        "DELETE reaches no declared route"
    );
    assert_eq!(edges, 0, "NO EDGE IS PRODUCED");
}

#[test]
fn resolution_does_not_let_a_catch_all_swallow_unrelated_calls() {
    // M04's hardest-won lesson must survive M05: a route with no
    // discriminating static segment accepts a whole class of paths.
    let project = Project::new(&[
        ("web/src/config.ts", r#"export const API_BASE = "/api/v1";"#),
        (
            "web/src/orders.ts",
            r#"import { API_BASE } from "./config";
await axios.post(`${API_BASE}/orders`, {});"#,
        ),
        (
            "api/catchall.py",
            r#"
from fastapi import FastAPI
app = FastAPI()

@app.post("/{path:path}")
async def catch_all(path: str):
    pass
"#,
        ),
    ]);

    let (decisions, edges, _) = project.run();
    assert_eq!(decisions[0].2, "/api/v1/orders", "the URL resolved…");
    assert_eq!(
        decisions[0].0,
        MatchStatus::Ambiguous,
        "…but a bare catch-all is still not evidence about this call"
    );
    assert_eq!(edges, 0, "NO EDGE IS PRODUCED");
}

#[test]
fn a_resolved_url_facing_two_equal_routes_is_ambiguous() {
    let project = Project::new(&[
        ("web/src/config.ts", r#"export const API_BASE = "/api";"#),
        (
            "web/src/orders.ts",
            r#"import { API_BASE } from "./config";
await axios.post(`${API_BASE}/orders/${id}`, {});"#,
        ),
        (
            "api/a.py",
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
        ),
    ]);

    let (decisions, edges, _) = project.run();
    assert_eq!(decisions[0].0, MatchStatus::Ambiguous);
    assert_eq!(edges, 0, "NO EDGE IS PRODUCED");
}

#[test]
fn an_empty_url_produces_no_edge() {
    let project = Project::new(&[
        ("web/src/orders.ts", r#"await axios.post("", {});"#),
        ("api/orders.py", ORDERS_API),
    ]);
    let (_, edges, _) = project.run();
    assert_eq!(edges, 0, "NO EDGE IS PRODUCED");
}

// ── Differential: M04 behaviour is preserved ────────────────────────

#[test]
fn literal_urls_behave_exactly_as_they_did_before_m05() {
    // A literal URL needs no evaluation, so resolve_url declines it and the
    // M04 path runs untouched.
    let project = Project::new(&[
        (
            "web/src/orders.ts",
            r#"await axios.post("/api/v1/orders", {});"#,
        ),
        ("api/orders.py", ORDERS_API),
    ]);

    let (decisions, edges, graph) = project.run();
    assert_eq!(decisions[0].0, MatchStatus::Exact);
    assert_eq!(edges, 1);
    assert_eq!(
        graph.edges().next().unwrap().confidence().to_string(),
        "0.98"
    );
}

// ── Slice 8: evidence ───────────────────────────────────────────────

#[test]
fn the_edge_evidence_explains_how_the_url_was_derived() {
    let project = Project::new(&[
        ("web/src/config.ts", r#"export const API_BASE = "/api/v1";"#),
        (
            "web/src/orders.ts",
            r#"import { API_BASE } from "./config";
await axios.get(`${API_BASE}/orders/${orderId}`);"#,
        ),
        ("api/orders.py", ORDERS_API),
    ]);
    let (_, _, graph) = project.run();

    let edge = graph.edges().next().expect("one edge");
    let evidence = edge.evidence().as_str();
    assert!(
        evidence.contains("/api/v1/orders"),
        "the derived route must appear: {evidence}"
    );
    assert!(
        !evidence.contains("orderId\": \"") && !evidence.contains("undefined"),
        "an undetermined value must never appear as a value: {evidence}"
    );
}

#[test]
fn python_clients_resolve_too() {
    let project = Project::new(&[
        (
            "worker/tasks.py",
            r#"
import requests

BASE = "/api/v1"

def sync():
    requests.post(f"{BASE}/orders")
"#,
        ),
        ("api/orders.py", ORDERS_API),
    ]);

    let (decisions, edges, _) = project.run();
    assert_eq!(decisions[0].2, "/api/v1/orders", "f-string resolved");
    assert_eq!(decisions[0].0, MatchStatus::Exact);
    assert_eq!(edges, 1);
    let _ = SourceLanguage::Python;
}
