//! Canonicalisation tests.
//!
//! Every test asserts an **exact** canonical form, not merely that
//! normalisation survived. A large share are negative: proving M03 does *not*
//! invent parameters, interpret regexes, fold query strings into paths, or
//! default a missing method matters more than proving it handles the easy
//! cases.
//!
//! The other recurring theme is the M04 boundary — nothing here produces or
//! implies a match between two observations.

use cartograph_parser::model::{
    HttpCallObservation, HttpMethodHint, RouteDeclarationStyle, RouteObservation, SourceLanguage,
    Span, TemplatePart, UrlObservation,
};
use cartograph_resolver::{
    CanonicalRoute, Methods, NormalizationNote, NormalizationStatus, ObservationKind, Segment,
    normalize_client_call, normalize_method, normalize_route_declaration,
};

fn span() -> Span {
    Span {
        start_line: 1,
        start_column: 1,
        end_line: 1,
        end_column: 2,
    }
}

/// Canonicalises a declared route path with the given methods.
fn declare(path: &str, methods: &[HttpMethodHint]) -> CanonicalRoute {
    normalize_route_declaration(
        &RouteObservation {
            receiver: None,
            style: RouteDeclarationStyle::VerbDecorator,
            methods: methods.to_vec(),
            path: UrlObservation::Literal {
                value: path.to_owned(),
            },
            handler: Some("handler".into()),
            span: span(),
        },
        "api/routes.py",
        SourceLanguage::Python,
    )
}

/// Canonicalises a URL-conf entry (the Django form, where regex may appear).
fn url_conf(path: &str) -> CanonicalRoute {
    normalize_route_declaration(
        &RouteObservation {
            receiver: None,
            style: RouteDeclarationStyle::UrlConfEntry,
            methods: Vec::new(),
            path: UrlObservation::Literal {
                value: path.to_owned(),
            },
            handler: Some("views.handler".into()),
            span: span(),
        },
        "api/urls.py",
        SourceLanguage::Python,
    )
}

/// Canonicalises a client call's literal URL.
fn call(url: &str, method: Option<HttpMethodHint>) -> CanonicalRoute {
    normalize_client_call(
        &HttpCallObservation {
            callee: "fetch".into(),
            method_hint: method,
            url: UrlObservation::Literal {
                value: url.to_owned(),
            },
            span: span(),
        },
        "web/src/api.ts",
        SourceLanguage::TypeScript,
    )
}

/// Canonicalises a client call whose URL is a template.
fn call_template(parts: Vec<TemplatePart>, method: Option<HttpMethodHint>) -> CanonicalRoute {
    normalize_client_call(
        &HttpCallObservation {
            callee: "fetch".into(),
            method_hint: method,
            url: UrlObservation::Template { parts },
            span: span(),
        },
        "web/src/api.ts",
        SourceLanguage::TypeScript,
    )
}

fn text(value: &str) -> TemplatePart {
    TemplatePart::Text {
        value: value.to_owned(),
    }
}

fn expr(source: &str) -> TemplatePart {
    TemplatePart::Expression {
        source: source.to_owned(),
        span: span(),
    }
}

fn statics(route: &CanonicalRoute) -> Vec<&str> {
    route
        .path
        .segments
        .iter()
        .map(|s| match s {
            Segment::Static { value } => value.as_str(),
            _ => "<variable>",
        })
        .collect()
}

// ── Static paths ────────────────────────────────────────────────────

#[test]
fn a_simple_path_canonicalises_to_its_segments() {
    let route = declare("/orders", &[HttpMethodHint::Get]);
    assert_eq!(statics(&route), ["orders"]);
    assert_eq!(route.status, NormalizationStatus::Exact);
    assert_eq!(route.path.shape(), "/orders");
    assert_eq!(route.to_string(), "GET /orders");
}

