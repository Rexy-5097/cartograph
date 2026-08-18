//! TypeScript extraction throughput.
//!
//! Reproducible by construction: every input is generated deterministically
//! in-memory (no filesystem, no network, no machine-specific corpus), so two
//! runs on two machines measure the same work. Numbers are internal — nothing
//! here is published (docs/benchmarks/README.md).

#![allow(missing_docs)] // criterion_group! expands to undocumented items

use cartograph_parser::Analyzer;
use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};

/// A small realistic component: imports, symbols, calls, template, HTTP.
const SMALL_FIXTURE: &str = r#"
import axios from "axios";
import { useState } from "react";

const BASE = "/api/v1";

export async function submitOrder(orderId: string): Promise<void> {
    const url = `${BASE}/orders/${orderId}`;
    await axios.post(url, { paid: true });
}

export class OrderQueue {
    private items: string[] = [];
    push(id: string): void {
        this.items.push(id);
    }
    async flush(): Promise<void> {
        await fetch("/api/v1/orders/flush", { method: "POST" });
    }
}
"#;

/// Deterministic synthetic module of `n` functions with calls and strings.
fn synthetic_module(n: usize) -> String {
    use std::fmt::Write;
    let mut out = String::with_capacity(n * 160);
    out.push_str("import { helper } from \"./helper\";\nconst BASE = \"/api\";\n");
    for i in 0..n {
        let _ = write!(
            out,
            "export async function handler{i}(id: string): Promise<void> {{\n    \
             const url = `${{BASE}}/items/${{id}}`;\n    \
             helper(url, \"label{i}\");\n    \
             await fetch(url, {{ method: \"POST\" }});\n}}\n"
        );
    }
    out
}

fn single_file(c: &mut Criterion) {
    let mut group = c.benchmark_group("ts_extraction/single_file");
    let mut analyzer = Analyzer::new().expect("grammars load");

    group.throughput(Throughput::Bytes(SMALL_FIXTURE.len() as u64));
    group.bench_function("small_fixture", |b| {
        b.iter(|| {
            analyzer
                .analyze_source("bench/small.ts", SMALL_FIXTURE)
                .unwrap()
        });
    });

    for n in [50_usize, 500] {
        let source = synthetic_module(n);
        group.throughput(Throughput::Bytes(source.len() as u64));
        group.bench_with_input(BenchmarkId::new("synthetic_fns", n), &source, |b, src| {
            b.iter(|| analyzer.analyze_source("bench/synthetic.ts", src).unwrap());
        });
    }
    group.finish();
}

fn multi_file(c: &mut Criterion) {
    // A deterministic 100-file project, varied sizes, analysed sequentially -
    // the M01 execution model.
    let files: Vec<(String, String)> = (0..100)
        .map(|i| (format!("bench/module{i}.ts"), synthetic_module(5 + i % 20)))
        .collect();
    let total: usize = files.iter().map(|(_, s)| s.len()).sum();

    let mut group = c.benchmark_group("ts_extraction/multi_file");
    group.throughput(Throughput::Bytes(total as u64));
    group.bench_function("project_100_files", |b| {
        let mut analyzer = Analyzer::new().expect("grammars load");
        b.iter(|| {
            for (path, source) in &files {
                analyzer.analyze_source(path, source).unwrap();
            }
        });
    });
    group.finish();
}

criterion_group!(benches, single_file, multi_file);
criterion_main!(benches);
