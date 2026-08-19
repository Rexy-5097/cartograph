//! The symbolic value model: what it determines, and what it refuses to.
//!
//! The invariant under test throughout: **an unresolved expression never
//! becomes a plausible-looking string.**

use cartograph_resolver::SymbolicValue;

fn lit(v: &str) -> SymbolicValue {
    SymbolicValue::Literal { value: v.into() }
}
fn unknown(n: &str) -> SymbolicValue {
    SymbolicValue::Unknown { name: n.into() }
}
fn env(n: &str) -> SymbolicValue {
    SymbolicValue::EnvVar { name: n.into() }
}
fn unsupported(s: &str) -> SymbolicValue {
    SymbolicValue::Unsupported { source: s.into() }
}
fn concat(parts: Vec<SymbolicValue>) -> SymbolicValue {
    SymbolicValue::Concat { parts }
}

// ── Determined values ───────────────────────────────────────────────

#[test]
fn a_literal_is_fully_known() {
    assert_eq!(lit("/api").as_literal().as_deref(), Some("/api"));
    assert!(lit("/api").is_fully_known());
    assert_eq!(lit("/api").to_string(), "/api");
}

#[test]
fn a_concatenation_of_literals_is_fully_known() {
    let v = concat(vec![lit("/api"), lit("/orders")]);
    assert_eq!(v.as_literal().as_deref(), Some("/api/orders"));
    assert!(v.is_fully_known());
}

// ── NEGATIVE: unresolved values never become strings ────────────────

#[test]
fn an_unknown_value_has_no_literal_form() {
    assert_eq!(unknown("id").as_literal(), None);
    assert!(!unknown("id").is_fully_known());
}

#[test]
fn an_environment_variable_has_no_literal_form() {
    assert_eq!(
        env("API_URL").as_literal(),
        None,
        "reading the analysing machine's environment would leak configuration \
         and make results unreproducible"
    );
}

#[test]
fn an_unsupported_expression_has_no_literal_form() {
    assert_eq!(unsupported("foo()").as_literal(), None);
}

#[test]
fn one_unresolved_part_makes_the_whole_concatenation_unresolved() {
    let v = concat(vec![lit("/api/orders/"), unknown("id")]);
    assert_eq!(
        v.as_literal(),
        None,
        "a partially known value is not a known value"
    );
}

#[test]
fn unresolved_values_never_render_as_plausible_strings() {
    // The failure mode this model exists to prevent: `undefined`, `null`,
    // `[object Object]` or an empty string flowing into a URL and matching a
    // real route. Every one of those looks like data.
    for value in [unknown("id"), env("API_URL"), unsupported("foo()")] {
        let rendered = value.to_string();
        for forbidden in ["undefined", "null", "[object Object]", "NaN"] {
            assert!(
                !rendered.contains(forbidden),
                "{value:?} rendered as {rendered}, which looks like a value"
            );
        }
        assert!(
            rendered.starts_with('{') && rendered.ends_with('}'),
            "an unresolved value must render as a visible marker, got {rendered}"
        );
        assert!(
            !rendered.is_empty(),
            "an empty rendering would vanish into a path"
        );
    }
}

#[test]
fn markers_are_distinguishable_from_each_other() {
    assert_eq!(unknown("id").to_string(), "{unknown}");
    assert_eq!(env("API_URL").to_string(), "{env:API_URL}");
    assert_eq!(unsupported("f()").to_string(), "{unsupported}");
}

#[test]
fn an_environment_variable_records_only_its_name() {
    // The name is evidence; the value is never read.
    let rendered = env("DATABASE_PASSWORD").to_string();
    assert_eq!(rendered, "{env:DATABASE_PASSWORD}");
    assert!(!rendered.contains("="), "no value is ever attached");
}

// ── Normalisation ───────────────────────────────────────────────────

#[test]
fn nested_concatenations_flatten() {
    let nested = concat(vec![concat(vec![lit("/api"), lit("/v1")]), unknown("id")]);
    let flat = nested.normalized();
    assert_eq!(flat.parts().len(), 2, "nesting collapsed: {flat:?}");
    assert_eq!(flat.to_string(), "/api/v1{unknown}");
}

#[test]
fn adjacent_literals_merge() {
    let v = concat(vec![lit("/api"), lit("/v1"), lit("/orders")]).normalized();
    assert_eq!(
        v,
        lit("/api/v1/orders"),
        "a fully literal chain collapses to one literal"
    );
}

#[test]
fn normalisation_is_idempotent() {
    let v = concat(vec![
        concat(vec![lit("/a"), lit("/b")]),
        unknown("x"),
        lit("/c"),
    ]);
    let once = v.normalized();
    let twice = once.clone().normalized();
    assert_eq!(once, twice);
}

#[test]
fn equivalent_expressions_written_differently_compare_equal() {
    let a = concat(vec![lit("/api/v1"), unknown("id")]).normalized();
    let b = concat(vec![concat(vec![lit("/api"), lit("/v1")]), unknown("id")]).normalized();
    assert_eq!(a, b);
}

#[test]
fn empty_literals_are_dropped_without_changing_meaning() {
    let v = concat(vec![lit(""), lit("/api"), lit("")]).normalized();
    assert_eq!(v, lit("/api"));
}

// ── Evidence ────────────────────────────────────────────────────────

#[test]
fn a_determined_value_has_no_unresolved_reason() {
    assert_eq!(lit("/api").unresolved_reason(), None);
    assert_eq!(concat(vec![lit("/a"), lit("/b")]).unresolved_reason(), None);
}

#[test]
fn each_unresolved_kind_explains_itself() {
    assert!(
        unknown("id")
            .unresolved_reason()
            .unwrap()
            .contains("not statically known")
    );
    assert!(
        env("API_URL")
            .unresolved_reason()
            .unwrap()
            .contains("environment variable")
    );
    assert!(
        unsupported("f()")
            .unresolved_reason()
            .unwrap()
            .contains("supported expression subset"),
        "unsupported is distinct from unknown: a form not interpreted, not a value not determined"
    );
}

#[test]
fn a_concatenation_reports_every_unresolved_part() {
    let v = concat(vec![env("API_URL"), lit("/orders/"), unknown("id")]);
    let reason = v.unresolved_reason().unwrap();
    assert!(
        reason.contains("API_URL") && reason.contains("id"),
        "{reason}"
    );
}

// ── Round trip ──────────────────────────────────────────────────────

#[test]
fn symbolic_values_round_trip_through_json() {
    let v = concat(vec![
        env("API_URL"),
        lit("/orders/"),
        unknown("id"),
        unsupported("f()"),
    ]);
    let json = serde_json::to_string(&v).unwrap();
    let back: SymbolicValue = serde_json::from_str(&json).unwrap();
    assert_eq!(v, back);
}
