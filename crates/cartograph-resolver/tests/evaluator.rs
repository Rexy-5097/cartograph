//! Expression evaluation: slices 2–6.
//!
//! Golden tests map an expression to its expected symbolic representation. The
//! refusal cases are the important half — this evaluator's job is to resolve
//! what the source determines and to decline everything else.

use cartograph_parser::Analyzer;
use cartograph_parser::model::FileAnalysis;
use cartograph_resolver::{Scope, SymbolicValue, evaluate_expression, evaluate_string_fact};

fn analyze(path: &str, src: &str) -> FileAnalysis {
    Analyzer::new()
        .expect("grammars load")
        .analyze_source(path, src)
        .expect("fixture parses")
}

/// Evaluates the URL of the first HTTP call in a source file.
fn url_of(path: &str, src: &str) -> SymbolicValue {
    let a = analyze(path, src);
    let scope = Scope::from_file(&a.symbols, &a.strings);
    let call = a.http_calls.first().expect("an HTTP call was extracted");
    // Re-express the call's URL as a string fact so the evaluator sees the
    // same structure the parser recorded.
    let fact = a
        .strings
        .iter()
        .find(|s| s.span() == call.span || s.span().start_line == call.span.start_line)
        .expect("the URL appears among the string facts");
    evaluate_string_fact(fact, &scope)
}

fn scope_of(path: &str, src: &str) -> Scope {
    let a = analyze(path, src);
    Scope::from_file(&a.symbols, &a.strings)
}

// ── Slice 1/2: literal and template propagation ─────────────────────

#[test]
fn a_module_constant_binds_to_its_literal() {
    let scope = scope_of(
        "web/src/config.ts",
        r#"const BASE = "/api";
const PATH = "/orders";"#,
    );
    assert_eq!(
        evaluate_expression("BASE", &scope).as_literal().as_deref(),
        Some("/api")
    );
    assert_eq!(
        evaluate_expression("PATH", &scope).as_literal().as_deref(),
        Some("/orders")
    );
}

#[test]
fn a_template_resolves_when_every_substitution_is_known() {
    let value = url_of(
        "web/src/api.ts",
        r#"const BASE = "/api";
await fetch(`${BASE}/orders`);"#,
    );
    assert_eq!(
        value.as_literal().as_deref(),
        Some("/api/orders"),
        "got {value:?}"
    );
}