#[test]
fn the_root_path_has_no_segments() {
    let route = declare("/", &[]);
    assert!(route.path.segments.is_empty());
    assert_eq!(route.path.shape(), "/");
    assert_eq!(route.path.to_string(), "/");
}

#[test]
fn nested_static_paths_keep_every_segment() {
    let route = declare("/api/v1/orders", &[HttpMethodHint::Post]);
    assert_eq!(statics(&route), ["api", "v1", "orders"]);
    assert_eq!(route.path.shape(), "/api/v1/orders");
}

// ── Parameter dialects ──────────────────────────────────────────────

#[test]
fn colon_style_declares_a_parameter() {
    let route = declare("/orders/:id", &[]);
    assert_eq!(
        route.path.segments[1],
        Segment::Parameter {
            name: Some("id".into()),
            constraint: None
        }
    );
    assert_eq!(route.path.shape(), "/orders/{*}");
}

#[test]
fn curly_style_declares_a_parameter() {
    let route = declare("/orders/{id}", &[]);
    assert_eq!(
        route.path.segments[1],
        Segment::Parameter {
            name: Some("id".into()),
            constraint: None
        }
    );
}

#[test]
fn angle_style_declares_a_parameter() {
    let route = declare("/orders/<id>", &[]);
    assert_eq!(
        route.path.segments[1],
        Segment::Parameter {
            name: Some("id".into()),
            constraint: None
        }
    );
}

#[test]
fn converters_are_preserved_not_interpreted() {
    for (path, name, constraint) in [
        ("/orders/<int:id>", "id", "int"),
        ("/orders/<uuid:id>", "id", "uuid"),
        ("/orders/<slug:slug>", "slug", "slug"),
    ] {
        let route = declare(path, &[]);
        assert_eq!(
            route.path.segments[1],
            Segment::Parameter {
                name: Some(name.into()),
                constraint: Some(constraint.into()),
            },
            "converter lost for {path}"
        );
    }
}

#[test]
fn converter_order_differs_between_dialects_and_both_are_read_correctly() {
    // Flask/Django put the converter first; FastAPI/Starlette put the name first.
    let flask = declare("/orders/<int:id>", &[]);
    let starlette = declare("/orders/{id:int}", &[]);
    assert_eq!(flask.path.segments[1], starlette.path.segments[1]);
}

#[test]
fn every_dialect_produces_the_same_shape() {
    let shapes: Vec<String> = [
        "/orders/:id",
        "/orders/{id}",
        "/orders/<id>",
        "/orders/<int:id>",
        "/orders/<uuid:id>",
    ]
    .iter()
    .map(|p| declare(p, &[]).path.shape())
    .collect();

    assert!(
        shapes.iter().all(|s| s == "/orders/{*}"),
        "dialects disagree on shape: {shapes:?}"
    );
}

#[test]
fn path_converters_are_catch_alls() {
    for path in ["/files/<path:rest>", "/files/{rest:path}"] {
        let route = declare(path, &[]);
        assert_eq!(
            route.path.segments[1],
            Segment::Wildcard {
                name: Some("rest".into())
            },
            "path converter not treated as catch-all in {path}"
        );
    }
    assert_eq!(
        declare("/files/*", &[]).path.segments[1],
        Segment::Wildcard { name: None }
    );
}

#[test]
fn parameter_names_are_preserved_verbatim() {
    let route = declare("/api/v1/orders/{order_id}", &[]);
    assert_eq!(
        route.path.segments[3],
        Segment::Parameter {
            name: Some("order_id".into()),
            constraint: None
        }
    );
}

// ── NEGATIVE: values are never promoted to parameters ────────────────

#[test]
fn a_numeric_segment_stays_literal() {
    let route = declare("/orders/123", &[]);
    assert_eq!(
        route.path.segments[1],
        Segment::Static {
            value: "123".into()
        },
        "a numeric value must never be inferred to be an identifier"
    );
    assert_eq!(route.path.shape(), "/orders/123");
}

