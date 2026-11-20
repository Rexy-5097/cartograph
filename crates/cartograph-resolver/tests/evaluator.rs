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
    let value = url_of("web/src/api.ts", r"await fetch(`/orders/${id}`);");
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
        r"await fetch(`${process.env.API_URL}/orders/${id}`);",
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
    let scope = scope_of("web/src/api.ts", r"const BASE = buildBase();");
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

// ── T-003: a `+` chain given as expression text ─────────────────────

#[test]
fn a_plus_chain_of_literals_and_constants_evaluates() {
    let scope = scope_of("src/c.ts", "const BASE = '/api';\nconst V = 'v1';\n");
    for (expr, expected) in [
        ("BASE + '/orders'", "/api/orders"),
        ("BASE+\"/orders\"", "/api/orders"),
        ("'/api' + '/orders'", "/api/orders"),
        ("BASE + '/' + V + '/orders'", "/api/v1/orders"),
        ("  BASE  +  '/a+b'  ", "/api/a+b"),
    ] {
        let value = evaluate_expression(expr, &scope);
        assert_eq!(
            value.as_literal().as_deref(),
            Some(expected),
            "{expr} gave {value:?}"
        );
    }
}

#[test]
fn a_plus_chain_keeps_what_it_cannot_determine_unknown() {
    let scope = scope_of("src/c.ts", "const BASE = '/api';\n");
    let value = evaluate_expression("BASE + '/orders/' + orderId", &scope);
    assert_eq!(value.as_literal(), None);
    assert_eq!(value.to_string(), "/api/orders/{unknown}");

    // A member access and an environment read keep their existing meaning.
    assert_eq!(
        evaluate_expression("config.base + '/orders'", &scope).to_string(),
        "{unknown}/orders"
    );
    assert_eq!(
        evaluate_expression("process.env.API + '/orders'", &scope).to_string(),
        "{env:API}/orders"
    );
}

#[test]
fn text_that_is_not_a_plain_plus_chain_stays_unsupported() {
    let scope = scope_of("src/c.ts", "const BASE = '/api';\n");
    for expr in [
        "BASE + build()",
        "BASE + paths[0]",
        "(BASE + '/x')",
        "BASE + '/x' + (a ? b : c)",
        "BASE - '/x'",
        "BASE + `/x`",
        "BASE + + '/x'",
        "+BASE",
        "BASE +",
        "BASE++",
        "BASE + 'it\\'s'",
        "BASE + '/x",
        "BASE '/x'",
        "BASE + a b",
        "a ? BASE + '/x' : '/y'",
        "BASE + '/x' || '/y'",
    ] {
        let value = evaluate_expression(expr, &scope);
        assert_eq!(value.as_literal(), None, "{expr} must not resolve");
        assert!(
            matches!(value, SymbolicValue::Unsupported { .. }),
            "{expr} should be unsupported, got {value:?}"
        );
    }
}

#[test]
fn a_constant_whose_initialiser_only_contains_a_string_is_not_bound() {
    // Second review, S-08: the first string in the initialiser was taken.
    let scope = scope_of(
        "src/c.ts",
        "export const A = isProd ? '/api' : '/internal';\nexport const B = '/api'.replace('/api', '/v2');\nexport const C = '/api';\n",
    );
    assert_eq!(evaluate_expression("A + '/x'", &scope).as_literal(), None);
    assert_eq!(evaluate_expression("B + '/x'", &scope).as_literal(), None);
    assert_eq!(
        evaluate_expression("C + '/x'", &scope)
            .as_literal()
            .as_deref(),
        Some("/api/x")
    );
}

// ── T-010: a constant is bound only when its value *is* a string ─────────

#[test]
fn a_value_holding_one_string_at_its_end_is_not_that_string() {
    // Third review, U-01: the last string ended where the declaration ended,
    // so `isProd ? PROD : '/internal'` was read as `/internal`.
    let scope = scope_of(
        "src/c.ts",
        "export const F1 = isProd ? PROD : '/internal';
export const F2 = cfg.base ?? '/internal';
export const F3 = x || '/internal';
export const F4 = import.meta.env.PROD ? import.meta.env.VITE_API : '/internal';
export const F5 = () => '/internal';
",
    );
    for name in ["F1", "F2", "F3", "F4", "F5"] {
        assert_eq!(
            evaluate_expression(&format!("{name} + '/x'"), &scope).as_literal(),
            None,
            "{name} was bound"
        );
    }
}

