//! Integration tests for TypeScript/TSX extraction.
//!
//! Everything here goes through the crate's public API against the fixtures in
//! `tests/fixtures/`. Assertions are about Cartograph's fact model — symbol
//! kinds, export status, template structure — never about tree-sitter node
//! names, so a grammar upgrade that preserves behaviour cannot break them.

use std::path::PathBuf;

use cartograph_parser::diagnostics::DiagnosticKind;
use cartograph_parser::model::{
    Callee, ConcatPart, ExportStatus, FileAnalysis, HttpMethodHint, ParseStatus, SourceLanguage,
    StringFact, SymbolKind, TemplatePart, UrlObservation,
};
use cartograph_parser::{Analyzer, ParserError};

fn fixtures_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
}

fn analyze(fixture: &str) -> FileAnalysis {
    Analyzer::new()
        .expect("grammars load")
        .analyze_file(&fixtures_root(), fixture)
        .expect("fixture path is valid")
}

// ── Imports ─────────────────────────────────────────────────────────

#[test]
fn extracts_every_import_form() {
    let analysis = analyze("imports.ts");
    assert_eq!(analysis.status, ParseStatus::Complete);
    assert_eq!(analysis.imports.len(), 7);

    let default = &analysis.imports[0];
    assert_eq!(default.specifier, "./api");
    assert_eq!(default.default_name.as_deref(), Some("defaultExport"));

    let namespace = &analysis.imports[1];
    assert_eq!(namespace.specifier, "node:path");
    assert_eq!(namespace.namespace_name.as_deref(), Some("path"));

    let named_import = &analysis.imports[2];
    let imported: Vec<_> = named_import
        .named
        .iter()
        .map(|n| n.imported.as_str())
        .collect();
    assert_eq!(imported, ["one", "two"]);

    let aliased = &analysis.imports[3];
    assert_eq!(aliased.named[0].imported, "original");
    assert_eq!(aliased.named[0].local.as_deref(), Some("renamed"));

    let type_only = &analysis.imports[4];
    assert!(type_only.type_only, "`import type` is marked type-only");

    let mixed = &analysis.imports[5];
    assert_eq!(mixed.default_name.as_deref(), Some("mixed"));
    assert_eq!(mixed.named[0].imported, "named");

    let side_effect = &analysis.imports[6];
    assert_eq!(side_effect.specifier, "./side-effect");
    assert!(side_effect.default_name.is_none() && side_effect.named.is_empty());
}

#[test]
fn import_specifiers_are_not_duplicated_as_string_facts() {
    let analysis = analyze("imports.ts");
    assert!(
        analysis.strings.is_empty(),
        "specifier strings belong to their Import facts: {:?}",
        analysis.strings
    );
}

#[test]
fn imports_carry_one_based_locations() {
    let analysis = analyze("imports.ts");
    // Line 1 is the comment; the first import is line 2.
    assert_eq!(analysis.imports[0].span.start_line, 2);
    assert_eq!(analysis.imports[0].span.start_column, 1);
}

// ── Symbols ─────────────────────────────────────────────────────────

fn find<'a>(analysis: &'a FileAnalysis, name: &str) -> &'a cartograph_parser::model::Symbol {
    analysis
        .symbols
        .iter()
        .find(|s| s.name == name)
        .unwrap_or_else(|| panic!("symbol `{name}` not extracted"))
}

#[test]
fn extracts_declarations_with_kind_export_and_asyncness() {
    let analysis = analyze("symbols.ts");
    assert_eq!(analysis.status, ParseStatus::Complete);

    assert_eq!(find(&analysis, "plain").kind, SymbolKind::Function);
    assert_eq!(find(&analysis, "plain").export, ExportStatus::Exported);
    assert!(!find(&analysis, "plain").is_async);

    assert!(find(&analysis, "fetchOrders").is_async);

    assert_eq!(find(&analysis, "Cart").kind, SymbolKind::Class);
    assert_eq!(find(&analysis, "Order").kind, SymbolKind::Interface);
    assert_eq!(find(&analysis, "OrderId").kind, SymbolKind::TypeAlias);
    assert_eq!(find(&analysis, "Status").kind, SymbolKind::Enum);
    assert_eq!(find(&analysis, "counter").kind, SymbolKind::Variable);
}