#[test]
fn a_template_with_an_unknown_substitution_stays_partly_unresolved() {
    let value = url_of("web/src/api.ts", r#"await fetch(`/orders/${id}`);"#);
    assert_eq!(value.as_literal(), None, "`id` is not statically known");
    assert_eq!(value.to_string(), "/orders/{unknown}");
}

#[test]
fn a_known_prefix_and_an_unknown_suffix_both_survive() {
    // The canonical example from the specification.
    let value = url_of(
        "web/src/api.ts",
        r#"const BASE = "/api/v1";
await fetch(`${BASE}/orders/${orderId}`);"#,
    );
    assert_eq!(value.to_string(), "/api/v1/orders/{unknown}");
    assert_eq!(value.as_literal(), None, "the whole URL is not determined");
}

// ── Slice 4: concatenation ──────────────────────────────────────────

#[test]
fn concatenation_of_literals_resolves() {
    let value = url_of(
        "web/src/api.ts",
        r#"const BASE = "/api";
await fetch(BASE + "/orders");"#,
    );
    assert_eq!(
        value.as_literal().as_deref(),
        Some("/api/orders"),
        "got {value:?}"
    );
}

#[test]
fn concatenation_with_an_unknown_keeps_it_unknown() {
    let value = url_of(
        "web/src/api.ts",
        r#"const BASE = "/api";
await fetch(BASE + "/orders/" + id);"#,
    );
    assert_eq!(value.to_string(), "/api/orders/{unknown}");
    assert_eq!(value.as_literal(), None);
}

// ── Slice 5: environment variables ──────────────────────────────────

#[test]
fn a_process_env_read_stays_symbolic_and_is_never_read() {
    let scope = Scope::new();
    let value = evaluate_expression("process.env.API_URL", &scope);
    assert_eq!(
        value,
        SymbolicValue::EnvVar {
            name: "API_URL".into()
        }
    );
    assert_eq!(value.as_literal(), None);
    assert_eq!(value.to_string(), "{env:API_URL}");
}

#[test]
fn vite_style_environment_reads_are_recognised() {
    let scope = Scope::new();
    assert_eq!(
        evaluate_expression("import.meta.env.VITE_API_URL", &scope),
        SymbolicValue::EnvVar {
            name: "VITE_API_URL".into()
        }
    );
}

#[test]
fn python_environment_reads_are_recognised() {
    let scope = Scope::new();
    for expr in [r#"os.environ["API_URL"]"#, r#"os.environ.get("API_URL")"#] {
        assert_eq!(
            evaluate_expression(expr, &scope),
            SymbolicValue::EnvVar {
                name: "API_URL".into()
            },
            "{expr}"
        );
    }
}

#[test]
fn an_environment_backed_url_reconstructs_the_known_part_only() {
    let value = url_of(
        "web/src/api.ts",
        r#"await fetch(`${process.env.API_URL}/orders/${id}`);"#,
    );
    assert_eq!(value.to_string(), "{env:API_URL}/orders/{unknown}");
    assert_eq!(value.as_literal(), None);
}

#[test]
fn the_analysing_machines_environment_is_never_consulted() {
    // PATH is set on every machine this can run on, so if the evaluator read
    // the environment its value would appear here — leaking configuration into
    // the graph and making the analysis depend on where it ran.
    let actual = std::env::var("PATH").expect("PATH is set");
    assert!(
        !actual.is_empty(),
        "the test needs a non-empty variable to detect a leak"
    );

    let value = evaluate_expression("process.env.PATH", &Scope::new());

    assert_eq!(
        value,
        SymbolicValue::EnvVar {
            name: "PATH".into()
        }
    );
    assert_eq!(
        value.as_literal(),
        None,
        "an environment read is never a known value"
    );
    assert!(
        !value.to_string().contains(&actual),
        "the environment must never be read into the graph"
    );
    assert_eq!(value.to_string(), "{env:PATH}", "only the name is recorded");
}

// ── Slice 6: symbolic unknowns and unsupported forms ────────────────

#[test]
fn an_unbound_identifier_is_unknown_not_unsupported() {
    assert_eq!(
        evaluate_expression("orderId", &Scope::new()),
        SymbolicValue::Unknown {
            name: "orderId".into()
        }
    );
}

#[test]
fn a_call_expression_is_unsupported_not_guessed() {
    let value = evaluate_expression("buildUrl()", &Scope::new());
    assert!(
        matches!(value, SymbolicValue::Unsupported { .. }),
        "a call must not be interpreted, got {value:?}"
    );
    assert_eq!(value.as_literal(), None);
}

#[test]
fn indexing_and_operators_are_unsupported() {
    for expr in ["paths[0]", "a ? b : c", "x || y", "items.map(f)"] {
        let value = evaluate_expression(expr, &Scope::new());
        assert_eq!(value.as_literal(), None, "{expr} must not resolve");
        assert!(
            matches!(value, SymbolicValue::Unsupported { .. }),
            "{expr} should be unsupported, got {value:?}"
        );
    }
}

#[test]
fn a_member_access_is_unknown_rather_than_unsupported() {
    // `config.baseUrl` names a value this analysis cannot follow; it is not a
    // form it fails to parse.
    assert_eq!(
        evaluate_expression("config.baseUrl", &Scope::new()),
        SymbolicValue::Unknown {
            name: "config.baseUrl".into()
        }
    );
}

#[test]
fn an_empty_expression_is_unsupported() {
    assert!(matches!(
        evaluate_expression("", &Scope::new()),
        SymbolicValue::Unsupported { .. }
    ));
}

#[test]
fn a_constant_bound_to_a_call_result_does_not_resolve() {
    let scope = scope_of("web/src/api.ts", r#"const BASE = buildBase();"#);
    let value = evaluate_expression("BASE", &scope);
    assert_eq!(
        value.as_literal(),
        None,
        "a constant is only known when its initialiser is a string, got {value:?}"
    );
}

#[test]
fn a_local_variable_is_not_treated_as_a_module_constant() {
    let scope = scope_of(
        "web/src/api.ts",
        r#"function f() {
    const BASE = "/inner";
    return BASE;
}"#,
    );
    assert_eq!(
        evaluate_expression("BASE", &scope).as_literal(),
        None,
        "a local's value depends on control flow this analysis does not model"
    );
}

// ── Transitive constants and cycles ─────────────────────────────────

#[test]
fn constants_defined_in_terms_of_others_resolve_transitively() {
    let value = url_of(
        "web/src/api.ts",
        r#"const ROOT = "/api";
const BASE = `${ROOT}/v1`;
await fetch(`${BASE}/orders`);"#,
    );
    assert_eq!(
        value.as_literal().as_deref(),
        Some("/api/v1/orders"),
        "got {value:?}"
    );
}

#[test]
fn a_cyclic_definition_terminates_as_unknown() {
    let mut scope = Scope::new();
    scope.bind("A", SymbolicValue::Unknown { name: "B".into() });
    scope.bind("B", SymbolicValue::Unknown { name: "A".into() });

    let value = evaluate_expression("A", &scope);
    assert_eq!(value.as_literal(), None, "a cycle must not resolve");
    // The real assertion is that this returned at all rather than recursing.
}

// ── Determinism ─────────────────────────────────────────────────────

#[test]
fn evaluation_is_deterministic() {
    let src = r#"const BASE = "/api/v1";
await fetch(`${BASE}/orders/${id}`);"#;
    assert_eq!(url_of("web/src/api.ts", src), url_of("web/src/api.ts", src));
}
