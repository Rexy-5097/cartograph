//! Integration tests for Python extraction.
//!
//! Exercised through the crate's public API against `tests/fixtures/python/`.
//! Assertions are about Cartograph's language-neutral fact model — never about
//! tree-sitter node names — so a grammar upgrade that preserves behaviour
//! cannot break them.
//!
//! The recurring theme is the observation/claim boundary: routes are
//! declarations that were *seen*, HTTP calls are shapes that were *seen*,
//! f-strings stay symbolic, and nothing is resolved.

use std::path::PathBuf;

use cartograph_parser::Analyzer;
use cartograph_parser::diagnostics::DiagnosticKind;
use cartograph_parser::model::{
    Callee, ConcatPart, ExportStatus, FileAnalysis, HttpMethodHint, ParseStatus,
    RouteDeclarationStyle, RouteObservation, SourceLanguage, StringFact, Symbol, SymbolKind,
    TemplatePart, UrlObservation,
};

fn fixtures_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/python")
}

fn analyze(fixture: &str) -> FileAnalysis {
    Analyzer::new()
        .expect("grammars load")
        .analyze_file(&fixtures_root(), fixture)
        .expect("fixture path is valid")
}

fn find<'a>(analysis: &'a FileAnalysis, name: &str) -> &'a Symbol {
    analysis
        .symbols
        .iter()
        .find(|s| s.name == name)
        .unwrap_or_else(|| panic!("symbol `{name}` not extracted"))
}

fn literal_path(route: &RouteObservation) -> &str {
    match &route.path {
        UrlObservation::Literal { value } => value,
        other => panic!("expected a literal path, got {other:?}"),
    }
}

fn route_for<'a>(analysis: &'a FileAnalysis, handler: &str) -> &'a RouteObservation {
    analysis
        .routes
        .iter()
        .find(|r| r.handler.as_deref() == Some(handler))
        .unwrap_or_else(|| panic!("no route observation for handler `{handler}`"))
}

// ── Language detection ──────────────────────────────────────────────

#[test]
fn python_files_are_recognised_by_extension() {
    assert_eq!(
        SourceLanguage::from_path("api/main.py"),
        Some(SourceLanguage::Python)
    );
    assert_eq!(
        SourceLanguage::from_path("api/types.pyi"),
        Some(SourceLanguage::Python)
    );
    assert_eq!(
        SourceLanguage::from_path("API/MAIN.PY"),
        Some(SourceLanguage::Python)
    );
    assert_eq!(SourceLanguage::Python.to_string(), "python");
}

// ── Imports ─────────────────────────────────────────────────────────

#[test]
fn extracts_every_import_form() {
    let analysis = analyze("imports.py");
    assert_eq!(analysis.status, ParseStatus::Complete);

    let by_specifier = |s: &str| {
        analysis
            .imports
            .iter()
            .find(|i| i.specifier == s)
            .unwrap_or_else(|| panic!("no import of `{s}`"))
    };

    assert!(by_specifier("os").named.is_empty(), "plain `import os`");
    assert_eq!(by_specifier("numpy").namespace_name.as_deref(), Some("np"));
    assert_eq!(
        by_specifier("a.b.c").specifier,
        "a.b.c",
        "dotted module kept whole"
    );
    assert_eq!(by_specifier("collections").named[0].imported, "OrderedDict");

    let aliased = by_specifier("typing");
    assert_eq!(aliased.named[0].imported, "List");
    assert_eq!(aliased.named[0].local.as_deref(), Some("L"));
}

#[test]
fn relative_imports_keep_their_leading_dots() {
    let analysis = analyze("imports.py");
    let specifiers: Vec<&str> = analysis
        .imports
        .iter()
        .map(|i| i.specifier.as_str())
        .collect();

    assert!(
        specifiers.contains(&".sibling"),
        "single-dot relative: {specifiers:?}"
    );
    assert!(
        specifiers.contains(&"..parent.pkg"),
        "double-dot relative: {specifiers:?}"
    );
    assert!(
        specifiers.contains(&"."),
        "`from . import x` keeps the bare dot"
    );
}

#[test]
fn wildcard_import_binds_a_namespace_rather_than_inventing_names() {
    let analysis = analyze("imports.py");
    let wildcard = analysis
        .imports
        .iter()
        .find(|i| i.specifier == "legacy")
        .expect("wildcard import extracted");

    assert_eq!(wildcard.namespace_name.as_deref(), Some("*"));
    assert!(
        wildcard.named.is_empty(),
        "`import *` must not enumerate names the source never listed"
    );
}

