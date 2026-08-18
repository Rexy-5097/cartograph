//! Matching rules, and the cases where matching must refuse.
//!
//! The rule tests pin behaviour; the false-positive tests are the ones that
//! protect the product's credibility. A resolver that connects everything is
//! worthless, so most of what follows asserts that no edge is produced.

use cartograph_parser::model::{
    HttpCallObservation, HttpMethodHint, RouteDeclarationStyle, RouteObservation, SourceLanguage,
    Span, TemplatePart, UrlObservation,
};
use cartograph_resolver::{
    MatchResult, MatchStatus, MethodCompatibility, PathCompatibility, RouteIndex,
    UnsupportedReason, match_client, normalize_client_call, normalize_route_declaration, priors,
};

fn span() -> Span {
    Span {
        start_line: 34,
        start_column: 5,
        end_line: 34,
        end_column: 40,
    }
}

/// Builds a backend index from `(path, methods, handler)` triples.
fn backend(routes: &[(&str, &[HttpMethodHint], &str)]) -> RouteIndex {
    RouteIndex::build(routes.iter().map(|(path, methods, handler)| {
        normalize_route_declaration(
            &RouteObservation {
                style: RouteDeclarationStyle::VerbDecorator,
                methods: methods.to_vec(),
                path: UrlObservation::Literal {
                    value: (*path).to_owned(),
                },
                handler: Some((*handler).to_owned()),
                span: span(),
            },
            "api/routes.py",
            SourceLanguage::Python,
        )
    }))
}

fn client(url: &str, method: Option<HttpMethodHint>) -> HttpCallObservation {
    HttpCallObservation {
        callee: "axios.post".into(),
        method_hint: method,
        url: UrlObservation::Literal {
            value: url.to_owned(),
        },
        span: span(),
    }
}

fn go(url: &str, method: Option<HttpMethodHint>, index: &RouteIndex) -> MatchResult {
    let canonical = normalize_client_call(
        &client(url, method),
        "web/src/api.ts",
        SourceLanguage::TypeScript,
    );
    match_client(&canonical, index)
}

const POST: Option<HttpMethodHint> = Some(HttpMethodHint::Post);
const GET: Option<HttpMethodHint> = Some(HttpMethodHint::Get);

// ── Method rules ────────────────────────────────────────────────────

#[test]
fn identical_methods_are_exactly_compatible() {
    let index = backend(&[("/orders", &[HttpMethodHint::Post], "create")]);
    let result = go("/orders", POST, &index);
    assert_eq!(result.status, MatchStatus::Exact);
    assert_eq!(result.candidates[0].method, MethodCompatibility::Exact);
}

#[test]
fn a_client_method_outside_the_declared_set_is_rejected() {
    let index = backend(&[("/orders", &[HttpMethodHint::Post], "create")]);
    let result = go("/orders", GET, &index);
    assert_eq!(result.status, MatchStatus::NoMatch);
    assert!(
        result.candidates.is_empty(),
        "an incompatible method is not a weak candidate, it is not a candidate"
    );
}

#[test]
fn a_client_method_within_a_declared_set_matches() {
    let index = backend(&[(
        "/orders",
        &[HttpMethodHint::Get, HttpMethodHint::Post],
        "orders",
    )]);
    assert_eq!(go("/orders", POST, &index).status, MatchStatus::Exact);
    assert_eq!(go("/orders", GET, &index).status, MatchStatus::Exact);
    assert_eq!(
        go("/orders", Some(HttpMethodHint::Delete), &index).status,
        MatchStatus::NoMatch
    );
}

#[test]
fn an_undeclared_client_method_is_a_candidate_but_never_exact() {
    let index = backend(&[("/orders", &[HttpMethodHint::Post], "create")]);
    let result = go("/orders", None, &index);

    assert_eq!(result.status, MatchStatus::Strong);
    assert_eq!(
        result.candidates[0].method,
        MethodCompatibility::ClientUnknown
    );
    assert!(
        result.candidates[0].confidence.get() < priors::EXACT_ROUTE_AND_METHOD.get(),
        "an undeclared method must weaken the claim"
    );
}

