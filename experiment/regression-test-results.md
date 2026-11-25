# Regression and integration test results

**Branch:** `experiment/m18-browser-explorer`.

**Raw logs:** `$R/output/` in the external experiment folder:
- `regression.txt`
- `verify-api.txt`
- `probe-edge-cases.txt`
- `tauri-check.txt`
- `reanalysis-timing.txt`

## Gates (AgentOS lifecycle step 5)

| Check | Result |
|---|---|
| `cargo fmt --all -- --check` | PASS |
| `cargo clippy --workspace --all-targets --all-features -- -D warnings` | PASS |
| `cargo test --workspace` | **1,379 passed, 0 failed, 32 ignored**, 76 suites. Includes 5 new tests in `crates/cartograph-desktop/tests/browser.rs`; the existing CLI, MCP, desktop and core suites are unchanged and pass. |
| `python agentos/tools/scripts/validate_agentos.py` | **98/100, PASS**, with exactly the 3 baseline warnings: `productivity_metrics.csv`, `.pre-commit-config.yaml` and `.devcontainer/devcontainer.json`. Its distribution-readiness sub-report shows 90/100 FAIL for the same two missing tooling files, as at baseline. |
| `python agentos/tools/scripts/run_gates.py` | **PASS, 9/9** (QG-001 … QG-009) |
| Server package: `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings -W clippy::pedantic`, `cargo test --offline` | PASS; 4 tests |
| Desktop: `npx vitest run` | **112 passed**: 102 existing + 10 new in `src/browser/model.test.ts` |
| Desktop: `npx tsc --noEmit` | clean |
| Desktop production build: `npm run build` (Tauri front end, `dist/`) | builds; 374 KiB JS, as before |
| Browser production build: `npm run build:browser` (`dist-browser/`) | builds; 398 KiB JS |
| Tauri shell: `cargo check --offline` in `desktop/src-tauri` | PASS (1m 08s). Its `cartograph-desktop` dependency now carries the additive functions. |

## What each client was checked for

- **CLI.** Its crate is unchanged, and all CLI tests pass, including the JSON schema tests and the test that forbids a `serve` subcommand. The engine binary used for the experiment (`9ee0dea`) produced the reference output.
- **MCP.** Its crate is unchanged; all MCP session and tool tests pass.
- **Desktop (Tauri).**
  - `App.tsx` and `src-tauri` are unchanged.
  - `GraphView` gained an optional prop, and its effect is skipped when the prop is absent.
  - The 102 desktop tests pass, and both the front-end build and the shell compile.
  - `session::analyze_as` returns the same payload as before (`analyze_as_is_unchanged_by_the_report`).
  - **Not done:** launching the Tauri window and clicking through it by hand.
- **Architecture contracts.**
  - ADR-0017: the scene is unchanged. Node locations travel in the separate report, and the scene test `the_scene_carries_no_analysis_payload` still passes.
  - ADR-0019: trace uses `cartograph_graph::trace`, not a client copy.
  - ADR-0021 C3: the CLI and MCP dependency graphs are unchanged.

## End-to-end on Flagsmith (`verify_api.py`, against the live server)

Every line PASS: **0 failures**.

- **All 21 counts equal `cartograph --json`:** files, nodes, edges, scene nodes and edges, summary nodes and edges, routes, HTTP observations, ORM models, tables, symbols, ambiguous, no-match, unsupported, HTTP-call edges, client-call edges, ORM edges, cross-language edges, diagnostics, failed files.
  - *Found and fixed while writing this report:* the first version of the script silently skipped routes, HTTP observations and tables because the CLI names those fields differently. The mapping is now explicit, and a missing field fails.
- **Every one of 1,092 drawn edges** is a CLI edge with the same id, endpoints and kind, and **no CLI edge is missing**. The graph never shows a relationship absent from the Rust graph.
- **Every one of 1,004 drawn nodes** is a CLI node with the same id, kind and name, and the node locations equal the CLI's.
- **Evidence:** 61 edges sampled across the id range. Evidence, provenance, confidence, file and line equal the CLI's.
- **Stale:** a lookup with an earlier analysis id → 409 `staleSelection`.
- **Invalid requests**, each answered with the expected status:

  | Request | Status |
  |---|---|
  | missing parameters | 400 |
  | unknown edge | 404 |
  | unknown node | 404 |
  | wrong method | 405 |
  | unknown endpoint | 404 |
  | no token | 401 |
  | foreign Host | 421 |
  | foreign Origin | 403 |
  | `../../secret.txt` | 404 |
  | uncited `README.md` | 404 |
  | cited file | 200 |
  | static `/../Cargo.toml` | 404 |