#[test]
fn a_word_segment_stays_literal() {
    let route = declare("/users/foo", &[]);
    assert_eq!(statics(&route), ["users", "foo"]);
}

#[test]
fn a_uuid_shaped_segment_stays_literal() {
    let route = declare("/orders/550e8400-e29b-41d4-a716-446655440000", &[]);
    assert!(
        route.path.segments[1].is_static(),
        "a UUID value is still a value"
    );
}

#[test]
fn a_literal_and_a_declared_parameter_do_not_share_a_shape() {
    assert!(
        !declare("/orders/123", &[]).same_shape(&declare("/orders/{id}", &[])),
        "a concrete path and a parameterised route are structurally different; \
         deciding a call to /orders/123 reaches /orders/{{id}} is M04's judgement"
    );
}

#[test]
fn client_urls_do_not_recognise_route_parameter_syntax() {
    // A client fetching a literal "/orders/:id" is requesting that text, not
    // declaring a parameter.
    let route = call("/orders/:id", Some(HttpMethodHint::Get));
    assert_eq!(
        route.path.segments[1],
        Segment::Static {
            value: ":id".into()
        },
        "client URLs carry values, not declarations"
    );
}

// ── Methods ─────────────────────────────────────────────────────────

#[test]
fn method_names_normalise_case_insensitively() {
    for name in ["get", "GET", "Get", "gEt"] {
        assert_eq!(normalize_method(name), Some(HttpMethodHint::Get), "{name}");
    }
    assert_eq!(normalize_method("post"), Some(HttpMethodHint::Post));
}

#[test]
fn an_unrecognised_method_name_is_not_defaulted() {
    assert_eq!(normalize_method("TRACE"), None);
    assert_eq!(normalize_method(""), None);
    assert_eq!(normalize_method("banana"), None);
}

#[test]
fn a_missing_method_stays_unknown() {
    let route = declare("/orders", &[]);
    assert_eq!(
        route.methods,
        Methods::Unknown,
        "an undeclared method must never become GET"
    );
    assert!(!route.methods.is_known());
    assert_eq!(route.to_string(), "UNKNOWN /orders");
}

#[test]
fn declared_methods_are_deduplicated_and_ordered_deterministically() {
    let a = declare(
        "/orders",
        &[
            HttpMethodHint::Post,
            HttpMethodHint::Get,
            HttpMethodHint::Post,
        ],
    );
    let b = declare("/orders", &[HttpMethodHint::Get, HttpMethodHint::Post]);
    assert_eq!(
        a.methods, b.methods,
        "method order must not affect the canonical form"
    );
    assert_eq!(
        a.methods,
        Methods::Declared(vec![HttpMethodHint::Get, HttpMethodHint::Post])
    );
}

#[test]
fn method_compatibility_is_structural_and_unknown_excludes_nothing() {
    let get = declare("/orders", &[HttpMethodHint::Get]);
    let post = declare("/orders", &[HttpMethodHint::Post]);
    let both = declare("/orders", &[HttpMethodHint::Get, HttpMethodHint::Post]);
    let unknown = declare("/orders", &[]);

    assert!(!get.compatible_method(&post));
    assert!(get.compatible_method(&both));
    assert!(
        get.compatible_method(&unknown) && unknown.compatible_method(&post),
        "an undeclared method rules nothing out"
    );
}

// ── Trailing slashes ────────────────────────────────────────────────

#[test]
fn trailing_slash_is_preserved_not_collapsed() {
    let without = declare("/orders", &[]);
    let with = declare("/orders/", &[]);

    assert!(!without.path.trailing_slash);
    assert!(with.path.trailing_slash);
    assert_eq!(without.path.shape(), "/orders");
    assert_eq!(with.path.shape(), "/orders/");
    assert_ne!(
        without.path, with.path,
        "frameworks disagree about whether these are one route; M03 must not decide"
    );
}

