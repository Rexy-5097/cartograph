# ADR-0001 — The Rust core is the product; CLI, desktop and MCP are clients

**Status:** Accepted · 2026-08-18 · Enforces RULES 001–002

## Context

Cartograph will eventually have three user-facing surfaces: a CLI, a desktop
application and an MCP server. Each needs analysis, graph queries, diffing and
layout. Implementing any of that per-surface triples the work and lets the
surfaces drift apart.

## Decision

All analysis, graph construction, storage, query, diff and layout logic lives
in one Rust core. The CLI, the Tauri desktop app and the MCP server are thin
clients of the same core graph API and hold no analysis logic. Layout is
computed in Rust — clustering and force layout in the core, the frontend
receiving `{id, x, y, cluster}` and doing nothing but drawing.

## Alternatives

- **Node/TypeScript core** — rejected: parsing 10k files and computing layout
  hits a performance ceiling (~5k nodes in JS layout), and the npm wrapper
  would become the engine rather than a downloader.
- **Per-surface logic** — rejected: three implementations of one product.

## Consequences

Adding a feature means adding it once. The npm package stays a thin binary
wrapper. Any client can be rewritten without touching analysis. The core's API
becomes a stability boundary that must be versioned deliberately from M09.