## The brief's 14-step workflow (live, in the browser)

| # | Step | Result |
|---|---|---|
| 1–2 | Select and analyse | Flagsmith at `899aa38` → analysis #1, 1,004 nodes |
| 3–4 | Start the server, open the browser automatically | `--open` launched the default browser, which connected within 8 s (fixture run). The Flagsmith session used the printed URL in the built-in pane. |
| 5 | Render the real graph | 1,004 nodes / 1,092 edges (screenshot 02) |
| 6 | Select a real node | `FFAdminUser`: 1 outgoing, 124 incoming |
| 7–8 | Select a real edge, open its evidence | an ORM edge, with evidence and "assumes LifecycleModel (outside the project)" (screenshot 03); source preview at the cited line |
| 9 | Search a real function | `test_get_feature_code_references…` found and selected, by mouse and by keyboard |
| 10 | TRACE | 3 paths, identical to `cartograph trace` (screenshot 04) |
| 11 | BLAST | 124 dependants at depth 1, identical to `cartograph blast` (screenshot 05) |
| 12 | Unresolved | 4 ambiguous, 384 no-match, 307 unsupported, grouped (screenshot 06) |
| 13–14 | Refresh, the UI updates | Analysis #2 ready; selection, trace, blast and focus cleared. Later, a refresh from *another* client is picked up within 5 s, and a selection refused as stale loads the newer analysis. |

## Edge cases (`probe_fixture.py` and the browser)

| Case | Result |
|---|---|
| Empty repository | status `failed`, `noSupportedSources` with the engine's hint; the path shown as `<absolute>`; `/api/report` → 409 `noAnalysis`. No crash. |
| Partial analysis (one good model and view, one broken `.py`, one broken `.ts`) | ready; 4 files, 2 with diagnostics, 0 failed; 2 nodes, 1 edge; the Diagnostics view lists both (screenshot 08) |
| Missing repository | refused at start: `` `does-not-exist` does not exist. `` |
| Server stopped | later requests are refused (connection refused); the browser shows the banner and now "Server unreachable" |
| Stale selection in the UI | refused, then the newer analysis is loaded with an explanation |
| Phone width (375 px) | no horizontal page scroll |

## Defects found by these tests and fixed on the branch

1. Legend swatch colours differed from the renderer's edge colours.
2. "ORM-access edges 870" was really access + queries.
3. The display name was "." when serving the current directory.
4. `/` served nothing (the build emits `browser.html`).
5. The verification script skipped three counts.
6. After a refresh from elsewhere, the UI was stuck refusing selections as stale.
7. The header said "ready" while the server was down.
8. Keyboard focus was lost after choosing a search result.
9. The inspector overflowed horizontally on long names.

Fixes 6–9 came after the full gate run. They touch only `desktop/src/browser/BrowserApp.tsx` and `browser.css`.

## After the independent review

The review's findings are listed with their disposition in `security-validation.md`. The fixes touch:
- `cartograph-desktop`: `session.rs`, `report.rs` and `lib.rs` docs, and the trace test;
- the server: request parsing, panic handling, token doc;
- the browser front end.

Re-run after them:
- `cargo test -p cartograph-desktop`: all pass, including the recursive trace comparison;
- clippy `-D warnings` (pedantic for the server);
- server tests (4);
- `tsc --noEmit`;
- vitest: **112 passed**, with 1 new test for `frontendRequests`;
- both front-end builds;
- **`verify_api.py` against the rebuilt server: 42 PASS, 0 failures**;
- the parsing re-probes (`review-fix-probes.txt`);
- the blast colours in the browser (screenshot 12).

The final full gate run (fmt, clippy, workspace tests, validator, gates) is recorded in the session log for the commit that carries these fixes.
