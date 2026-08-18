//! Python extraction throughput.
//!
//! Reproducible by construction: every input is generated deterministically
//! in-memory (no filesystem, no network, no machine-specific corpus). Numbers
//! are internal — nothing here is published (docs/benchmarks/README.md).

#![allow(missing_docs)] // criterion_group! expands to undocumented items

use std::fmt::Write;

use cartograph_parser::Analyzer;
use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};

/// A small realistic `FastAPI` module: imports, routes, f-strings, HTTP client.
const SMALL_FIXTURE: &str = r#"
import httpx
from fastapi import FastAPI

from .models import Order

app = FastAPI()
BASE = "/api/v1"


@app.get("/orders")
async def list_orders():
    return await httpx.get(f"{BASE}/upstream/orders")


@app.post("/orders")
async def create_order(payload: dict) -> Order:
    order = Order(**payload)
    order.save()
    return order


class OrderQueue:
    def __init__(self):
        self.items = []

    def push(self, order_id: str) -> None:
        self.items.append(order_id)
"#;

/// Deterministic synthetic module of `n` routed handlers.
fn synthetic_module(n: usize) -> String {
    let mut out = String::with_capacity(n * 200);
    out.push_str(
        "import requests\nfrom fastapi import APIRouter\n\nrouter = APIRouter()\nBASE = \"/api\"\n",
    );
    for i in 0..n {
        let _ = write!(
            out,
            "\n@router.post(\"/items/{i}\")\nasync def handler{i}(item_id: str) -> None:\n    \
             url = f\"{{BASE}}/items/{{item_id}}\"\n    \
             requests.post(url, json={{\"n\": {i}}})\n    \
             helper(url, \"label{i}\")\n"
        );
    }
    out
}

fn single_file(c: &mut Criterion) {
    let mut group = c.benchmark_group("py_extraction/single_file");
    let mut analyzer = Analyzer::new().expect("grammars load");

    group.throughput(Throughput::Bytes(SMALL_FIXTURE.len() as u64));
    group.bench_function("small_fixture", |b| {
        b.iter(|| {
            analyzer
                .analyze_source("bench/small.py", SMALL_FIXTURE)
                .unwrap()
        });
    });

    for n in [50_usize, 500] {
        let source = synthetic_module(n);
        group.throughput(Throughput::Bytes(source.len() as u64));
        group.bench_with_input(
            BenchmarkId::new("synthetic_routes", n),
            &source,
            |b, src| {
                b.iter(|| analyzer.analyze_source("bench/synthetic.py", src).unwrap());
            },
        );
    }
    group.finish();
}

fn multi_file(c: &mut Criterion) {
    let files: Vec<(String, String)> = (0..100)
        .map(|i| (format!("bench/module{i}.py"), synthetic_module(5 + i % 20)))
        .collect();
    let total: usize = files.iter().map(|(_, s)| s.len()).sum();

    let mut group = c.benchmark_group("py_extraction/multi_file");
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
