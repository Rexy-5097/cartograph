//! Canonicalisation throughput.
//!
//! Reproducible by construction: every input is generated deterministically
//! in-memory. Numbers are internal — nothing here is published
//! (docs/benchmarks/README.md).

#![allow(missing_docs)] // criterion_group! expands to undocumented items

use cartograph_parser::model::{
    HttpMethodHint, RouteDeclarationStyle, RouteObservation, SourceLanguage, Span, TemplatePart,
    UrlObservation,
};
use cartograph_resolver::normalize_route_declaration;
use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};

fn span() -> Span {
    Span {
        start_line: 1,
        start_column: 1,
        end_line: 1,
        end_column: 2,
    }
}

fn declaration(path: &str) -> RouteObservation {
    RouteObservation {
        style: RouteDeclarationStyle::VerbDecorator,
        methods: vec![HttpMethodHint::Get],
        path: UrlObservation::Literal {
            value: path.to_owned(),
        },
        handler: Some("handler".into()),
        span: span(),
    }
}

/// A small set spanning every dialect the normaliser supports.
fn small_fixture() -> Vec<RouteObservation> {
    [
        "/orders",
        "/orders/",
        "/orders/:id",
        "/orders/{id}",
        "/orders/<int:id>",
        "/api/v1/orders/{order_id}/items",
        "https://example.com/api/orders?x=1#f",
        "/files/<path:rest>",
    ]
    .iter()
    .map(|p| declaration(p))
    .collect()
}

/// A deterministic corpus of `n` route declarations, dialects interleaved.
fn synthetic_corpus(n: usize) -> Vec<RouteObservation> {
    (0..n)
        .map(|i| {
            let path = match i % 4 {
                0 => format!("/api/v1/resource{i}/{{id}}"),
                1 => format!("/api/v1/resource{i}/:id/items"),
                2 => format!("/api/v1/resource{i}/<int:id>/"),
                _ => format!("/api/v1/resource{i}/static/segment"),
            };
            declaration(&path)
        })
        .collect()
}

fn canonicalize_all(routes: &[RouteObservation]) {
    for route in routes {
        let _ = normalize_route_declaration(route, "api/routes.py", SourceLanguage::Python);
    }
}

fn literals(c: &mut Criterion) {
    let mut group = c.benchmark_group("canonicalization/literal");

    let small = small_fixture();
    group.throughput(Throughput::Elements(small.len() as u64));
    group.bench_function("small_fixture", |b| b.iter(|| canonicalize_all(&small)));

    for n in [100_usize, 1_000] {
        let corpus = synthetic_corpus(n);
        group.throughput(Throughput::Elements(n as u64));
        group.bench_with_input(BenchmarkId::new("synthetic_routes", n), &corpus, |b, c| {
            b.iter(|| canonicalize_all(c));
        });
    }
    group.finish();
}

fn templates(c: &mut Criterion) {
    // Template paths take the segment-reassembly path through the normaliser.
    let corpus: Vec<RouteObservation> = (0..1_000)
        .map(|i| RouteObservation {
            style: RouteDeclarationStyle::VerbDecorator,
            methods: vec![HttpMethodHint::Post],
            path: UrlObservation::Template {
                parts: vec![
                    TemplatePart::Expression {
                        source: "BASE".into(),
                        span: span(),
                    },
                    TemplatePart::Text {
                        value: format!("/resource{i}/"),
                    },
                    TemplatePart::Expression {
                        source: "id".into(),
                        span: span(),
                    },
                ],
            },
            handler: None,
            span: span(),
        })
        .collect();

    let mut group = c.benchmark_group("canonicalization/template");
    group.throughput(Throughput::Elements(corpus.len() as u64));
    group.bench_function("synthetic_templates_1000", |b| {
        b.iter(|| canonicalize_all(&corpus));
    });
    group.finish();
}

criterion_group!(benches, literals, templates);
criterion_main!(benches);