#[test]
fn an_undeclared_route_method_is_a_candidate_but_never_exact() {
    // Flask's `@app.route("/x")` declares no method.
    let index = backend(&[("/orders", &[], "orders")]);
    let result = go("/orders", POST, &index);

    assert_eq!(result.status, MatchStatus::Strong);
    assert_eq!(
        result.candidates[0].method,
        MethodCompatibility::ServerUnknown
    );
    assert!(result.candidates[0].confidence.get() < priors::EXACT_ROUTE_AND_METHOD.get());
}

#[test]
fn neither_side_declaring_a_method_is_still_not_exact() {
    let index = backend(&[("/orders", &[], "orders")]);
    let result = go("/orders", None, &index);
    assert_eq!(result.status, MatchStatus::Strong);
    assert_eq!(
        result.candidates[0].method,
        MethodCompatibility::BothUnknown
    );
}

#[test]
fn an_undeclared_method_is_never_read_as_get() {
    // If a missing method were defaulted to GET, this would be NoMatch.
    let index = backend(&[("/orders", &[HttpMethodHint::Post], "create")]);
    assert_eq!(
        go("/orders", None, &index).status,
        MatchStatus::Strong,
        "an undeclared method excludes nothing; defaulting it to GET would invent a fact"
    );
}

// ── Path rules ──────────────────────────────────────────────────────

#[test]
fn every_static_segment_must_be_equal() {
    let index = backend(&[("/api/v1/orders", &[HttpMethodHint::Post], "create")]);
    assert_eq!(
        go("/api/v1/orders", POST, &index).status,
        MatchStatus::Exact
    );
    for miss in ["/api/v2/orders", "/api/v1/invoices", "/apiv1/orders"] {
        assert_eq!(
            go(miss, POST, &index).status,
            MatchStatus::NoMatch,
            "{miss} must not match"
        );
    }
}

#[test]
fn segment_counts_must_agree_when_there_is_no_wildcard() {
    let index = backend(&[("/orders/{id}", &[HttpMethodHint::Post], "replace")]);
    assert_eq!(go("/orders", POST, &index).status, MatchStatus::NoMatch);
    assert_eq!(
        go("/orders/1/items", POST, &index).status,
        MatchStatus::NoMatch
    );
    assert_eq!(go("/orders/1", POST, &index).status, MatchStatus::Strong);
}

#[test]
fn a_concrete_value_binds_a_declared_parameter() {
    let index = backend(&[("/orders/{id}", &[HttpMethodHint::Post], "replace")]);
    let result = go("/orders/123", POST, &index);
    assert_eq!(result.candidates[0].path, PathCompatibility::ParameterBound);
    assert_eq!(result.candidates[0].confidence, priors::PARAMETER_BOUND);
}

#[test]
fn several_parameters_bind_position_by_position() {
    let index = backend(&[("/orders/{id}/items/{item}", &[HttpMethodHint::Post], "item")]);
    assert_eq!(
        go("/orders/1/items/9", POST, &index).status,
        MatchStatus::Strong
    );
    assert_eq!(
        go("/orders/1/products/9", POST, &index).status,
        MatchStatus::NoMatch,
        "an interleaved static segment must still match exactly"
    );
}

#[test]
fn an_unknown_client_value_aligns_with_a_parameter_but_stays_undetermined() {
    let index = backend(&[("/orders/{id}", &[HttpMethodHint::Post], "replace")]);
    let canonical = normalize_client_call(
        &HttpCallObservation {
            callee: "axios.post".into(),
            method_hint: POST,
            url: UrlObservation::Template {
                parts: vec![
                    TemplatePart::Text {
                        value: "/orders/".into(),
                    },
                    TemplatePart::Expression {
                        source: "orderId".into(),
                        span: span(),
                    },
                ],
            },
            span: span(),
        },
        "web/src/api.ts",
        SourceLanguage::TypeScript,
    );
    let result = match_client(&canonical, &index);

    assert_eq!(result.status, MatchStatus::Strong);
    assert_eq!(
        result.candidates[0].path,
        PathCompatibility::VariableAligned
    );
    assert_eq!(result.candidates[0].confidence, priors::VARIABLE_ALIGNED);
    assert!(
        result.candidates[0].confidence.get() < priors::PARAMETER_BOUND.get(),
        "an unknown value is weaker evidence than a concrete one"
    );
}