#[test]
fn import_specifiers_are_not_duplicated_as_string_facts() {
    let analysis = analyze("imports.py");
    assert!(
        analysis.strings.is_empty(),
        "specifier strings belong to their Import facts: {:?}",
        analysis.strings
    );
}

// ── Symbols ─────────────────────────────────────────────────────────

#[test]
fn extracts_functions_classes_methods_and_module_variables() {
    let analysis = analyze("symbols.py");

    assert_eq!(find(&analysis, "plain").kind, SymbolKind::Function);
    assert_eq!(find(&analysis, "Cart").kind, SymbolKind::Class);
    assert_eq!(find(&analysis, "add").kind, SymbolKind::Method);
    assert_eq!(find(&analysis, "BASE").kind, SymbolKind::Variable);
}

#[test]
fn async_definitions_are_marked() {
    let analysis = analyze("symbols.py");
    assert!(find(&analysis, "fetch_orders").is_async);
    assert!(find(&analysis, "sync").is_async, "async method");
    assert!(!find(&analysis, "plain").is_async);
}

#[test]
fn enclosing_symbols_are_tracked_for_methods_and_nested_functions() {
    let analysis = analyze("symbols.py");
    assert_eq!(find(&analysis, "add").enclosing.as_deref(), Some("Cart"));
    assert_eq!(
        find(&analysis, "nested").enclosing.as_deref(),
        Some("outer")
    );
    assert_eq!(find(&analysis, "plain").enclosing, None);
}

#[test]
fn python_symbols_are_not_given_invented_export_semantics() {
    let analysis = analyze("symbols.py");
    // Python has no export statement. Claiming "NotExported" would assert
    // something the source never said.
    assert_eq!(find(&analysis, "plain").export, ExportStatus::NotApplicable);
    assert_eq!(
        find(&analysis, "_private_const").export,
        ExportStatus::NotApplicable,
        "a leading underscore is a naming convention, not a declaration"
    );
}

#[test]
fn dunder_all_is_honoured_because_it_is_a_real_export_list() {
    let analysis = analyze("symbols.py");
    assert_eq!(
        find(&analysis, "fetch_orders").export,
        ExportStatus::Exported
    );
    assert_eq!(find(&analysis, "Cart").export, ExportStatus::Exported);
    // Not listed in __all__: still importable, so still NotApplicable.
    assert_eq!(find(&analysis, "plain").export, ExportStatus::NotApplicable);
}

#[test]
fn decorators_are_recorded_as_observations() {
    let analysis = analyze("symbols.py");
    assert_eq!(find(&analysis, "helper").decorators, vec!["staticmethod"]);
    assert!(find(&analysis, "add").decorators.is_empty());
}

// ── Calls ───────────────────────────────────────────────────────────

#[test]
fn extracts_identifier_and_attribute_chain_calls() {
    let analysis = analyze("calls.py");
    let rendered: Vec<String> = analysis
        .calls
        .iter()
        .map(|c| c.callee.to_string())
        .collect();

    assert!(rendered.contains(&"foo".to_string()));
    assert!(rendered.contains(&"obj.foo".to_string()));
    assert!(rendered.contains(&"pkg.mod.fn".to_string()));
    assert!(
        rendered.contains(&"ClassName".to_string()),
        "instantiation is an ordinary call in Python"
    );
}

#[test]
fn instantiation_is_not_marked_as_new() {
    let analysis = analyze("calls.py");
    assert!(
        analysis.calls.iter().all(|c| !c.is_new),
        "Python has no `new`; claiming otherwise needs to know what a name refers to"
    );
}

#[test]
fn argument_counts_are_recorded() {
    let analysis = analyze("calls.py");
    let foo = analysis
        .calls
        .iter()
        .find(|c| c.callee.to_string() == "foo")
        .unwrap();
    assert_eq!(foo.argument_count, 2);
}

#[test]
fn unreliable_callees_are_skipped_not_guessed() {
    let analysis = analyze("calls.py");
    for call in &analysis.calls {
        let rendered = call.callee.to_string();
        assert!(
            !rendered.contains('[') && !rendered.contains('('),
            "subscript/call-of-call callee leaked into facts: {rendered}"
        );
    }
}

