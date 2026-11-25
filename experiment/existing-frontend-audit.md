# Existing frontend and data-surface audit

**Scope.** A read-only audit at `upstream/main` `9ee0dea`, which includes PR #111, before any experiment code was written. CLI samples were run on in-repo fixtures only.

The full working notes (677 lines, with JSON samples and line references) are in `D:/WorkSpace/Projects/cartograph-reviews/2026-11-25-m18-browser-explorer/audit/existing-frontend-audit-notes.md`. This file answers the brief's 13 questions and states what the experiment did about each.

## Answers

1. **What data Cartograph generates.**
   - **The graph:** nodes and edges. Each edge carries a kind, an uncalibrated confidence, a provenance, an evidence sentence, a file and a line.
   - **Counts:** totals of observations, resolutions and refusals.
   - **Match decisions:** every client call, with its status and candidates.
   - **The ORM mapping:** per-file diagnostics.
   - **From the graph:** a deterministic layout (`cartograph_graph::layout`), TRACE (`trace::trace`) and BLAST (`blast::blast_radius`).

2. **The JSON output.** Schema `1.2` (`crates/cartograph-cli/src/json.rs`).
   - **Envelope:** `{schema_version, command, repository{name, requested}, languages, summary, nodes[], edges[], diagnostics[], errors[], result?}`.
   - **Node:** `{id, kind, name, file?, line?}`.
   - **Edge:** `{id, source, target, kind, confidence, provenance, evidence, file, line}`.
   - **`match --json`** predates the envelope: `{totals, graph, matches[], orm[], orm_ambiguous[]}`.
   - **Errors** are a JSON envelope on stdout, with exit codes 3–7.

3. **Graph information.**
   - Node kinds (14 declared): in practice `function`, `class`, `table`, `route` and `file`.
   - Edge kinds: `http-call`, `call`, `orm-access` and `queries` are produced. `import`, `inherits`, `implements` and `references` are declared but never produced.

4. **Ids.**
   - `NodeId`/`EdgeId` are bare `u64` and **graph-local**: the same id names different things in two analyses.
   - The desktop guards with `AnalysisId`; a mismatch is refused as `staleSelection`.
   - Graphology keys are `String(id)` and `` `e${id}` ``.

5. **Labels, kinds and positions.**
   - Positions are computed in Rust only (ADR-0015).
   - The *scene* (`cartograph-desktop/src/scene.rs`, ADR-0017) is `{id, x, y, cluster, label, kind}` per node and `{id, source, target, kind}` per edge. It deliberately carries no file, line, confidence or evidence.

6. **Evidence for one edge.** `AnalysisSession::evidence(analysis, edge)` returns an `EvidenceRecord`: endpoints, kind, confidence (`calibrated: false`), provenance, derived/inferred, the evidence sentence, and the location.

7. **TRACE.** `cartograph_graph::trace::trace(graph, from, max_depth)`:
   - outward depth-first search, all edge kinds, default depth 10;
   - stops are `chain-end`, `max-depth` and `cycle`.

   The CLI and MCP render it. **The desktop did not expose it.**

8. **BLAST.** `cartograph_graph::blast::blast_radius`:
   - an inward walk over `call`, `http-call`, `orm-access` and `queries` only;
   - path confidence is the weakest link and node confidence the best path (ADR-0018);
   - no hop limit.

   The desktop exposes it through `blast::radius`.

9. **Refused relationships.**
   - Their status is `ambiguous`, `no-match` or `unsupported`, with an `unsupported_reason`, in `match --json` `matches[]`.
   - Counts appear in every summary.
   - Up to three same-file refusals appear at a broken trace chain.
   - **The desktop dropped them**: `session::analyze_as` kept only the graph.

10. **Diagnostics.**
    - `FileDiagnostic{file, severity, kind, message, line?}`; the message is structural, never source text.
    - Whole-run failures map to `ErrorCode`.
    - Skipped trees (`node_modules`, `migrations` and so on) are not counted anywhere.
    - The desktop shipped only the failed-file count.

11. **What the desktop displays.** The map, coloured by cluster and sized by kind; search; edge evidence; blast highlighting and summary; ASK; counts; largest clusters. **It does not display** trace, refusals, per-file diagnostics, languages or cross-language counts.

12. **Reusable components.** Everything except `App.tsx`:
    - `GraphView`, `EvidencePanel`, `graph.ts` (the scene → Graphology builder), `blast.ts`, `search.ts`, `evidence.ts`, `session.ts` (types), `sigmaSettings.ts` and `styles.css`;
    - none of them imports Tauri.

    `App.tsx` holds all 8 Tauri call sites: 7 `invoke` commands and the native folder dialog.

13. **What is needed without Tauri.**
    - A transport for the seven commands.
    - A replacement for the folder dialog: a browser cannot obtain a filesystem path.
    - Atomic `AnalysisId` allocation.
    - A security model for a local listener, which Tauri IPC does not need.
    - The Tauri command handlers are one-line calls into `cartograph-desktop`, which has no Tauri dependency, so a server can call the same functions.

## Governance constraints found (not code problems)

- The CLI test suite **forbids** a `serve` or `ui` subcommand (`crates/cartograph-cli/src/main.rs`).
- ADR-0020 §C rejected loopback HTTP for MCP. ADR-0005 and `ARCHITECTURE.md` defer any server, and Axum in particular.
- ADR-0021 C3: the CLI and MCP must stay incapable of egress by dependency graph. `cartograph-desktop` pulls in `cartograph-ask`, and through it `ureq` and the keychain stores.
- No ADR covers a browser client, so a proposed ADR is required. It is [ADR-0024](../docs/adr/ADR-0024-browser-client-on-loopback.md), *Proposed*.

## Latent defects found while reading (pre-existing, not fixed here)

1. **`desktop/src-tauri/src/main.rs`.** The next `AnalysisId` is read under the lock, the lock is released, and the analysis runs. Two concurrent analyses would therefore get the same id. In Tauri this is unreachable only because the UI serialises analyses. **The experiment's server allocates under the lock.**
2. **The same file.** The ASK grant is replaced *before* the analysis. If the analysis then fails, the grant and the graph describe different repositories. This is unreachable in the Tauri UI.
3. **`desktop/src/evidence.ts`.** It maps provenance `"open-api-spec"`, but Rust serialises `"openapi-spec"`. No analysis produces it today, but it is TS/Rust contract drift.
4. **`desktop/src/session.ts`.** It keeps the absolute folder path in front-end state. It is not rendered.

These are reported for the owner. Fixing them is outside the experiment's scope.

## What the experiment reused and what it added

**Reused unchanged:**
- `EvidencePanel`
- the scene → Graphology builder and edge colours (`graph.ts`)
- `describeBlast` and `impactedNodeKeys` (`blast.ts`)
- `findNodes` (`search.ts`)
- the Sigma settings
- the session types and `styles.css`

**Extended:**
- `GraphView` gained one optional `view` prop (node and edge reducers for filters, highlight and selection) and an `onSigma` callback.
- Without the prop, its behaviour is unchanged; the desktop does not pass it.

**`App.tsx` was not touched.** The browser has its own shell (`desktop/src/browser/`), so the desktop IPC adapter is preserved as it was.