// ── Wildcards ───────────────────────────────────────────────────────

#[test]
fn a_trailing_wildcard_consumes_remaining_segments() {
    let index = backend(&[("/files/{rest:path}", &[HttpMethodHint::Get], "serve")]);
    let result = go("/files/a/b/c", GET, &index);
    assert_eq!(result.candidates[0].path, PathCompatibility::WildcardSuffix);
    assert_eq!(result.candidates[0].confidence, priors::WILDCARD_SUFFIX);
}

#[test]
fn a_wildcard_requires_at_least_one_segment_to_consume() {
    let index = backend(&[("/files/{rest:path}", &[HttpMethodHint::Get], "serve")]);
    assert_eq!(
        go("/files", GET, &index).status,
        MatchStatus::NoMatch,
        "a wildcard is not optional"
    );
}

#[test]
fn a_wildcard_does_not_ignore_the_prefix_before_it() {
    let index = backend(&[("/files/{rest:path}", &[HttpMethodHint::Get], "serve")]);
    assert_eq!(go("/assets/a/b", GET, &index).status, MatchStatus::NoMatch);
}

#[test]
fn a_wildcard_match_is_the_weakest_accepted_evidence() {
    assert!(priors::WILDCARD_SUFFIX.get() < priors::VARIABLE_ALIGNED.get());
    assert!(priors::VARIABLE_ALIGNED.get() < priors::PARAMETER_BOUND.get());
    assert!(priors::PARAMETER_BOUND.get() < priors::EXACT_ROUTE_AND_METHOD.get());
}

// ── Ambiguity ───────────────────────────────────────────────────────

#[test]
fn equally_plausible_routes_are_ambiguous_and_expose_no_accepted_candidate() {
    let index = backend(&[
        ("/orders/{id}", &[HttpMethodHint::Post], "by_id"),
        ("/orders/{slug}", &[HttpMethodHint::Post], "by_slug"),
    ]);
    let result = go("/orders/123", POST, &index);

    assert_eq!(result.status, MatchStatus::Ambiguous);
    assert_eq!(result.candidates.len(), 2);
    assert!(result.accepted().is_none());
}

#[test]
fn a_stronger_candidate_wins_outright_over_a_weaker_one() {
    // A literal route and a parameterised route both accept /orders/latest,
    // but the literal is determined and the parameter is not.
    let index = backend(&[
        ("/orders/latest", &[HttpMethodHint::Get], "latest"),
        ("/orders/{id}", &[HttpMethodHint::Get], "by_id"),
    ]);
    let result = go("/orders/latest", GET, &index);

    assert_eq!(result.status, MatchStatus::Exact);
    assert_eq!(
        result.candidates.len(),
        2,
        "the weaker candidate is retained"
    );
    assert_eq!(
        result
            .accepted()
            .unwrap()
            .route
            .provenance
            .handler
            .as_deref(),
        Some("latest")
    );
}

#[test]
fn nested_routes_of_different_lengths_do_not_compete() {
    let index = backend(&[
        ("/orders", &[HttpMethodHint::Post], "create"),
        ("/orders/{id}", &[HttpMethodHint::Post], "replace"),
        ("/orders/{id}/submit", &[HttpMethodHint::Post], "submit"),
    ]);
    assert_eq!(go("/orders", POST, &index).status, MatchStatus::Exact);
    assert_eq!(go("/orders/1", POST, &index).status, MatchStatus::Strong);
    assert_eq!(
        go("/orders/1/submit", POST, &index).status,
        MatchStatus::Strong
    );
}

// ── FALSE-POSITIVE DEFENCE ──────────────────────────────────────────