#[test]
fn attribute_callee_preserves_the_object_chain() {
    let analysis = analyze("calls.py");
    let chain = analysis
        .calls
        .iter()
        .find(|c| c.callee.to_string() == "pkg.mod.fn")
        .unwrap();
    match &chain.callee {
        Callee::Member { object, property } => {
            assert_eq!(object, &["pkg", "mod"]);
            assert_eq!(property, "fn");
        }
        other @ Callee::Identifier { .. } => panic!("expected attribute callee, got {other:?}"),
    }
}

// ── Strings ─────────────────────────────────────────────────────────

#[test]
fn extracts_literals_including_empty_escaped_and_triple_quoted() {
    let analysis = analyze("strings.py");
    let literals: Vec<&str> = analysis
        .strings
        .iter()
        .filter_map(|s| match s {
            StringFact::Literal { value, .. } => Some(value.as_str()),
            _ => None,
        })
        .collect();

    assert!(literals.contains(&"/api/v1"));
    assert!(literals.contains(&""), "empty string is still a fact");
    assert!(
        literals.contains(&r"line\nbreak"),
        "escapes preserved as written, not interpreted: {literals:?}"
    );
    assert!(
        literals
            .iter()
            .any(|l| l.contains('\n') && l.contains("multi")),
        "triple-quoted content extracted: {literals:?}"
    );
}

#[test]
fn f_string_structure_is_preserved_and_never_evaluated() {
    let analysis = analyze("strings.py");
    let template = analysis
        .strings
        .iter()
        .find_map(|s| match s {
            StringFact::Template { parts, span } if span.start_line == 8 => Some(parts),
            _ => None,
        })
        .expect("f\"/orders/{order_id}\" extracted");

    assert_eq!(template.len(), 2);
    assert_eq!(
        template[0],
        TemplatePart::Text {
            value: "/orders/".into()
        }
    );
    match &template[1] {
        TemplatePart::Expression { source, span } => {
            assert_eq!(
                source, "order_id",
                "the expression stays symbolic; M02 must not resolve `order_id`"
            );
            assert_eq!(
                span.start_line, 8,
                "the substitution carries its own location"
            );
        }
        other @ TemplatePart::Text { .. } => panic!("expected a substitution, got {other:?}"),
    }
}

#[test]
fn f_string_with_leading_and_interior_expressions_keeps_order() {
    let analysis = analyze("strings.py");
    let parts = analysis
        .strings
        .iter()
        .find_map(|s| match s {
            StringFact::Template { parts, span } if span.start_line == 9 => Some(parts),
            _ => None,
        })
        .expect("f\"{BASE}/orders/{order_id}/items\" extracted");

    let shape: Vec<String> = parts
        .iter()
        .map(|p| match p {
            TemplatePart::Text { value } => format!("text:{value}"),
            TemplatePart::Expression { source, .. } => format!("expr:{source}"),
        })
        .collect();
    assert_eq!(
        shape,
        ["expr:BASE", "text:/orders/", "expr:order_id", "text:/items"]
    );
}

#[test]
fn f_string_without_substitutions_is_a_plain_literal() {
    let analysis = analyze("strings.py");
    assert!(
        analysis.strings.iter().any(|s| matches!(
            s,
            StringFact::Literal { value, .. } if value == "just text"
        )),
        "an f-string with no substitution carries no structure worth keeping"
    );
}

#[test]
fn string_concatenation_is_flattened_in_order() {
    let analysis = analyze("strings.py");
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
    let analysis = analyze("strings.py");
    assert_eq!(
        analysis
            .strings
            .iter()
            .filter(|s| matches!(s, StringFact::Concatenation { .. }))
            .count(),
        1
    );
}

// ── FastAPI ─────────────────────────────────────────────────────────

#[test]
fn fastapi_verb_decorators_become_route_observations() {
    let analysis = analyze("fastapi_routes.py");

    for (handler, method, path) in [
        ("list_orders", HttpMethodHint::Get, "/orders"),
        ("create_order", HttpMethodHint::Post, "/orders"),
        ("replace_order", HttpMethodHint::Put, "/orders/{id}"),
        ("remove_order", HttpMethodHint::Delete, "/orders/{id}"),
    ] {
        let route = route_for(&analysis, handler);
        assert_eq!(route.style, RouteDeclarationStyle::VerbDecorator);
        assert_eq!(route.methods, vec![method]);
        assert_eq!(literal_path(route), path);
    }
}