#[test]
fn methods_know_their_enclosing_class_and_nested_functions_their_host() {
    let analysis = analyze("symbols.ts");

    let add = find(&analysis, "add");
    assert_eq!(add.kind, SymbolKind::Method);
    assert_eq!(add.enclosing.as_deref(), Some("Cart"));

    let sync = find(&analysis, "sync");
    assert!(sync.is_async);
    assert_eq!(sync.enclosing.as_deref(), Some("Cart"));

    let nested = find(&analysis, "nested");
    assert_eq!(nested.enclosing.as_deref(), Some("inner_host"));

    assert_eq!(find(&analysis, "plain").enclosing, None);
}

#[test]
fn export_clause_and_export_default_identifier_mark_symbols_after_the_fact() {
    let analysis = analyze("symbols.ts");
    assert_eq!(
        find(&analysis, "BASE").export,
        ExportStatus::Exported,
        "`export {{ BASE }}` marks the earlier declaration"
    );
    assert_eq!(
        find(&analysis, "Hidden").export,
        ExportStatus::DefaultExported,
        "`export default Hidden` marks the earlier declaration"
    );
    assert_eq!(find(&analysis, "counter").export, ExportStatus::NotExported);
}

#[test]
fn anonymous_default_export_is_named_default() {
    let analysis = analyze("default_fn.ts");
    let symbol = find(&analysis, "default");
    assert_eq!(symbol.kind, SymbolKind::Function);
    assert_eq!(symbol.export, ExportStatus::DefaultExported);
}

// ── Calls ───────────────────────────────────────────────────────────

#[test]
fn extracts_identifier_member_chain_and_new_calls() {
    let analysis = analyze("calls.ts");
    let rendered: Vec<(String, bool)> = analysis
        .calls
        .iter()
        .map(|c| (c.callee.to_string(), c.is_new))
        .collect();

    assert!(rendered.contains(&("foo".into(), false)));
    assert!(rendered.contains(&("console.log".into(), false)));
    assert!(rendered.contains(&("a.b.c".into(), false)));
    assert!(rendered.contains(&("Foo".into(), true)), "new Foo(42)");
    assert!(rendered.contains(&("this_is_fine".into(), false)));
}

#[test]
fn call_argument_counts_are_recorded() {
    let analysis = analyze("calls.ts");
    let foo = analysis
        .calls
        .iter()
        .find(|c| c.callee.to_string() == "foo")
        .unwrap();
    assert_eq!(foo.argument_count, 2);

    let new_foo = analysis.calls.iter().find(|c| c.is_new).unwrap();
    assert_eq!(new_foo.argument_count, 1);
}

#[test]
fn unreliable_callees_are_skipped_not_guessed() {
    let analysis = analyze("calls.ts");
    for call in &analysis.calls {
        let rendered = call.callee.to_string();
        assert!(
            !rendered.contains('[') && !rendered.contains('('),
            "computed/IIFE callee leaked into facts: {rendered}"
        );
    }
    // The two skipped shapes reduce the total: 5 recorded, not 7.
    assert_eq!(analysis.calls.len(), 5);
}

#[test]
fn member_callee_preserves_the_object_chain() {
    let analysis = analyze("calls.ts");
    let chain = analysis
        .calls
        .iter()
        .find(|c| c.callee.to_string() == "a.b.c")
        .unwrap();
    match &chain.callee {
        Callee::Member { object, property } => {
            assert_eq!(object, &["a", "b"]);
            assert_eq!(property, "c");
        }
        other @ Callee::Identifier { .. } => panic!("expected member callee, got {other:?}"),
    }
}

// ── Strings ─────────────────────────────────────────────────────────

#[test]
fn extracts_literals_including_empty_and_escaped() {
    let analysis = analyze("strings.ts");
    let literals: Vec<&str> = analysis
        .strings
        .iter()
        .filter_map(|s| match s {
            StringFact::Literal { value, .. } => Some(value.as_str()),
            _ => None,
        })
        .collect();
    assert!(literals.contains(&"/api/v1"));
    assert!(literals.contains(&"hello world"));
    assert!(literals.contains(&""), "empty string is still a fact");
    assert!(
        literals.contains(&r"line\nbreak"),
        "escapes preserved as written, not interpreted: {literals:?}"
    );
}