#[test]
fn identical_endpoints_in_unrelated_services_are_ambiguous_not_joined() {
    // Two services in one monorepo both expose POST /health. Cartograph has no
    // evidence about which one a client reaches.
    let index = RouteIndex::build(
        [
            ("billing/routes.py", "billing_health"),
            ("shipping/routes.py", "shipping_health"),
        ]
        .iter()
        .map(|(file, handler)| {
            normalize_route_declaration(
                &RouteObservation {
                    style: RouteDeclarationStyle::VerbDecorator,
                    methods: vec![HttpMethodHint::Post],
                    path: UrlObservation::Literal {
                        value: "/health".into(),
                    },
                    handler: Some((*handler).to_owned()),
                    span: span(),
                },
                file,
                SourceLanguage::Python,
            )
        }),
    );

    let result = go("/health", POST, &index);
    assert_eq!(
        result.status,
        MatchStatus::Ambiguous,
        "picking a service without evidence is exactly the failure mode this milestone must avoid"
    );
}

#[test]
fn an_absolute_url_to_another_host_is_refused() {
    let index = backend(&[("/orders", &[HttpMethodHint::Post], "create")]);
    let result = go("https://api.stripe.com/orders", POST, &index);

    assert_eq!(result.status, MatchStatus::Unsupported);
    assert_eq!(
        result.unsupported_reason,
        Some(UnsupportedReason::ClientTargetsAbsoluteHost),
        "a third-party call must not be joined to a local route"
    );
    assert!(result.accepted().is_none());
}

#[test]
fn an_unresolved_client_prefix_is_refused_rather_than_suffix_matched() {
    let index = backend(&[("/orders/{id}", &[HttpMethodHint::Post], "replace")]);
    let canonical = normalize_client_call(
        &HttpCallObservation {
            callee: "axios.post".into(),
            method_hint: POST,
            url: UrlObservation::Template {
                parts: vec![
                    TemplatePart::Expression {
                        source: "BASE".into(),
                        span: span(),
                    },
                    TemplatePart::Text {
                        value: "/orders/".into(),
                    },
                    TemplatePart::Expression {
                        source: "id".into(),
                        span: span(),
                    },
                ],
            },
            span: span(),
        },
        "web/src/api.ts",
        SourceLanguage::TypeScript,
    );
    let result = match_client(&canonical, &index);

    assert_eq!(result.status, MatchStatus::Unsupported);
    assert_eq!(
        result.unsupported_reason,
        Some(UnsupportedReason::ClientPrefixUnresolved),
        "the prefix's segment count is unknown, so no alignment is safe until M05"
    );
}

#[test]
fn a_client_path_that_could_not_be_canonicalised_is_refused() {
    let index = backend(&[("/orders", &[HttpMethodHint::Post], "create")]);
    let canonical = normalize_client_call(
        &HttpCallObservation {
            callee: "requests.post".into(),
            method_hint: POST,
            url: UrlObservation::Dynamic {
                source: "url".into(),
            },
            span: span(),
        },
        "api/client.py",
        SourceLanguage::Python,
    );
    let result = match_client(&canonical, &index);

    assert_eq!(result.status, MatchStatus::Unsupported);
    assert_eq!(
        result.unsupported_reason,
        Some(UnsupportedReason::ClientPathNotCanonical)
    );
}

#[test]
fn an_unsupported_route_is_never_indexed() {
    let index = RouteIndex::build([normalize_route_declaration(
        &RouteObservation {
            style: RouteDeclarationStyle::UrlConfEntry,
            methods: Vec::new(),
            path: UrlObservation::Literal {
                value: r"^orders/(\d+)$".into(),
            },
            handler: Some("views.detail".into()),
            span: span(),
        },
        "api/urls.py",
        SourceLanguage::Python,
    )]);

    assert!(
        index.is_empty(),
        "an uninterpreted regex offers nothing to match"
    );
    assert_eq!(go("/orders/1", POST, &index).status, MatchStatus::NoMatch);
}