#[test]
fn parameterised_paths_are_preserved_unnormalised() {
    let analysis = analyze("fastapi_routes.py");
    assert_eq!(
        literal_path(route_for(&analysis, "replace_order")),
        "/orders/{id}",
        "canonicalising `{{id}}` is M03's job; M02 records what was written"
    );
}

#[test]
fn apirouter_declarations_are_observed_too() {
    let analysis = analyze("fastapi_routes.py");
    let route = route_for(&analysis, "add_item");
    assert_eq!(route.style, RouteDeclarationStyle::VerbDecorator);
    assert_eq!(route.methods, vec![HttpMethodHint::Post]);
    assert_eq!(literal_path(route), "/router/items");
}

#[test]
fn non_route_decorators_do_not_produce_routes() {
    let analysis = analyze("fastapi_routes.py");
    assert!(
        !analysis
            .routes
            .iter()
            .any(|r| r.handler.as_deref() == Some("add_timing")),
        "`@app.middleware(\"http\")` declares no route"
    );
    assert!(
        !analysis
            .routes
            .iter()
            .any(|r| r.handler.as_deref() == Some("not_a_route")),
        "`@staticmethod` declares no route"
    );
}

#[test]
fn route_decorators_never_become_http_client_observations() {
    // The decisive false-positive case: `@app.get("/orders")` is syntactically
    // a call to `app.get` with a path-like argument. Treating it as an HTTP
    // client call would give every FastAPI app a phantom outbound request.
    let analysis = analyze("fastapi_routes.py");
    assert!(
        analysis.http_calls.is_empty(),
        "route declarations leaked into HTTP observations: {:?}",
        analysis.http_calls
    );
}

// ── Flask ───────────────────────────────────────────────────────────

#[test]
fn flask_route_declares_its_methods() {
    let analysis = analyze("flask_routes.py");
    let route = route_for(&analysis, "create_order");
    assert_eq!(route.style, RouteDeclarationStyle::RouteDecorator);
    assert_eq!(route.methods, vec![HttpMethodHint::Post]);
    assert_eq!(literal_path(route), "/orders");
}

#[test]
fn flask_methods_list_yields_every_declared_method() {
    let analysis = analyze("flask_routes.py");
    assert_eq!(
        route_for(&analysis, "orders").methods,
        vec![HttpMethodHint::Get, HttpMethodHint::Post],
        "a route may declare several methods, which is why this is a list"
    );
}

#[test]
fn absent_flask_methods_stay_unknown_rather_than_defaulting_to_get() {
    let analysis = analyze("flask_routes.py");
    assert!(
        route_for(&analysis, "health").methods.is_empty(),
        "Flask implies GET through framework semantics; inferring it is M03+, not observation"
    );
}

#[test]
fn non_literal_flask_methods_stay_unknown() {
    let analysis = analyze("flask_routes.py");
    assert!(
        route_for(&analysis, "dynamic_methods").methods.is_empty(),
        "`methods=ALLOWED` names a variable; its value is not known here"
    );
}

#[test]
fn blueprint_routes_are_observed() {
    let analysis = analyze("flask_routes.py");
    let route = route_for(&analysis, "blueprint_items");
    assert_eq!(route.style, RouteDeclarationStyle::RouteDecorator);
    assert_eq!(route.methods, vec![HttpMethodHint::Delete]);
}

// ── Django ──────────────────────────────────────────────────────────

#[test]
fn django_path_entries_become_route_observations() {
    let analysis = analyze("django_urls.py");
    let route = route_for(&analysis, "views.create_order");
    assert_eq!(route.style, RouteDeclarationStyle::UrlConfEntry);
    assert_eq!(literal_path(route), "orders/");
}

#[test]
fn django_declares_no_http_method_so_none_is_recorded() {
    let analysis = analyze("django_urls.py");
    for route in analysis
        .routes
        .iter()
        .filter(|r| r.style == RouteDeclarationStyle::UrlConfEntry)
    {
        assert!(
            route.methods.is_empty(),
            "a Django URL conf entry states no method; the view decides"
        );
    }
}

#[test]
fn django_converter_syntax_and_regexes_are_preserved_verbatim() {
    let analysis = analyze("django_urls.py");
    assert_eq!(
        literal_path(route_for(&analysis, "views.order_detail")),
        "orders/<int:pk>/",
        "`<int:pk>` is a dialect M03 canonicalises; M02 keeps it as written"
    );
    let regex_route = analysis
        .routes
        .iter()
        .find(|r| r.handler.as_deref() == Some("views.archive"))
        .expect("re_path observed");
    assert_eq!(literal_path(regex_route), "^archive/(?P<year>[0-9]{4})/$");
}