#[test]
fn a_value_split_over_lines_is_not_its_last_string() {
    // Third review, U-02: a string left of the declarator's column on a later
    // line was not seen.
    let scope = scope_of(
        "src/c.ts",
        "export const F3 = isProd
  ? '/prod' : '/internal';
export const F4 =
  isProd ? PROD
: '/internal';
",
    );
    assert_eq!(evaluate_expression("F3 + '/x'", &scope).as_literal(), None);
    assert_eq!(evaluate_expression("F4 + '/x'", &scope).as_literal(), None);
}

#[test]
fn a_string_in_parentheses_or_a_type_assertion_is_still_the_value() {
    // Third review, U-03: these were bound before the second review's fix and
    // refused after it.
    let scope = scope_of(
        "src/c.ts",
        "export const A = '/api' as const;
export const B = '/api' satisfies string;
export const C = ('/api');
export const D = '/api', E = '/x';
export const T = `/api/v1`;
",
    );
    for name in ["A", "B", "C", "D"] {
        assert_eq!(
            evaluate_expression(&format!("{name} + '/x'"), &scope)
                .as_literal()
                .as_deref(),
            Some("/api/x"),
            "{name} was not bound"
        );
    }
    assert_eq!(
        evaluate_expression("E", &scope).as_literal().as_deref(),
        Some("/x")
    );
    assert_eq!(
        evaluate_expression("T + '/x'", &scope)
            .as_literal()
            .as_deref(),
        Some("/api/v1/x")
    );
}

#[test]
fn python_constants_are_bound_by_the_same_rule() {
    let scope = scope_of(
        "svc/c.py",
        "BASE_A = \"/api\"
BASE_B = (\"/api\")
BASE_T = settings.BASE if DEBUG else \"/internal\"
BASE_O = settings.BASE or \"/internal\"
",
    );
    for name in ["BASE_A", "BASE_B"] {
        assert_eq!(
            evaluate_expression(&format!("{name} + '/x'"), &scope)
                .as_literal()
                .as_deref(),
            Some("/api/x"),
            "{name}"
        );
    }
    for name in ["BASE_T", "BASE_O"] {
        assert_eq!(
            evaluate_expression(&format!("{name} + '/x'"), &scope).as_literal(),
            None,
            "{name} was bound"
        );
    }
}

// ── Fourth review ────────────────────────────────────────────────────────

#[test]
fn a_let_or_var_is_not_a_constant() {
    // V-01: a top-level `API = …` runs at module load, before any call.
    let scope = scope_of(
        "src/c.ts",
        "export let A = '/api/a/';
A = A + 'secret/';
export var B = '/api/b/';
export const C = '/api/c/';
",
    );
    assert_eq!(evaluate_expression("A + 'x'", &scope).as_literal(), None);
    assert_eq!(evaluate_expression("B + 'x'", &scope).as_literal(), None);
    assert_eq!(
        evaluate_expression("C + 'x'", &scope)
            .as_literal()
            .as_deref(),
        Some("/api/c/x")
    );
}

#[test]
fn a_name_declared_twice_at_module_level_is_not_a_constant() {
    // V-02, V-03: a later assignment, or a block declaration of the same name.
    let ts = scope_of(
        "src/c.ts",
        "const A = '/api/a/';
if (isProd) { const A = '/api/x/'; }
{ let B = '/api/b/'; }
",
    );
    assert_eq!(evaluate_expression("A + 'x'", &ts).as_literal(), None);
    let py = scope_of(
        "svc/c.py",
        "BASE2 = \"/api/a/\"
BASE2 = BASE2.replace(\"a\", \"x\")
BASE3 = \"/api/a/\"
if DEBUG:
    BASE3 = \"/api/x/\"
BASE4 = \"/api/a/\"
",
    );
    assert_eq!(evaluate_expression("BASE2 + 'x'", &py).as_literal(), None);
    assert_eq!(evaluate_expression("BASE3 + 'x'", &py).as_literal(), None);
    assert_eq!(
        evaluate_expression("BASE4 + 'x'", &py)
            .as_literal()
            .as_deref(),
        Some("/api/a/x")
    );
}

#[test]
fn a_comment_or_a_non_null_assertion_does_not_hide_the_string() {
    // V-10.
    let ts = scope_of(
        "src/c.ts",
        "export const K = (/* base */ '/api/k/');\nexport const N = '/api/n/'!;\n",
    );
    for name in ["K", "N"] {
        assert!(
            evaluate_expression(&format!("{name} + 'x'"), &ts)
                .as_literal()
                .is_some(),
            "{name}"
        );
    }
    let py = scope_of("svc/c.py", "BASE = (  # c\n    \"/api/a/\"\n)\n");
    assert_eq!(
        evaluate_expression("BASE + 'x'", &py)
            .as_literal()
            .as_deref(),
        Some("/api/a/x")
    );
}