#[test]
fn a_client_call_is_never_indexed_as_a_backend_route() {
    // Feeding client observations into the route index would let two client
    // calls "match" each other.
    let index = RouteIndex::build([normalize_client_call(
        &client("/orders", POST),
        "web/src/api.ts",
        SourceLanguage::TypeScript,
    )]);
    assert!(
        index.is_empty(),
        "only route declarations may be candidates"
    );
}

#[test]
fn a_query_string_difference_does_not_create_a_path_mismatch() {
    // Query is metadata, not path structure (M03), so it neither breaks nor
    // creates a match.
    let index = backend(&[("/orders", &[HttpMethodHint::Post], "create")]);
    assert_eq!(
        go("/orders?expand=items", POST, &index).status,
        MatchStatus::Exact
    );
}

#[test]
fn a_numeric_looking_route_segment_is_still_a_literal() {
    // The route declares a literal `2024`, not a parameter, so only that exact
    // value reaches it.
    let index = backend(&[("/reports/2024", &[HttpMethodHint::Get], "report_2024")]);
    assert_eq!(go("/reports/2024", GET, &index).status, MatchStatus::Exact);
    assert_eq!(
        go("/reports/2025", GET, &index).status,
        MatchStatus::NoMatch
    );
}

#[test]
fn an_unknown_client_value_against_a_literal_is_possible_but_never_exact() {
    let index = backend(&[("/orders/latest", &[HttpMethodHint::Get], "latest")]);
    let canonical = normalize_client_call(
        &HttpCallObservation {
            callee: "axios.get".into(),
            method_hint: GET,
            url: UrlObservation::Template {
                parts: vec![
                    TemplatePart::Text {
                        value: "/orders/".into(),
                    },
                    TemplatePart::Expression {
                        source: "which".into(),
                        span: span(),
                    },
                ],
            },
            span: span(),
        },
        "web/src/api.ts",
        SourceLanguage::TypeScript,
    );
    let result = match_client(&canonical, &index);

    assert_eq!(result.status, MatchStatus::Strong);
    assert_eq!(
        result.candidates[0].path,
        PathCompatibility::VariableAligned
    );
    assert!(
        result.candidates[0].confidence.get() < priors::EXACT_ROUTE_AND_METHOD.get(),
        "the value was never verified to be `latest`"
    );
}

#[test]
fn an_empty_backend_matches_nothing() {
    let index = backend(&[]);
    assert!(index.is_empty());
    assert_eq!(go("/orders", POST, &index).status, MatchStatus::NoMatch);
}

// ── Determinism ─────────────────────────────────────────────────────

#[test]
fn matching_is_deterministic_including_candidate_order() {
    let index = backend(&[
        ("/orders/{id}", &[HttpMethodHint::Post], "a"),
        ("/orders/latest", &[HttpMethodHint::Post], "b"),
        ("/orders/{slug}", &[HttpMethodHint::Post], "c"),
    ]);
    let first = go("/orders/latest", POST, &index);
    let second = go("/orders/latest", POST, &index);

    assert_eq!(first.status, second.status);
    let names = |r: &MatchResult| -> Vec<String> {
        r.candidates
            .iter()
            .map(|c| c.route.provenance.handler.clone().unwrap_or_default())
            .collect()
    };
    assert_eq!(names(&first), names(&second));
}

#[test]
fn candidate_order_does_not_depend_on_declaration_order() {
    let forward = backend(&[
        ("/orders/{id}", &[HttpMethodHint::Post], "a"),
        ("/orders/latest", &[HttpMethodHint::Post], "b"),
    ]);
    let reversed = backend(&[
        ("/orders/latest", &[HttpMethodHint::Post], "b"),
        ("/orders/{id}", &[HttpMethodHint::Post], "a"),
    ]);
    assert_eq!(
        go("/orders/latest", POST, &forward)
            .accepted()
            .unwrap()
            .route
            .provenance
            .handler,
        go("/orders/latest", POST, &reversed)
            .accepted()
            .unwrap()
            .route
            .provenance
            .handler
    );
}

// ── Indexing ────────────────────────────────────────────────────────