#[test]
fn trailing_slash_does_not_add_a_segment() {
    assert_eq!(declare("/orders/", &[]).path.arity(), 1);
    assert_eq!(statics(&declare("/orders/", &[])), ["orders"]);
}

#[test]
fn the_root_path_is_not_treated_as_having_a_trailing_slash() {
    assert!(!declare("/", &[]).path.trailing_slash);
}

// ── URL decomposition ───────────────────────────────────────────────

#[test]
fn absolute_urls_are_decomposed() {
    let route = call(
        "https://example.com/api/orders/123",
        Some(HttpMethodHint::Get),
    );
    assert_eq!(route.path.scheme.as_deref(), Some("https"));
    assert_eq!(route.path.authority.as_deref(), Some("example.com"));
    assert_eq!(statics(&route), ["api", "orders", "123"]);
    assert!(route.notes.contains(&NormalizationNote::AuthoritySeparated));
}

#[test]
fn a_port_stays_with_the_authority() {
    let route = call("http://localhost:8080/orders", None);
    assert_eq!(route.path.authority.as_deref(), Some("localhost:8080"));
    assert_eq!(statics(&route), ["orders"]);
}

#[test]
fn scheme_is_lowercased_but_authority_is_not_rewritten() {
    let route = call("HTTPS://Example.COM/orders", None);
    assert_eq!(route.path.scheme.as_deref(), Some("https"));
    assert_eq!(route.path.authority.as_deref(), Some("Example.COM"));
}

// ── NEGATIVE: query and fragment never become path structure ─────────

#[test]
fn a_query_string_is_separated_from_the_path() {
    let route = call("/orders?id=123", Some(HttpMethodHint::Get));
    assert_eq!(
        statics(&route),
        ["orders"],
        "query must not become a segment"
    );
    assert_eq!(route.path.query.as_deref(), Some("id=123"));
    assert_eq!(route.path.shape(), "/orders");
    assert!(route.notes.contains(&NormalizationNote::QuerySeparated));
}

#[test]
fn query_parameter_names_never_become_path_parameters() {
    let route = call("/orders?order_id=123&expand=items", None);
    assert_eq!(route.path.arity(), 1);
    assert!(
        route.path.segments.iter().all(Segment::is_static),
        "a query parameter is not a path parameter"
    );
}

#[test]
fn a_fragment_is_separated_from_the_path() {
    let route = call("/orders#section", None);
    assert_eq!(statics(&route), ["orders"]);
    assert_eq!(route.path.fragment.as_deref(), Some("section"));
    assert!(route.notes.contains(&NormalizationNote::FragmentSeparated));
}

#[test]
fn a_fragment_containing_a_question_mark_is_not_read_as_a_query() {
    let route = call("/orders#a?b", None);
    assert_eq!(route.path.fragment.as_deref(), Some("a?b"));
    assert_eq!(route.path.query, None);
}

#[test]
fn query_and_fragment_together_are_both_separated() {
    let route = call("/orders?id=1#top", None);
    assert_eq!(statics(&route), ["orders"]);
    assert_eq!(route.path.query.as_deref(), Some("id=1"));
    assert_eq!(route.path.fragment.as_deref(), Some("top"));
}

// ── Templates ───────────────────────────────────────────────────────

#[test]
fn a_template_substitution_becomes_a_dynamic_segment_not_a_parameter() {
    let route = call_template(
        vec![text("/orders/"), expr("orderId")],
        Some(HttpMethodHint::Get),
    );

    assert_eq!(statics(&route), ["orders", "<variable>"]);
    assert_eq!(
        route.path.segments[1],
        Segment::Dynamic {
            source: "orderId".into()
        },
        "a substituted value is not a declared parameter"
    );
    assert_eq!(route.status, NormalizationStatus::Partial);
    assert!(
        route
            .notes
            .contains(&NormalizationNote::TemplateSubstitutionUnresolved)
    );
}

