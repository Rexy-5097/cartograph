# Tech stack — Cartograph

> Layer: project · **Frozen. Not reopened until M9 ships.** Deviations require an ADR.

## Rust core (the product)

| Concern | Choice | Introduced at |
|---|---|---|
| Parsing | tree-sitter | M01 |
| Semantic resolution | async-lsp + lsp-types (tsserver, Pyright) | M04 |
| Graph algorithms | petgraph `StableDiGraph` — never in public API | **M00 ✓** |
| Storage | redb | M09/M10 |
| Git | gix | M07 |
| File watching | notify | M10 |
| Parallelism | rayon | M01+ |
| Async | tokio | M04 |
| CLI | clap (derive) | **M00 ✓** |
| Errors | thiserror (libs) / anyhow (binaries) | **M00 ✓** |
| Logging | tracing (spans; redaction at tracing layer) | **M00 ✓** |
| Serialization | serde + serde_json | **M00 ✓** |
| Benchmarks | criterion | **M00 ✓** |
| Time | time (RFC 3339 edge timestamps) | **M00 ✓** |
| Agent protocol | rmcp (official Rust MCP SDK) | M15 |

## Desktop (from M11)

Tauri v2 · React 19 + TypeScript + Vite · shadcn/ui · Tailwind · Zustand ·
Graphology · Sigma.js v3 (WebGL) · Playwright.

## Deferred deliberately

`salsa` (hand-build invalidation at M10 first) · `wgpu` (no rendering problem
below 50k visible nodes) · Leiden clustering (directory/package boundaries are
the v1 prior).

## Rejected for v1

Neo4j · PostgreSQL · Redis · S3 · Kafka · Kubernetes · vector DBs · RAG ·
LangChain · Docker in local dev · Python in the core · custom GPU renderer ·
mobile · accounts · billing · cloud processing of source.

## Dependency test

Every addition must answer: **what concrete Cartograph problem does this solve
right now?** "We might need it later" is a no. Enforced by QG-006.