#[test]
fn template_structure_is_preserved_not_resolved() {
    let analysis = analyze("strings.ts");
    let template = analysis
        .strings
        .iter()
        .find_map(|s| match s {
            StringFact::Template { parts, .. } if parts.len() > 1 => Some(parts),
            _ => None,
        })
        .expect("the BASE/orders template is extracted");

    // `${BASE}/orders/${orderId}` -> [Expr(BASE), Text(/orders/), Expr(orderId)]
    assert_eq!(template.len(), 3);
    match (&template[0], &template[1], &template[2]) {
        (
            TemplatePart::Expression { source: first, .. },
            TemplatePart::Text { value },
            TemplatePart::Expression { source: second, .. },
        ) => {
            assert_eq!(first, "BASE");
            assert_eq!(value, "/orders/");
            assert_eq!(second, "orderId");
        }
        other => panic!("template structure lost: {other:?}"),
    }
}

#[test]
fn substitution_free_template_is_a_single_text_part() {
    let analysis = analyze("strings.ts");
    let simple = analysis
        .strings
        .iter()
        .find_map(|s| match s {
            StringFact::Template { parts, .. } if parts.len() == 1 => Some(parts),
            _ => None,
        })
        .expect("`just text` template extracted");
    assert_eq!(
        simple[0],
        TemplatePart::Text {
            value: "just text".into()
        }
    );
}

#[test]
fn string_concatenation_is_flattened_with_operands_in_order() {
    let analysis = analyze("strings.ts");
    let concat = analysis
        .strings
        .iter()
        .find_map(|s| match s {
            StringFact::Concatenation { parts, .. } => Some(parts),
            _ => None,
        })
        .expect("\"pre\" + BASE + \"post\" extracted");

    assert_eq!(
        concat,
        &vec![
            ConcatPart::Literal {
                value: "pre".into()
            },
            ConcatPart::Expression {
                source: "BASE".into()
            },
            ConcatPart::Literal {
                value: "post".into()
            },
        ]
    );
}

#[test]
fn numeric_addition_is_not_a_concatenation_fact() {
    let analysis = analyze("strings.ts");
    let concats = analysis
        .strings
        .iter()
        .filter(|s| matches!(s, StringFact::Concatenation { .. }))
        .count();
    assert_eq!(concats, 1, "only the string-bearing chain is recorded");
}

// ── HTTP observations ───────────────────────────────────────────────

#[test]
fn observes_fetch_and_client_verbs_with_method_hints() {
    let analysis = analyze("http.ts");
    let by_callee: Vec<(String, Option<HttpMethodHint>)> = analysis
        .http_calls
        .iter()
        .map(|h| (h.callee.clone(), h.method_hint))
        .collect();

    assert!(
        by_callee.contains(&("fetch".into(), None)),
        "bare fetch: method unknown, NOT defaulted to GET"
    );
    assert!(
        by_callee.contains(&("fetch".into(), Some(HttpMethodHint::Post))),
        "options.method literal read"
    );
    assert!(by_callee.contains(&("axios.get".into(), Some(HttpMethodHint::Get))));
    assert!(by_callee.contains(&("axios.post".into(), Some(HttpMethodHint::Post))));
    assert!(by_callee.contains(&("client.get".into(), Some(HttpMethodHint::Get))));
}

#[test]
fn non_literal_method_option_stays_unknown() {
    let analysis = analyze("http.ts");
    let dynamic_method = analysis
        .http_calls
        .iter()
        .filter(|h| h.callee == "fetch")
        .filter(|h| h.method_hint.is_none())
        .count();
    // Both `fetch("/orders")` and the dynamicMethod variant: unknown, not guessed.
    assert_eq!(dynamic_method, 2);
}

#[test]
fn map_get_is_not_observed_but_pathlike_unknown_client_is() {
    let analysis = analyze("http.ts");
    assert!(
        !analysis.http_calls.iter().any(|h| h.callee == "routes.get"),
        "a Map lookup is not an HTTP call"
    );
    assert!(
        analysis.http_calls.iter().any(|h| h.callee == "widget.get"),
        "unknown object with a path-like argument is observed"
    );
    // ...but both remain in the ordinary call-site facts:
    assert!(
        analysis
            .calls
            .iter()
            .any(|c| c.callee.to_string() == "routes.get")
    );
}