#[test]
fn a_leading_substitution_marks_the_prefix_unknown() {
    // `${BASE}/orders/${id}` — the path's own prefix is not known here.
    let route = call_template(
        vec![expr("BASE"), text("/orders/"), expr("id")],
        Some(HttpMethodHint::Post),
    );

    assert!(
        route
            .notes
            .contains(&NormalizationNote::TemplatePrefixUnknown),
        "an unknown prefix must be flagged, not silently dropped"
    );
    assert_eq!(route.status, NormalizationStatus::Partial);
    assert_eq!(
        route.path.segments[0],
        Segment::Dynamic {
            source: "BASE".into()
        }
    );
    assert_eq!(
        route.path.segments[1],
        Segment::Static {
            value: "orders".into()
        }
    );
}

#[test]
fn base_is_never_evaluated_even_when_its_value_appears_elsewhere() {
    let route = call_template(vec![expr("BASE"), text("/orders")], None);
    assert_eq!(
        route.path.segments[0],
        Segment::Dynamic {
            source: "BASE".into()
        },
        "resolving BASE to /api/v1 is M05's constant propagation, not M03's"
    );
    assert_ne!(route.path.shape(), "/api/v1/orders");
}

#[test]
fn a_substitution_glued_to_text_makes_the_whole_segment_unknown() {
    // `/v${major}/orders` — the first segment's value is not known.
    let route = call_template(vec![text("/v"), expr("major"), text("/orders")], None);
    assert!(
        matches!(route.path.segments[0], Segment::Dynamic { .. }),
        "a partially-known segment must not be reported as a known value"
    );
    assert_eq!(
        route.path.segments[1],
        Segment::Static {
            value: "orders".into()
        }
    );
}

#[test]
fn a_template_with_no_substitutions_still_canonicalises() {
    let route = call_template(vec![text("/orders/latest")], None);
    assert_eq!(statics(&route), ["orders", "latest"]);
}

// ── NEGATIVE: regex is never interpreted ────────────────────────────

#[test]
fn regex_url_conf_patterns_are_not_interpreted() {
    let route = url_conf(r"^archive/(?P<year>[0-9]{4})/$");

    assert_eq!(
        route.status,
        NormalizationStatus::Unsupported,
        "a regex must not be read as structured path syntax"
    );
    assert!(
        route
            .notes
            .contains(&NormalizationNote::RegexNotInterpreted)
    );
    assert!(route.path.segments.is_empty(), "no structure is asserted");
    assert_eq!(
        route.provenance.raw_path, r"^archive/(?P<year>[0-9]{4})/$",
        "the raw pattern is retained"
    );
}

#[test]
fn a_plain_url_conf_path_is_still_canonicalised() {
    let route = url_conf("orders/<int:pk>/");
    assert_eq!(route.status, NormalizationStatus::Exact);
    assert_eq!(
        route.path.segments[1],
        Segment::Parameter {
            name: Some("pk".into()),
            constraint: Some("int".into())
        }
    );
    assert!(route.path.trailing_slash);
}

#[test]
fn an_unsupported_route_shares_a_shape_with_nothing() {
    let regex = url_conf(r"^orders/(\d+)$");
    let plain = declare("/orders/{id}", &[]);
    assert!(
        !regex.same_shape(&plain) && !regex.same_shape(&regex),
        "there is no structure to compare, so no shape can be claimed"
    );
}

// ── NEGATIVE: non-path observations ─────────────────────────────────

#[test]
fn a_dynamic_url_is_not_interpreted() {
    let route = normalize_client_call(
        &HttpCallObservation {
            callee: "requests.post".into(),
            method_hint: Some(HttpMethodHint::Post),
            url: UrlObservation::Dynamic {
                source: "url".into(),
            },
            span: span(),
        },
        "api/client.py",
        SourceLanguage::Python,
    );

    assert_eq!(route.status, NormalizationStatus::Unsupported);
    assert!(
        route
            .notes
            .contains(&NormalizationNote::DynamicPathNotInterpreted)
    );
    assert_eq!(route.provenance.raw_path, "url");
    assert_eq!(
        route.methods,
        Methods::Declared(vec![HttpMethodHint::Post]),
        "an unknown path does not erase a known method"
    );
}