#[test]
fn the_index_reports_what_it_holds() {
    let index = backend(&[
        ("/a", &[HttpMethodHint::Get], "a"),
        ("/b/{id}", &[HttpMethodHint::Get], "b"),
    ]);
    assert_eq!(index.len(), 2);
    assert!(!index.is_empty());
}

#[test]
fn indexing_does_not_change_results_at_scale() {
    // A large backend must produce the same answer as a small one for the same
    // route: candidate generation is an optimisation, not a semantic filter.
    let mut routes: Vec<(String, Vec<HttpMethodHint>, String)> = (0..500)
        .map(|i| {
            (
                format!("/api/resource{i}/{{id}}"),
                vec![HttpMethodHint::Get],
                format!("h{i}"),
            )
        })
        .collect();
    routes.push((
        "/api/orders/{id}".into(),
        vec![HttpMethodHint::Post],
        "create".into(),
    ));

    let index = RouteIndex::build(routes.iter().map(|(p, m, h)| {
        normalize_route_declaration(
            &RouteObservation {
                style: RouteDeclarationStyle::VerbDecorator,
                methods: m.clone(),
                path: UrlObservation::Literal { value: p.clone() },
                handler: Some(h.clone()),
                span: span(),
            },
            "api/routes.py",
            SourceLanguage::Python,
        )
    }));

    let result = go("/api/orders/7", POST, &index);
    assert_eq!(result.status, MatchStatus::Strong);
    assert_eq!(
        result
            .accepted()
            .unwrap()
            .route
            .provenance
            .handler
            .as_deref(),
        Some("create")
    );
}

// ── Discriminating evidence ─────────────────────────────────────────
//
// Both cases below were found by running the matcher over real repositories,
// not by reasoning about it. They are the strongest argument for why real
// corpora belong in the loop.

#[test]
fn a_bare_catch_all_route_does_not_swallow_unrelated_calls() {
    // Found in pallets/flask: one test module declares a catch-all, and it was
    // matching client calls from every other module in the repository.
    let index = backend(&[("/{path:path}", &[], "catch_all")]);
    let result = go("/auth/login", POST, &index);

    assert_eq!(
        result.status,
        MatchStatus::Ambiguous,
        "a route that accepts every path is not evidence about any one call"
    );
    assert!(result.accepted().is_none(), "and so it produces no edge");
    assert!(
        !result.candidates.is_empty(),
        "the candidate is still visible for inspection"
    );
    assert!(!result.candidates[0].discriminating);
}

#[test]
fn a_single_parameter_route_does_not_capture_every_one_segment_call() {
    // Found in pallets/flask: `GET /user_id` matched `GET /{name}` declared in
    // an unrelated test module.
    let index = backend(&[("/{name}", &[HttpMethodHint::Get], "index")]);
    let result = go("/user_id", GET, &index);

    assert_eq!(result.status, MatchStatus::Ambiguous);
    assert!(result.accepted().is_none());
}

#[test]
fn one_shared_static_segment_is_enough_to_discriminate() {
    let index = backend(&[("/api/{name}", &[HttpMethodHint::Get], "index")]);
    let result = go("/api/user_id", GET, &index);

    assert_eq!(result.status, MatchStatus::Strong);
    assert!(result.candidates[0].discriminating);
}

#[test]
fn a_wildcard_still_matches_when_its_prefix_is_static() {
    let index = backend(&[("/files/{rest:path}", &[HttpMethodHint::Get], "serve")]);
    let result = go("/files/a/b", GET, &index);

    assert_eq!(result.status, MatchStatus::Strong);
    assert!(
        result.candidates[0].discriminating,
        "the `files` prefix is what makes the wildcard meaningful"
    );
}

#[test]
fn the_root_path_matches_only_the_root_route() {
    let index = backend(&[("/", &[HttpMethodHint::Get], "root")]);
    let result = go("/", GET, &index);
    assert_eq!(
        result.status,
        MatchStatus::Exact,
        "an empty path is discriminating by equality"
    );
    assert_eq!(go("/orders", GET, &index).status, MatchStatus::NoMatch);
}
