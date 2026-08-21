//! Cross-language matching throughput.
//!
//! Reproducible by construction: every route and client observation is
//! generated deterministically in memory. Numbers are internal — nothing here
//! is published, and none of it is an accuracy claim, which belongs to
//! CrossStack-Bench at M08.

#![allow(missing_docs)] // criterion_group! expands to undocumented items

use cartograph_parser::model::{
    HttpCallObservation, HttpMethodHint, RouteDeclarationStyle, RouteObservation, SourceLanguage,
    Span, UrlObservation,
};
use cartograph_resolver::{
    RouteIndex, match_client, normalize_client_call, normalize_route_declaration,
};
use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};

fn span() -> Span {
    Span {
        start_line: 1,
        start_column: 1,
        end_line: 1,
        end_column: 2,
    }
}

fn route(path: &str, method: HttpMethodHint, handler: &str) -> RouteObservation {
    RouteObservation {
        receiver: None,
        style: RouteDeclarationStyle::VerbDecorator,
        methods: vec![method],
        path: UrlObservation::Literal {
            value: path.to_owned(),
        },
        handler: Some(handler.to_owned()),
        span: span(),
    }
}

fn call(url: &str, method: Option<HttpMethodHint>) -> HttpCallObservation {
    HttpCallObservation {
        enclosing: None,
        callee: "axios.post".into(),
        method_hint: method,
        url: UrlObservation::Literal {
            value: url.to_owned(),
        },
        span: span(),
    }
}

/// A deterministic backend of `n` routes across several shapes.
fn backend(n: usize) -> RouteIndex {
    RouteIndex::build((0..n).map(|i| {
        let path = match i % 4 {
            0 => format!("/api/v1/resource{i}"),
            1 => format!("/api/v1/resource{i}/{{id}}"),
            2 => format!("/api/v1/resource{i}/{{id}}/items"),
            _ => format!("/api/v1/resource{i}/files/{{rest:path}}"),
        };
        let method = if i % 2 == 0 {
            HttpMethodHint::Post
        } else {
            HttpMethodHint::Get
        };
        normalize_route_declaration(
            &route(&path, method, &format!("handler{i}")),
            "api/routes.py",
            SourceLanguage::Python,
        )
    }))
}

/// The distinct matching outcomes, so each rule's cost is visible.
fn outcomes(c: &mut Criterion) {
    let index = RouteIndex::build([
        normalize_route_declaration(
            &route("/api/orders", HttpMethodHint::Post, "create"),
            "a.py",
            SourceLanguage::Python,
        ),
        normalize_route_declaration(
            &route("/api/orders/{id}", HttpMethodHint::Post, "replace"),
            "a.py",
            SourceLanguage::Python,
        ),
        normalize_route_declaration(
            &route("/api/files/{rest:path}", HttpMethodHint::Get, "files"),
            "a.py",
            SourceLanguage::Python,
        ),
    ]);

    let cases: Vec<(&str, HttpCallObservation)> = vec![
        (
            "exact_method_and_path",
            call("/api/orders", Some(HttpMethodHint::Post)),
        ),
        (
            "method_mismatch",
            call("/api/orders", Some(HttpMethodHint::Delete)),
        ),
        (
            "static_path_mismatch",
            call("/api/invoices", Some(HttpMethodHint::Post)),
        ),
        (
            "parameter_bound",
            call("/api/orders/123", Some(HttpMethodHint::Post)),
        ),
        (
            "wildcard_suffix",
            call("/api/files/a/b/c", Some(HttpMethodHint::Get)),
        ),
        ("unknown_method", call("/api/orders", None)),
        (
            "no_match",
            call("/nowhere/at/all", Some(HttpMethodHint::Post)),
        ),
    ];

    let mut group = c.benchmark_group("matching/outcome");
    for (name, observation) in &cases {
        let client =
            normalize_client_call(observation, "web/src/api.ts", SourceLanguage::TypeScript);
        group.bench_function(*name, |b| b.iter(|| match_client(&client, &index)));
    }
    group.finish();
}

/// Index construction, separated from matching so each is measurable.
fn indexing(c: &mut Criterion) {
    let mut group = c.benchmark_group("matching/index_build");
    for n in [100_usize, 1_000] {
        group.throughput(Throughput::Elements(n as u64));
        group.bench_with_input(BenchmarkId::from_parameter(n), &n, |b, &n| {
            b.iter(|| backend(n));
        });
    }
    group.finish();
}

/// Matching against a large backend: this is what candidate indexing buys.
fn scaling(c: &mut Criterion) {
    let mut group = c.benchmark_group("matching/against_backend");
    for n in [100_usize, 1_000] {
        let index = backend(n);
        let client = normalize_client_call(
            &call("/api/v1/resource500/42", Some(HttpMethodHint::Get)),
            "web/src/api.ts",
            SourceLanguage::TypeScript,
        );
        group.throughput(Throughput::Elements(1));
        group.bench_with_input(BenchmarkId::new("routes", n), &index, |b, index| {
            b.iter(|| match_client(&client, index));
        });
    }
    group.finish();
}

/// End to end: build the index, then match a batch of clients against it.
fn end_to_end(c: &mut Criterion) {
    let clients: Vec<_> = (0..100)
        .map(|i| {
            normalize_client_call(
                &call(
                    &format!("/api/v1/resource{i}/42"),
                    Some(HttpMethodHint::Get),
                ),
                "web/src/api.ts",
                SourceLanguage::TypeScript,
            )
        })
        .collect();

    let mut group = c.benchmark_group("matching/end_to_end");
    group.throughput(Throughput::Elements(clients.len() as u64));
    group.bench_function("1000_routes_100_clients", |b| {
        b.iter(|| {
            let index = backend(1_000);
            for client in &clients {
                let _ = match_client(client, &index);
            }
        });
    });
    group.finish();
}

criterion_group!(benches, outcomes, indexing, scaling, end_to_end);
criterion_main!(benches);