#[test]
fn template_urls_stay_templates_in_observations() {
    let analysis = analyze("http.ts");
    let template_url = analysis
        .http_calls
        .iter()
        .find(|h| h.callee == "axios.post")
        .expect("axios.post observed");
    match &template_url.url {
        UrlObservation::Template { parts } => {
            assert!(
                parts.iter().any(
                    |p| matches!(p, TemplatePart::Expression { source, .. } if source == "BASE")
                ),
                "template expression structure preserved for M05"
            );
        }
        other => panic!("template URL must not be flattened or resolved: {other:?}"),
    }
}

// ── TSX ─────────────────────────────────────────────────────────────

#[test]
fn tsx_component_extracts_like_typescript() {
    let analysis = analyze("component.tsx");
    assert_eq!(analysis.language, SourceLanguage::Tsx);
    assert_eq!(analysis.status, ParseStatus::Complete);

    let component = find(&analysis, "CheckoutButton");
    assert_eq!(component.kind, SymbolKind::Function);
    assert_eq!(component.export, ExportStatus::Exported);

    let on_submit = find(&analysis, "onSubmit");
    assert!(on_submit.is_async);
    assert_eq!(on_submit.enclosing.as_deref(), Some("CheckoutButton"));

    assert!(analysis.imports.iter().any(|i| i.specifier == "react"));
    assert!(
        analysis.http_calls.iter().any(|h| h.callee == "axios.post"),
        "the HTTP observation inside the component is found"
    );
}

// ── Error tolerance ─────────────────────────────────────────────────

#[test]
fn malformed_file_still_yields_facts_from_valid_regions() {
    let analysis = analyze("malformed.ts");
    assert_eq!(analysis.status, ParseStatus::CompleteWithErrors);
    assert!(
        !analysis.diagnostics.is_empty(),
        "the broken region produces diagnostics"
    );
    assert!(
        analysis.imports.iter().any(|i| i.specifier == "./good"),
        "imports before the damage survive"
    );
    assert!(
        analysis.symbols.iter().any(|s| s.name == "fine"),
        "declarations before the damage survive"
    );
    assert!(
        analysis.strings.iter().any(|s| matches!(
            s,
            StringFact::Literal { value, .. } if value == "/still/extracted"
        )),
        "facts after the damage survive"
    );
}

#[test]
fn diagnostics_have_locations_and_carry_no_source_text() {
    let analysis = analyze("malformed.ts");
    for diagnostic in &analysis.diagnostics {
        assert!(
            !diagnostic.message.contains("broken(") && !diagnostic.message.contains("good"),
            "diagnostic message quotes source text: {}",
            diagnostic.message
        );
    }
    assert!(
        analysis.diagnostics.iter().any(|d| d.span.is_some()),
        "syntax diagnostics point at where the problem is"
    );
}

#[test]
fn non_utf8_file_fails_softly_with_a_diagnostic() {
    let analysis = analyze("invalid-utf8.ts");
    assert_eq!(analysis.status, ParseStatus::Failed);
    assert_eq!(analysis.diagnostics.len(), 1);
    assert_eq!(analysis.diagnostics[0].kind, DiagnosticKind::NotUtf8);
    assert!(analysis.symbols.is_empty());
}

#[test]
fn missing_file_fails_softly_rather_than_erroring() {
    let analysis = Analyzer::new()
        .unwrap()
        .analyze_file(&fixtures_root(), "does-not-exist.ts")
        .expect("a missing file is a diagnostic, not a ParserError");
    assert_eq!(analysis.status, ParseStatus::Failed);
    assert_eq!(analysis.diagnostics[0].kind, DiagnosticKind::Unreadable);
}

// ── API contract ────────────────────────────────────────────────────

#[test]
fn absolute_and_traversal_paths_are_refused() {
    let mut analyzer = Analyzer::new().unwrap();
    assert!(matches!(
        analyzer.analyze_source("/etc/passwd.ts", ""),
        Err(ParserError::InvalidPath(_))
    ));
    assert!(matches!(
        analyzer.analyze_source("../outside.ts", ""),
        Err(ParserError::InvalidPath(_))
    ));
}

