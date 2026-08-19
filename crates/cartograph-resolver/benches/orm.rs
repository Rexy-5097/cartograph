//! ORM resolution throughput. Deterministic in-memory inputs; internal only.

#![allow(missing_docs)] // criterion_group! expands to undocumented items

use std::fmt::Write as _;

use cartograph_graph::ArchitectureGraph;
use cartograph_parser::Analyzer;
use cartograph_parser::model::FileAnalysis;
use cartograph_resolver::{OrmAnalysis, add_orm_edges};
use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};

fn analyze(path: &str, src: &str) -> FileAnalysis {
    Analyzer::new()
        .expect("grammars")
        .analyze_source(path, src)
        .expect("parses")
}

/// `n` `SQLAlchemy` models, each with an explicit table.
fn models(n: usize) -> String {
    let mut s = String::from(
        "from sqlalchemy.orm import DeclarativeBase\n\nclass Base(DeclarativeBase):\n    pass\n\n",
    );
    for i in 0..n {
        let _ = write!(
            s,
            "\nclass Model{i}(Base):\n    __tablename__ = \"table_{i}\"\n"
        );
    }
    s
}

/// Handlers touching those models.
fn handlers(n: usize) -> String {
    let mut s = String::new();
    for i in 0..n {
        let _ = write!(s, "\ndef handler{i}(p):\n    return Model{i}(**p)\n");
    }
    s
}

fn discovery(c: &mut Criterion) {
    let mut group = c.benchmark_group("orm/discovery");
    for n in [1_usize, 50, 500] {
        let file = analyze("api/models.py", &models(n));
        group.throughput(Throughput::Elements(n as u64));
        group.bench_with_input(BenchmarkId::from_parameter(n), &file, |b, f| {
            b.iter(|| OrmAnalysis::build(std::slice::from_ref(f)));
        });
    }
    group.finish();
}

fn access_analysis(c: &mut Criterion) {
    let files = vec![
        analyze("api/models.py", &models(100)),
        analyze("api/handlers.py", &handlers(100)),
    ];
    let mut group = c.benchmark_group("orm/access");
    group.throughput(Throughput::Elements(100));
    group.bench_function("100_models_100_handlers", |b| {
        b.iter(|| OrmAnalysis::build(&files));
    });
    group.finish();
}

fn edge_building(c: &mut Criterion) {
    let files = vec![
        analyze("api/models.py", &models(100)),
        analyze("api/handlers.py", &handlers(100)),
    ];
    let analysis = OrmAnalysis::build(&files);
    let mut group = c.benchmark_group("orm/edges");
    group.throughput(Throughput::Elements(200));
    group.bench_function("build_graph", |b| {
        b.iter(|| {
            let mut g = ArchitectureGraph::new();
            add_orm_edges(&mut g, &analysis, None).expect("endpoints exist");
        });
    });
    group.finish();
}

fn full_stack(c: &mut Criterion) {
    let files = vec![
        analyze(
            "web/app.tsx",
            "import axios from \"axios\";\nawait axios.post(\"/api/orders\", {});",
        ),
        analyze(
            "api/models.py",
            "from sqlalchemy.orm import DeclarativeBase\nclass Base(DeclarativeBase):\n    pass\nclass Order(Base):\n    __tablename__ = \"orders\"\n",
        ),
        analyze(
            "api/orders.py",
            "from fastapi import FastAPI\napp = FastAPI()\n\n@app.post(\"/api/orders\")\nasync def create_order(p):\n    return Order(**p)\n",
        ),
    ];
    let mut group = c.benchmark_group("orm/full_stack");
    group.bench_function("chain", |b| {
        b.iter(|| {
            let analysis = OrmAnalysis::build(&files);
            let mut g = ArchitectureGraph::new();
            add_orm_edges(&mut g, &analysis, None).expect("endpoints exist");
        });
    });
    group.finish();
}

criterion_group!(
    benches,
    discovery,
    access_analysis,
    edge_building,
    full_stack
);
criterion_main!(benches);
