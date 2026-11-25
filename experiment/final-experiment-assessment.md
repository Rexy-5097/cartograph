# Final experiment assessment — M18 browser architecture explorer

**Verdict.** The experiment has been executed end to end on a real, randomly drawn repository.

- **It works.** The browser shows Cartograph's actual analysis of Flagsmith, interactively. Every count, node, edge and sampled evidence record is verified equal to the CLI's output.
- **The picture is narrow.** For this repository the engine establishes the backend, its ORM and its test traffic well. It establishes **nothing** between the React frontend and the Django API. The browser shows that honestly; it does not fill the gap.
- **Status.** An experiment on an isolated branch, with a *Proposed* ADR. Nothing is merged, pushed or released.

## Success criteria from the brief

| # | Criterion | Met? | Evidence |
|---|---|---|---|
| 1 | Real, unfamiliar repository chosen by a documented random process | Yes | `repository-selection.md`: seed from the `upstream/main` hash, procedure written before the draw, first unit passed |
| 2 | The current engine analyses it | Yes | 2,755 files, 0 failed, 1,004 nodes, 1,092 edges (`output-analysis.md`) |
| 3 | The actual result appears in a browser page | Yes | screenshots 01–07; `verify_api.py` |
| 4 | The graph is interactive | Yes | pan and zoom, click node or edge, search, filters, focus, camera framing |
| 5 | Nodes and relationships can be inspected | Yes | node detail (124 incoming for `FFAdminUser`), edge evidence, source preview |
| 6 | Every selected edge shows its actual evidence | Yes | `EvidencePanel` over `/api/evidence`; 61 sampled records equal the CLI's |
| 7 | TRACE works visually | Yes | 3 paths on the map and in the panel, identical to `cartograph trace` |
| 8 | BLAST works visually | Yes, after the review fix | 124 dependants listed, identical to `cartograph blast`; highlighted on the map once the review's colour defect was fixed (screenshot 12) |
| 9 | Unresolved relationships are understandable | Yes | 695 refusals grouped by status and reason in plain language, with candidates and locations |
| 10 | Displayed data matches the Rust output | Yes | all 21 counts, all 1,004 nodes, all 1,092 edges, 61 evidence records, 0 failures |
| 11 | Desktop and CLI remain intact | Yes, by tests and builds | 1,379 workspace tests, 111 desktop tests, both front-end builds, Tauri `cargo check`; `App.tsx`, CLI and MCP unchanged. The Tauri window was not clicked through by hand. |
| 12 | Local-first privacy | Yes, with recorded caveats | `security-validation.md`; loopback, token, Host/Origin, cited-source only, no paths, no egress code. The binary still links `ureq` (ADR-0024 F). |
| 13 | Usable on a realistically sized codebase | Partly | Usable with "hide test code", layer filters and focus. The unfiltered 1,004-node map is a hairball, and labels overlap when zoomed. No browser frame-rate measurement (pane hidden). |
| 14 | Passes regression and integration tests | Yes | fmt, clippy, workspace tests, validator 98/100 PASS (baseline), gates 9/9, server tests, vitest, builds, `verify_api.py` 0 failures |

## What the experiment showed

1. **The existing renderer is reusable.**
   - Nearly the whole desktop front end ran in a plain browser without change: `GraphView`, `EvidencePanel`, the scene builder, blast, search and the Sigma settings.
   - `GraphView` needed only an optional prop.
   - Tauri was confined to `App.tsx`, which was left alone.
2. **No second analysis engine was needed.**
   - Four small read-only functions in `cartograph-desktop` (report, trace, inspect, source) exposed what the desktop was dropping, chiefly the match decisions and diagnostics.
   - The TypeScript computes only presentation, labelled as such.
3. **A loopback server can be kept narrow.** One repository from argv, a per-launch token, Host/Origin checks, and cited-source only. It is about 700 lines on the standard library, with no new crate.
4. **The output, not the UI, limits the value.**
   - The most important view for Flagsmith is *Unresolved*: it explains why the frontend is disconnected.
   - The single highest-value engine change for full-stack maps of RTK-Query apps is recognising `createApi` / `builder.query`.
   - Next after that are DRF router routes and a test-code flag.

## Defects found and fixed during the experiment

Nine were found by my own testing (`regression-test-results.md`) and sixteen more came from the independent review (`security-validation.md`); all are in the experiment's own code. Four pre-existing desktop issues were found in the audit and are reported, not fixed (`existing-frontend-audit.md`).

## Independent review

A separate agent that did not write the code reviewed the full diff. It found **no critical issue**; the server withstood every Host, Origin, token, traversal and malformed-request probe.

It found one **important** front-end defect: the blast view's affected-node colours were replaced by layer colours, so BLAST did not visibly show which nodes were affected (screenshot 05 shows this). That is now fixed and checked in the browser (screenshot 12). Seven minor findings and ten nits were raised. Sixteen of the 18 are fixed; two are recorded as limitations (source read at preview time, token on the browser command line). The full table is in `security-validation.md`.

**Verification of the fixes.** A second independent agent confirmed most fixes and found five incomplete (edge filters during trace and blast, four late-answer paths, and some wording). Those were finished in a third round and re-tested: vitest 112, both builds, server tests, `verify_api.py` 42/42, raw-socket probes. The third round itself was not independently re-checked. Details are in `security-validation.md`.

## Owner decisions needed

1. Accept, revise or reject **ADR-0024**, including whether this becomes a CLI subcommand (it currently cannot, under ADR-0021 C3), a separate binary, or Tauri views.
2. If it continues: replace the token source, remove `cartograph-ask` from the server's dependency graph, cap connections, and port the new views to Tauri or the reverse.
3. Whether to schedule the engine work in `output-analysis.md` §8 (RTK Query, DRF routers, test flag) as a milestone.
4. Whether to fix the four pre-existing desktop issues.

Merging, pushing and releasing all wait for the owner.
