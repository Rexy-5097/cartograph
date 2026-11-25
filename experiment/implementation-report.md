# Implementation report

**Branch:** `experiment/m18-browser-explorer`, from `upstream/main` `9ee0dea`, which includes PR #111. Nothing is on `main`, nothing has been pushed, and no PR has been opened.

| Commit | Content |
|---|---|
| `fc491cc` | `cartograph-desktop`: `report`, `trace`, `source` modules; `session::analyze_report_as`; tests |
| `1272969` | `cartograph-serve` (loopback server); `inspect` module |
| `ddb368e` | Browser front end reusing the desktop renderer; `GraphView` `view` prop |
| `87b7446` | Refinements (legend colours from the renderer, test-code filter, focus, camera framing, trace explanations); verification scripts |
| *(next commit)* | Deliverables; proposed ADR-0024; UI fixes from the live run; fixes from two independent reviews |

## Changes by area

**`crates/cartograph-desktop`** (workspace member; additive, read-only; 5 new tests in `tests/browser.rs`)
- `report.rs`: `compose(analysis, &Analysis) -> AnalysisReport`, built from the pipeline's `Totals`, `results`, `orm` and `diagnostics`, plus node locations.
- `trace.rs`: `walk` over `cartograph_graph::trace::trace`, with the same stale and unknown guards as `blast`.
- `inspect.rs`: `node`, a node's in and out neighbours.
- `source.rs`: `excerpt`, cited-file-only and canonicalised under the root, bounded to 61 lines and 2 MB.
- `session.rs`: `analyze_report_as`. `analyze_as` now calls it and drops the report, so its behaviour is unchanged; a test asserts this.
- `Cargo.toml`: depends on `cartograph-resolver` (already in the workspace) for `MatchStatus`. One line changes in the workspace `Cargo.lock`, and the same line in `desktop/src-tauri/Cargo.lock`, written by `cargo check` there; without it, a `--locked` Tauri build would fail.

**`experiment/browser-explorer/server`** (excluded package `cartograph-serve`; about 700 lines; 4 unit tests)
- The HTTP server, static files and security checks described in `api-contract.md`.
- Its own `Cargo.lock`; builds with `--offline`.

**`desktop/`**
- `src/GraphView.tsx`: optional `view?: GraphViewOptions` (hidden node/edge kinds and clusters, highlight, selected node/edge, colour override, per-node style) through Sigma reducers, and an optional `onSigma`. When `view` is absent the effect is skipped; the desktop never passes it.
- `src/browser/`:
  - `api.ts`: transport, token handling, types;
  - `model.ts`: presentation derivations, 9 tests in `model.test.ts`;
  - `BrowserApp.tsx`: the shell and views;
  - `SourcePreview.tsx`;
  - `browser.css`;
  - `main.tsx`.
- `browser.html`, `vite.browser.config.ts`, and the `build:browser` script. `tsconfig` includes the new config. `dist-browser/` is git-ignored.

**`experiment/browser-explorer/verify/`**
- `verify_api.py`: the browser API against `cartograph --json` (counts, identity of every node and edge, sampled evidence, stale ids, invalid requests, latency).
- `probe_fixture.py`: the empty, partial and missing repository cases, and shutdown.

**Not changed:** `App.tsx`, `src-tauri`, the CLI, MCP, the parser, the resolver, the graph and the pipeline. `ARCHITECTURE.md` is also untouched.

## Reproduction

The commands use the external experiment folder `R=D:/WorkSpace/Projects/cartograph-reviews/2026-11-25-m18-browser-explorer`.

```powershell
# 1. Build the front end (desktop/)
npm ci
npm run build:browser
```

```powershell
# 2. Build the server (experiment/browser-explorer/server)
cargo build --release --offline
```

```powershell
# 3. Serve the pinned Flagsmith clone and open the default browser (run inside the clone)
<path-to>/cartograph-serve.exe --open --port 47820 --assets <repo>/desktop/dist-browser .
```

```powershell
# 4. Compare the browser API with the CLI (token from the printed URL)
cartograph . --json > summary.json
python -B experiment/browser-explorer/verify/verify_api.py http://127.0.0.1:47820 <token> summary.json
```

```powershell
# 5. Edge cases
python -B experiment/browser-explorer/verify/probe_fixture.py <cartograph-serve.exe> <dist-browser> <empty-or-partial-dir> 47831
```

The binaries used are in `$R/builds/` with `SHA256SUMS`:
- `cartograph.exe` at `9ee0dea`: `496abda6…27a7`
- `cartograph-serve.exe`: `dd180976…e097` (the final build; earlier builds in the session are superseded)

Raw output is in `$R/output/` and screenshots in `$R/screenshots/`.

## How the work was done

- **Slices.** Each slice was built and tested before the next: the backend functions with Rust tests, then the server with a fixture smoke test, then the front end with vitest, the build and the live Flagsmith run.
- **UI defects found in the live run and fixed:**
  - legend swatches did not match the renderer's edge colours;
  - an "ORM-access edges 870" label actually counted access plus queries;
  - the display name was "." when run inside the repository;
  - `/` served nothing because the build emits `browser.html`.
- **A defect found while writing this report and fixed:** the verification script silently skipped 3 counts (routes, HTTP observations, tables) because the CLI names them differently. The mapping is now explicit, and a missing field fails.
