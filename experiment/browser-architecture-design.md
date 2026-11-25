# Browser architecture design

> Experimental. The architectural decision is recorded as
> [ADR-0024](../docs/adr/ADR-0024-browser-client-on-loopback.md), **Proposed**.
> `ARCHITECTURE.md` is unchanged.

## Layering

```
Rust core (parser, resolver, graph: trace, blast, layout)
  └─ cartograph-pipeline::pipeline::run            (unchanged)
      └─ cartograph-desktop                         (Tauri-free; additive functions)
          ├─ session::analyze_report_as  → AnalysisPayload + AnalysisReport + AnalysisSession
          ├─ evidence / blast            (existing)
          ├─ trace::walk, inspect::node, source::excerpt, report::compose   (new, read-only)
          │
          ├─ desktop/src-tauri  (Tauri IPC)  → desktop/src/App.tsx            (unchanged)
          └─ cartograph-serve   (loopback HTTP) → desktop/src/browser/*.tsx    (new)
                                                     └─ GraphView → Sigma     (shared)
```

This is the brief's chain: Rust core → shared pipeline → local browser API → React → Sigma. The server and the Tauri shell are two thin adapters over the same `cartograph-desktop` functions. Neither contains analysis.

## Why a separate binary rather than `cartograph serve`

Three reasons:
- The CLI's own tests forbid a `serve` subcommand.
- The CLI must stay incapable of network egress by its dependency graph (ADR-0021 C3).
- The only crate holding the session, evidence and blast code (`cartograph-desktop`) depends on `cartograph-ask`, and through it on `ureq`.

So the command is `cartograph-serve [--open] [--port N] [REPOSITORY]`, in an excluded package (`experiment/browser-explorer/server`), like `desktop/src-tauri`. `cartograph serve --open` is the brief's *desired* experience. It is recorded as an open decision in the ADR, not implemented against the CLI's rules.

## Server

- **Concurrency.**
  - Standard library only: `TcpListener` on `127.0.0.1`, one thread per connection, a 10 s read timeout, a 16 KiB head limit and a 1 KiB body limit.
  - **No new crate.**
- **State.**
  - One repository, validated with `cartograph_desktop::repository::validate` at start.
  - The newest completed analysis is held as an `Arc` (payload JSON, report JSON, `AnalysisSession`).
  - One running analysis at most; a repeat `POST /api/analyze` returns the running id.
  - The last failure is kept.
  - Ids are issued under the lock, before the analysis runs.
- **Analysis off the request path.** `POST /api/analyze` returns `202 {started}`; the worker thread publishes the result, and the client polls `/api/status`. Every lookup clones the `Arc` and releases the lock, so a long reanalysis never blocks evidence lookups on the current one.
- **Static files.**
  - Served from `desktop/dist-browser` (`npm run build:browser`).
  - `/` serves `browser.html`.
  - A path with `..`, an empty segment or a backslash is refused, and the resolved path must stay inside the canonical assets root.

## Security model

- Loopback bind, a fixed repository, a per-launch token in the URL fragment, the token sent back in a custom header, and Host/Origin checks.
- A CSP with `script-src 'self'`, a cited-file-only source window, and no absolute path in any payload.

The details and test results are in `security-validation.md`.

## Front end

`desktop/browser.html` → `src/browser/main.tsx` → `BrowserApp.tsx`, built by `vite.browser.config.ts` into `dist-browser/`. The desktop build (`vite.config.ts` → `dist/`) is unchanged.

| Area | Implementation | Data source |
|---|---|---|
| Top bar | wordmark + EXPERIMENTAL, repository display name, analysis id and state, refresh, search, zoom/reset, settings (colour by layer or cluster) | `/api/status`, scene |
| Left navigation | overview, full map, frontend, API and routes, backend, database, dependencies, trace, blast, unresolved, diagnostics; each with a real count or "none" | report + scene |
| Canvas | the existing `GraphView` (Sigma), with filters, highlights and selection through node and edge reducers; legend with layer and relationship toggles; "hide test code (by file path)"; neighbourhood focus; camera framing | scene |
| Inspector | `EvidencePanel` for an edge, plus "show source"; node detail with in/out relationships and Trace / Blast / Centre / Focus | `/api/evidence`, `/api/node`, `/api/source` |
| Overview | totals, languages, relationships by kind, a callout when there are no cross-language edges | `/api/report` |
| TRACE | a path list from the core's trace; each stop explained; a class-based-view note; map highlight | `/api/trace` |
| BLAST | `describeBlast` summary and the impacted list by depth; map highlight | `/api/blast` |
| Unresolved | decisions grouped by status and reason, with client file:line and candidates; route nodes; ORM-ambiguous names | report `decisions` |
| Diagnostics | per-file table | report `diagnostics` |
| Source | a bounded excerpt with line numbers, the marked line and lexical colouring | `/api/source` |

**Presentation-only derivations** are labelled as such in the UI:
- **Layer**, from file extension and node kind: `.ts/.tsx` is frontend, `route` is API, `table` is database.
- **Test code**, from a path pattern.
- **The camera fit.**

None of them changes which nodes or edges exist.

**Stale answers.** Each request records the analysis id it was made for. A reply for another analysis is discarded, and a refresh clears the selection, trace, blast and focus.