#[test]
fn a_non_path_string_is_canonicalised_literally_rather_than_rejected() {
    // `config.get("timeout")` never reaches here — the parser already excludes
    // it. If some observation does carry a non-path string, it must not be
    // reshaped into a plausible-looking route.
    let route = call("timeout", None);
    assert_eq!(statics(&route), ["timeout"]);
    assert!(route.path.segments.iter().all(Segment::is_static));
}

// ── Malformed and edge-case paths ───────────────────────────────────

#[test]
fn repeated_slashes_are_dropped_and_recorded() {
    let route = declare("/orders//items", &[]);
    assert_eq!(statics(&route), ["orders", "items"]);
    assert!(
        route
            .notes
            .contains(&NormalizationNote::EmptySegmentsDropped),
        "collapsing must be recorded rather than hidden"
    );
}

#[test]
fn a_path_without_a_leading_slash_canonicalises_the_same_way() {
    assert_eq!(
        declare("orders/items", &[]).path.segments,
        declare("/orders/items", &[]).path.segments,
        "Django URL conf paths are written without a leading slash"
    );
}

#[test]
fn an_empty_path_produces_no_segments() {
    let route = declare("", &[]);
    assert!(route.path.segments.is_empty());
    assert!(!route.path.trailing_slash);
}

#[test]
fn percent_encoding_is_preserved_rather_than_decoded() {
    let route = call("/orders/order%2F123", None);
    assert_eq!(
        route.path.segments[1],
        Segment::Static {
            value: "order%2F123".into()
        },
        "decoding %2F would invent a segment boundary the source never had"
    );
    assert_eq!(route.path.arity(), 2);
}

#[test]
fn a_space_encoded_segment_keeps_its_encoding() {
    let route = call("/search/hello%20world", None);
    assert_eq!(statics(&route), ["search", "hello%20world"]);
}

#[test]
fn an_unclosed_brace_is_literal_text_not_a_parameter() {
    let route = declare("/orders/{id", &[]);
    assert_eq!(
        route.path.segments[1],
        Segment::Static {
            value: "{id".into()
        },
        "malformed parameter syntax is not a parameter"
    );
}

#[test]
fn a_bare_colon_is_not_a_parameter() {
    let route = declare("/orders/:", &[]);
    assert_eq!(
        route.path.segments[1],
        Segment::Static { value: ":".into() }
    );
}

// ── Provenance ──────────────────────────────────────────────────────

#[test]
fn provenance_retains_the_source_and_the_raw_path() {
    let route = declare("/orders/{id}", &[HttpMethodHint::Get]);
    assert_eq!(route.provenance.kind, ObservationKind::RouteDeclaration);
    assert_eq!(route.provenance.language, SourceLanguage::Python);
    assert_eq!(route.provenance.file, "api/routes.py");
    assert_eq!(route.provenance.raw_path, "/orders/{id}");
    assert_eq!(route.provenance.handler.as_deref(), Some("handler"));
}

#[test]
fn client_calls_are_distinguishable_from_declarations() {
    assert_eq!(
        call("/orders", None).provenance.kind,
        ObservationKind::ClientCall
    );
    assert_eq!(call("/orders", None).provenance.handler, None);
}

#[test]
fn a_template_raw_path_is_reconstructed_readably() {
    let route = call_template(vec![expr("BASE"), text("/orders")], None);
    assert_eq!(route.provenance.raw_path, "${BASE}/orders");
}

// ── Structural comparison is not matching ───────────────────────────

#[test]
fn same_shape_ignores_parameter_names_and_dialects() {
    assert!(declare("/orders/{id}", &[]).same_shape(&declare("/orders/:orderId", &[])));
    assert!(declare("/orders/<int:id>", &[]).same_shape(&declare("/orders/{id}", &[])));
}