#[test]
fn unsupported_extensions_are_refused_not_guessed() {
    let mut analyzer = Analyzer::new().unwrap();
    // `.py` became supported at M02, so the unsupported case now uses a
    // language no extractor claims. Guessing a grammar from unknown source
    // would fabricate facts.
    assert!(matches!(
        analyzer.analyze_source("main.go", "package main"),
        Err(ParserError::UnsupportedLanguage { .. })
    ));
    assert!(matches!(
        analyzer.analyze_source("README.md", "# hello"),
        Err(ParserError::UnsupportedLanguage { .. })
    ));
}

#[test]
fn analysis_round_trips_through_json() {
    let analysis = analyze("component.tsx");
    let json = serde_json::to_string(&analysis).unwrap();
    let back: FileAnalysis = serde_json::from_str(&json).unwrap();
    assert_eq!(analysis, back);
}

// ── M07 regression: a server route declaration is not a client call ──

/// `PostHog`, `services/agent-proxy/src/hono/public-routes.ts:30` and two more.
///
/// `Express` and `Hono` declare routes with the same syntax a client uses to call
/// one. At M07 three such declarations in `PostHog`'s Node services produced
/// `HttpCall` edges to a Python `/_health` endpoint in an unrelated service.
#[test]
fn an_express_or_hono_route_declaration_is_not_an_http_call() {
    let mut analyzer = Analyzer::new().expect("grammars load");
    let analysis = analyzer
        .analyze_source(
            "services/proxy/src/routes.ts",
            r"
export function registerPublicRoutes(app) {
  app.get('/_health', (c) => c.json({ status: 'ok' }));
  app.post('/_reload', function (req, res) { res.end(); });
}
",
        )
        .expect("fixture parses");
    assert!(
        analysis.http_calls.is_empty(),
        "route declarations were recorded as client calls: {:?}",
        analysis.http_calls
    );
}

#[test]
fn a_known_client_passing_a_callback_is_still_a_request() {
    // Node's own `http.get(url, callback)` passes a function and is a real
    // request. The rule must not confuse it with a route registration.
    let mut analyzer = Analyzer::new().expect("grammars load");
    let analysis = analyzer
        .analyze_source(
            "src/fetchData.ts",
            r"
http.get('/api/orders', (res) => { res.resume(); });
",
        )
        .expect("fixture parses");
    assert_eq!(analysis.http_calls.len(), 1, "{:?}", analysis.http_calls);
}

#[test]
fn an_ordinary_client_call_with_options_is_unaffected() {
    let mut analyzer = Analyzer::new().expect("grammars load");
    let analysis = analyzer
        .analyze_source(
            "src/api.ts",
            r"
apiClient.post('/api/orders', { body: payload });
fetch('/api/items', { method: 'PUT' });
",
        )
        .expect("fixture parses");
    assert_eq!(analysis.http_calls.len(), 2, "{:?}", analysis.http_calls);
}

#[test]
fn a_route_registered_with_a_named_handler_is_still_indistinguishable() {
    // The boundary of the rule above, asserted so it stays visible.
    //
    // `router.get("/items", handleItems)` and `client.get("/items", config)`
    // are the same syntax; only resolving the identifier separates them, and
    // no milestone has claimed interprocedural resolution. This is recorded
    // as an observation and may still become an edge, so it is a known false
    // positive rather than a solved case.
    let mut analyzer = Analyzer::new().expect("grammars load");
    let analysis = analyzer
        .analyze_source(
            "services/proxy/src/routes.ts",
            r"
router.get('/items', handleItems);
",
        )
        .expect("fixture parses");
    assert_eq!(
        analysis.http_calls.len(),
        1,
        "if this becomes 0, the named-handler case was solved and the \
         limitation note in docs/benchmarks/m07-report.md should be removed"
    );
}

// ── M07 remediation: requests described by a configuration object ────

/// Airflow, `ui/openapi-gen/requests/services.gen.ts:3667`.
///
/// The shape every generated client in the M07 corpus uses, and the reason
/// only twelve of 1,458 `HttpCall` edges had a TypeScript client.
#[test]
fn a_request_configuration_object_is_an_http_observation() {
    let mut analyzer = Analyzer::new().expect("grammars load");
    let analysis = analyzer
        .analyze_source(
            "ui/openapi-gen/requests/services.gen.ts",
            r"
export class PoolService {
    public static postPool(data) {
        return __request(OpenAPI, {
            method: 'POST',
            url: '/api/v2/pools',
            body: data.requestBody,
        });
    }
}
",
        )
        .expect("fixture parses");

    assert_eq!(analysis.http_calls.len(), 1, "{:?}", analysis.http_calls);
    let call = &analysis.http_calls[0];
    assert_eq!(call.method_hint, Some(HttpMethodHint::Post));
    assert_eq!(
        call.url,
        UrlObservation::Literal {
            value: "/api/v2/pools".to_owned()
        }
    );
}