#[test]
fn django_class_based_view_handler_is_captured() {
    let analysis = analyze("django_urls.py");
    assert_eq!(
        route_for(&analysis, "LegacyView.as_view").style,
        RouteDeclarationStyle::UrlConfEntry
    );
}

#[test]
fn django_path_with_a_non_literal_pattern_is_not_recorded() {
    let analysis = analyze("django_urls.py");
    assert!(
        !analysis
            .routes
            .iter()
            .any(|r| r.handler.as_deref() == Some("views.dynamic")),
        "`path(prefix, ...)` gives no observable pattern; recording it would be a guess"
    );
}

// ── HTTP observations ───────────────────────────────────────────────

#[test]
fn recognises_requests_and_httpx_verbs() {
    let analysis = analyze("http_calls.py");
    let seen: Vec<(String, Option<HttpMethodHint>)> = analysis
        .http_calls
        .iter()
        .map(|h| (h.callee.clone(), h.method_hint))
        .collect();

    assert!(seen.contains(&("requests.get".into(), Some(HttpMethodHint::Get))));
    assert!(seen.contains(&("requests.post".into(), Some(HttpMethodHint::Post))));
    assert!(seen.contains(&("requests.put".into(), Some(HttpMethodHint::Put))));
    assert!(seen.contains(&("requests.delete".into(), Some(HttpMethodHint::Delete))));
    assert!(seen.contains(&("httpx.get".into(), Some(HttpMethodHint::Get))));
}

#[test]
fn generic_client_objects_are_recognised() {
    let analysis = analyze("http_calls.py");
    let callees: Vec<&str> = analysis
        .http_calls
        .iter()
        .map(|h| h.callee.as_str())
        .collect();
    assert!(callees.contains(&"client.get"));
    assert!(callees.contains(&"session.post"));
    assert!(
        callees.contains(&"api.post"),
        "unknown receiver with a path-like argument is still observed"
    );
}

#[test]
fn dictionary_and_environment_lookups_are_not_http() {
    let analysis = analyze("http_calls.py");
    for callee in ["config.get", "os.environ.get", "cache.get"] {
        assert!(
            !analysis.http_calls.iter().any(|h| h.callee == callee),
            "`{callee}` is not an HTTP call"
        );
    }
    // ...but they remain ordinary call-site facts.
    assert!(
        analysis
            .calls
            .iter()
            .any(|c| c.callee.to_string() == "config.get")
    );
}

#[test]
fn dynamic_urls_stay_dynamic_and_f_strings_stay_templates() {
    let analysis = analyze("http_calls.py");

    let dynamic = analysis
        .http_calls
        .iter()
        .find(|h| matches!(h.url, UrlObservation::Dynamic { .. }))
        .expect("requests.post(url) observed");
    match &dynamic.url {
        UrlObservation::Dynamic { source } => assert_eq!(source, "url"),
        other => panic!("expected dynamic URL, got {other:?}"),
    }

    let templated = analysis
        .http_calls
        .iter()
        .find(|h| matches!(h.url, UrlObservation::Template { .. }))
        .expect("httpx.post(f\"...\") observed");
    match &templated.url {
        UrlObservation::Template { parts } => assert!(
            parts
                .iter()
                .any(|p| matches!(p, TemplatePart::Expression { source, .. } if source == "BASE")),
            "template structure preserved for later analysis"
        ),
        other => panic!("expected template URL, got {other:?}"),
    }
}

// ── Error tolerance ─────────────────────────────────────────────────