#[test]
fn same_shape_requires_equal_static_text_and_arity() {
    assert!(!declare("/orders/{id}", &[]).same_shape(&declare("/invoices/{id}", &[])));
    assert!(!declare("/orders/{id}", &[]).same_shape(&declare("/orders", &[])));
    assert!(!declare("/orders/{id}", &[]).same_shape(&declare("/orders/{id}/items", &[])));
}

#[test]
fn a_shared_shape_is_not_a_claim_that_two_routes_are_the_same_endpoint() {
    // Two unrelated services can declare structurally identical routes. M03
    // reports the structural fact and nothing more; deciding these are the
    // same endpoint requires evidence M03 does not have and does not seek.
    let billing = normalize_route_declaration(
        &RouteObservation {
            receiver: None,
            style: RouteDeclarationStyle::VerbDecorator,
            methods: vec![HttpMethodHint::Get],
            path: UrlObservation::Literal {
                value: "/orders/{id}".into(),
            },
            handler: Some("billing_get".into()),
            span: span(),
        },
        "billing/routes.py",
        SourceLanguage::Python,
    );
    let shipping = normalize_route_declaration(
        &RouteObservation {
            receiver: None,
            style: RouteDeclarationStyle::VerbDecorator,
            methods: vec![HttpMethodHint::Get],
            path: UrlObservation::Literal {
                value: "/orders/{id}".into(),
            },
            handler: Some("shipping_get".into()),
            span: span(),
        },
        "shipping/routes.py",
        SourceLanguage::Python,
    );

    assert!(billing.same_shape(&shipping));
    assert_ne!(
        billing.provenance, shipping.provenance,
        "identical shapes from different files remain distinct observations"
    );
}

#[test]
fn a_client_call_and_a_route_declaration_can_share_a_shape_without_being_joined() {
    let client = call_template(
        vec![text("/api/orders/"), expr("id")],
        Some(HttpMethodHint::Post),
    );
    let route = declare("/api/orders/{id}", &[HttpMethodHint::Post]);

    assert!(client.same_shape(&route));
    assert!(client.compatible_method(&route));
    // Structural compatibility is the *input* to M04, not its output. Nothing
    // in M03 records that these two are connected.
    assert_eq!(client.provenance.kind, ObservationKind::ClientCall);
    assert_eq!(route.provenance.kind, ObservationKind::RouteDeclaration);
}

// ── Determinism and serialisation ───────────────────────────────────

#[test]
fn canonicalisation_is_deterministic() {
    for path in [
        "/orders/{id}",
        "/orders/",
        "https://x.test/a/b?q=1#f",
        "/a//b",
    ] {
        assert_eq!(
            declare(path, &[HttpMethodHint::Get]),
            declare(path, &[HttpMethodHint::Get])
        );
    }
}

#[test]
fn canonicalising_an_already_canonical_shape_is_stable() {
    // Feeding a canonical rendering back in yields the same structure, so a
    // pipeline cannot drift by normalising twice.
    let once = declare("/orders/{id}/items", &[]);
    let twice = declare(&once.path.to_string(), &[]);
    assert_eq!(once.path.segments, twice.path.segments);
}

#[test]
fn canonical_routes_round_trip_through_json() {
    for route in [
        declare(
            "/orders/<int:id>/",
            &[HttpMethodHint::Get, HttpMethodHint::Post],
        ),
        call("https://example.com/api?x=1#f", None),
        url_conf(r"^archive/(\d{4})$"),
        call_template(
            vec![expr("BASE"), text("/orders")],
            Some(HttpMethodHint::Post),
        ),
    ] {
        let json = serde_json::to_string(&route).unwrap();
        let back: CanonicalRoute = serde_json::from_str(&json).unwrap();
        assert_eq!(route, back);
    }
}
