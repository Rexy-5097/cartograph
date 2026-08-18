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
    assert!(matches!(
        analyzer.analyze_source("script.py", "x = 1"),
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