#[test]
fn malformed_python_still_yields_facts_from_valid_regions() {
    let analysis = analyze("malformed.py");
    assert_eq!(analysis.status, ParseStatus::CompleteWithErrors);
    assert!(!analysis.diagnostics.is_empty());

    assert!(
        analysis.imports.iter().any(|i| i.specifier == "good"),
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
fn diagnostics_are_located_and_carry_no_source_text() {
    let analysis = analyze("malformed.py");
    for diagnostic in &analysis.diagnostics {
        assert!(
            !diagnostic.message.contains("broken") && !diagnostic.message.contains("good"),
            "diagnostic quotes source text: {}",
            diagnostic.message
        );
    }
    assert!(analysis.diagnostics.iter().any(|d| d.span.is_some()));
    assert!(
        analysis.diagnostics.len() <= 26,
        "diagnostics are capped per file"
    );
}

#[test]
fn non_utf8_python_fails_softly() {
    let analysis = analyze("invalid_utf8.py");
    assert_eq!(analysis.status, ParseStatus::Failed);
    assert_eq!(analysis.diagnostics[0].kind, DiagnosticKind::NotUtf8);
    assert!(analysis.symbols.is_empty() && analysis.routes.is_empty());
}

#[test]
fn missing_python_file_fails_softly() {
    let analysis = Analyzer::new()
        .unwrap()
        .analyze_file(&fixtures_root(), "does-not-exist.py")
        .expect("a missing file is a diagnostic, not a ParserError");
    assert_eq!(analysis.status, ParseStatus::Failed);
    assert_eq!(analysis.diagnostics[0].kind, DiagnosticKind::Unreadable);
}

// ── Model round-trip ────────────────────────────────────────────────

#[test]
fn python_analysis_round_trips_through_json() {
    for fixture in [
        "fastapi_routes.py",
        "flask_routes.py",
        "django_urls.py",
        "strings.py",
    ] {
        let analysis = analyze(fixture);
        let json = serde_json::to_string(&analysis).unwrap();
        let back: FileAnalysis = serde_json::from_str(&json).unwrap();
        assert_eq!(analysis, back, "round trip lost information for {fixture}");
    }
}

#[test]
fn declaration_styles_serialise_as_the_syntax_they_name() {
    assert_eq!(
        serde_json::to_value(RouteDeclarationStyle::VerbDecorator).unwrap(),
        serde_json::json!("verb-decorator")
    );
    assert_eq!(
        serde_json::to_value(RouteDeclarationStyle::UrlConfEntry).unwrap(),
        serde_json::json!("url-conf-entry")
    );
}

// ── M07 regression: a blueprint may be bound to any name ─────────────

fn analyze_source(source: &str) -> FileAnalysis {
    let mut analyzer = Analyzer::new().expect("grammars load");
    analyzer
        .analyze_source("app/views.py", source)
        .expect("fixture parses")
}

/// Superset, `superset/views/health.py:26-36`.
///
/// Four Flask routes registered on a blueprint named `health_blueprint`. The
/// receiver-name list recognised `bp`, `app`, `router`, `api`, `blueprint` and
/// anything ending `_router`/`_app`, so all four were invisible at M07 — the
/// only route misses left in the corpus.
#[test]
fn a_blueprint_bound_to_any_name_still_declares_routes() {
    let analysis = analyze_source(
        r#"
health_blueprint = Blueprint("health", __name__)

@health_blueprint.route("/health")
def health():
    return "OK"

@health_blueprint.route("/ping")
def ping():
    return "OK"
"#,
    );
    let paths: Vec<String> = analysis
        .routes
        .iter()
        .map(|r| match &r.path {
            UrlObservation::Literal { value } => value.clone(),
            other => format!("{other:?}"),
        })
        .collect();
    assert_eq!(paths, vec!["/health", "/ping"], "{:?}", analysis.routes);
    assert_eq!(analysis.routes[0].handler.as_deref(), Some("health"));
}

#[test]
fn an_unnamed_receiver_still_needs_a_path_argument() {
    // The evidence that makes an unknown receiver acceptable is the literal
    // path. `@mock.patch("app.core.settings")` is a decorator named for an
    // HTTP verb whose argument is a dotted import path, and it declares no
    // route. Without this the corpus would gain thousands of phantom routes:
    // PostHog alone contains 6,844 `@patch(...)` decorators.
    let analysis = analyze_source(
        r#"
@mock.patch("app.core.config.settings")
def test_something(mocked):
    pass

@some_object.get(timeout)
def not_a_route():
    pass
"#,
    );
    assert!(
        analysis.routes.is_empty(),
        "a non-path argument must not declare a route: {:?}",
        analysis.routes
    );
}

#[test]
fn a_known_registrar_still_works_without_a_literal_path() {
    // A recognised receiver is evidence in its own right, so a router whose
    // path is built elsewhere is still observed — the previous behaviour is
    // widened, never narrowed.
    let analysis = analyze_source(
        r#"
@router.get(SOME_PATH)
def listing():
    pass
"#,
    );
    assert_eq!(analysis.routes.len(), 1, "{:?}", analysis.routes);
}