/// full-stack-fastapi, `frontend/src/client/sdk.gen.ts`.
///
/// A parenthesised callee the shape rules cannot name, whose URL is still
/// stated literally in the options object.
#[test]
fn a_configuration_object_is_read_whatever_the_callee_looks_like() {
    let mut analyzer = Analyzer::new().expect("grammars load");
    let analysis = analyzer
        .analyze_source(
            "src/client/sdk.gen.ts",
            r"
export class ItemsService {
    public static readItems(options) {
        return (options?.client ?? client).get({
            url: '/api/v1/items/',
            responseType: 'json',
        });
    }
}
",
        )
        .expect("fixture parses");
    assert_eq!(analysis.http_calls.len(), 1, "{:?}", analysis.http_calls);
    assert_eq!(
        analysis.http_calls[0].url,
        UrlObservation::Literal {
            value: "/api/v1/items/".to_owned()
        }
    );
    // The object states no `method`, but the callee spells one. `.get` is a
    // GET — written down, not inferred.
    assert_eq!(
        analysis.http_calls[0].method_hint,
        Some(HttpMethodHint::Get)
    );
}

/// Superset, `superset-frontend/src/**`.
#[test]
fn an_endpoint_property_is_read_like_a_url() {
    let mut analyzer = Analyzer::new().expect("grammars load");
    let analysis = analyzer
        .analyze_source(
            "src/actions.ts",
            r"
export function fetchCharts() {
  return SupersetClient.get({ endpoint: '/api/v1/chart/' });
}
",
        )
        .expect("fixture parses");
    assert_eq!(analysis.http_calls.len(), 1, "{:?}", analysis.http_calls);
    assert_eq!(
        analysis.http_calls[0].url,
        UrlObservation::Literal {
            value: "/api/v1/chart/".to_owned()
        }
    );
}

#[test]
fn a_routing_table_is_never_read_as_an_http_call() {
    // The false-positive class this rule is shaped around. React Router, Vue
    // Router and TanStack all describe screens with `{ path: "/orders" }`.
    // Accepting `path` as a URL key would turn a frontend's own route table
    // into outbound requests — more false positives than the rule finds true
    // ones.
    let mut analyzer = Analyzer::new().expect("grammars load");
    let analysis = analyzer
        .analyze_source(
            "src/routes.tsx",
            r"
export const routes = [
  { path: '/orders', element: Orders },
  { path: '/orders/:id', element: OrderDetail },
];
createBrowserRouter([{ path: '/settings', element: Settings }]);
",
        )
        .expect("fixture parses");
    assert!(
        analysis.http_calls.is_empty(),
        "a routing table became HTTP calls: {:?}",
        analysis.http_calls
    );
}

#[test]
fn a_configuration_object_without_a_path_like_url_is_refused() {
    let mut analyzer = Analyzer::new().expect("grammars load");
    let analysis = analyzer
        .analyze_source(
            "src/config.ts",
            r"
configure({ url: baseUrl, method: 'POST' });
track({ url: 'analytics-event', name: 'click' });
",
        )
        .expect("fixture parses");
    assert!(
        analysis.http_calls.is_empty(),
        "a non-literal or non-path url became an observation: {:?}",
        analysis.http_calls
    );
}

#[test]
fn a_templated_configuration_url_keeps_its_unknown_parts() {
    let mut analyzer = Analyzer::new().expect("grammars load");
    let analysis = analyzer
        .analyze_source(
            "src/client.ts",
            "export function get(id) { return request({ method: 'GET', url: `/api/v2/pools/${id}` }); }\n",
        )
        .expect("fixture parses");
    assert_eq!(analysis.http_calls.len(), 1, "{:?}", analysis.http_calls);
    assert!(
        matches!(analysis.http_calls[0].url, UrlObservation::Template { .. }),
        "an interpolated URL must stay a template: {:?}",
        analysis.http_calls[0].url
    );
}
