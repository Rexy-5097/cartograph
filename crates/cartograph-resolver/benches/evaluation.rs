//! Dynamic URL evaluation throughput.
//!
//! Deterministic in-memory inputs. Internal numbers only; nothing published,
//! and none of it an accuracy claim.

#![allow(missing_docs)] // criterion_group! expands to undocumented items

use std::fmt::Write as _;

use cartograph_parser::Analyzer;
use cartograph_parser::model::FileAnalysis;
use cartograph_resolver::{Scope, collect_exported_constants, evaluate_expression, scope_for_file};
use criterion::{Criterion, Throughput, criterion_group, criterion_main};

fn analyze(path: &str, src: &str) -> FileAnalysis {
    Analyzer::new()
        .expect("grammars")
        .analyze_source(path, src)
        .expect("parses")
}

fn scope_of(src: &str) -> (FileAnalysis, Scope) {
    let f = analyze("web/src/api.ts", src);
    let s = Scope::from_file(&f.symbols, &f.strings);
    (f, s)
}

fn expressions(c: &mut Criterion) {
    let (_, scope) = scope_of("const BASE = \"/api/v1\";\nconst ROOT = `${BASE}/v2`;");
    let mut group = c.benchmark_group("evaluation/expression");
    for (name, expr) in [
        ("literal_constant", "BASE"),
        ("transitive_constant", "ROOT"),
        ("unknown_identifier", "orderId"),
        ("environment_read", "process.env.API_URL"),
        ("unsupported_call", "buildUrl()"),
    ] {
        group.bench_function(name, |b| b.iter(|| evaluate_expression(expr, &scope)));
    }
    group.finish();
}

fn scope_building(c: &mut Criterion) {
    // A module with many constants, which is what `Scope::from_file` walks.
    let mut src = String::new();
    for i in 0..200 {
        let _ = writeln!(src, "const C{i} = \"/segment{i}\";");
    }

    let file = analyze("web/src/config.ts", &src);

    let mut group = c.benchmark_group("evaluation/scope");
    group.throughput(Throughput::Elements(200));
    group.bench_function("from_file_200_constants", |b| {
        b.iter(|| Scope::from_file(&file.symbols, &file.strings));
    });
    group.finish();
}

fn multi_file(c: &mut Criterion) {
    // A realistic project: one config module, many importers.
    let mut files = vec![analyze(
        "web/src/config.ts",
        "export const API_BASE = \"/api/v1\";",
    )];
    for i in 0..100 {
        files.push(analyze(
            &format!("web/src/mod{i}.ts"),
            "import { API_BASE } from \"./config\";\nawait axios.post(`${API_BASE}/orders/${id}`, {});",
        ));
    }

    let mut group = c.benchmark_group("evaluation/project");
    group.throughput(Throughput::Elements(files.len() as u64));
    group.bench_function("100_importers", |b| {
        b.iter(|| {
            let exported = collect_exported_constants(&files);
            for f in &files {
                let _ = scope_for_file(f, &exported);
            }
        });
    });
    group.finish();
}

criterion_group!(benches, expressions, scope_building, multi_file);
criterion_main!(benches);
