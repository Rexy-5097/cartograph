# State — Cartograph

> Layer: project · **Update at the end of every session** · Machine-readable
> twin: [../artifacts/project-state.yaml](../artifacts/project-state.yaml)

## Current position

| Field | Value |
|---|---|
| Active milestone | **M07 — Verified full-stack chain** (unlocked, not started) |
| Status | M00–M06 **ACCEPTED**; M07 unlocked, no code written |
| Branch | `main` |
| Next permitted milestone | M07 — **unlocked**; M08 locked until M07 is accepted |
| Last accepted checkpoint | `cartograph-m06` (immutable, commit `ef26ae2`) |
| Spec | Frozen V3, August 2026 |

## What exists

- Rust workspace, six crates. Domain model (evidence/provenance/confidence
  enforced by construction), architecture graph over petgraph, testkit with
  the spec's worked cross-stack chain as fixtures.
- **M06: ORM and database resolution** — SQLAlchemy and Django models to
  tables, handler-to-model access, and the complete cross-stack chain walkable
  end to end. Two defects found by output inspection: a Django `Meta` name
  collision, and a graph identity split (ADR-0011).
- **M05: dynamic URL resolution** — symbolic value model, restricted expression
  evaluation, cross-file constants, partial reconstruction into the unchanged
  M04 matcher. Real repositories: zero additional matches, zero regressions;
  those corpora use runtime-configured SDK clients M05 correctly refuses.
- **M04: cross-language route matching** — the first semantic join and the
  first `HttpCall` edges, with candidate indexing, deterministic compatibility
  rules, a discriminating-evidence requirement, and ambiguity that produces no
  edge. Validated on FastAPI/Flask/DRF; the discriminating rule removed 138
  false edges found in Flask's own repository.
- **M03: canonical route normalisation** — one observation in, one canonical
  form out. Five parameter dialects, URL decomposition, template structure,
  explicit refusal of regexes and dynamic paths. No matching: a `CanonicalRoute`
  cannot reference another observation.
- **M02: Python extractor** (peer of the TypeScript one behind one dispatch):
  imports, symbols with `__all__` handling, decorators, calls, f-strings,
  HTTP observations, and shared `RouteObservation` for verb/route decorators
  and URL-conf entries. Smoke-tested on FastAPI/Flask/DRF (384 files, 0 failed).
- **M01: TypeScript/TSX extractor** (tree-sitter, contained behind a
  language-neutral fact model): imports, symbols (+enclosing/+export), call
  sites, string/template/concatenation structure, HTTP observations,
  error-tolerant diagnostics. CLI `parse` command. Smoke-tested on zustand
  (35 files) and swr (194 files).
- 335 tests passing; fmt/clippy(-D warnings)/check clean; ts_extraction
  criterion group (numbers internal).
- AgentOS v1.0.0 vendored under `agentos/`, validator PASS 99/100.
- Governance: PROJECT_RULES (25 rules), ARCHITECTURE, ROADMAP, CHECKPOINTS,
  SECURITY, CONTRIBUTING, AGENTS, AGENTOS docs; CI; PR/issue templates;
  CODEOWNERS; quality gates QG-001…009; milestone definitions M00…M17;
  9 Cartograph ADRs.

## What does NOT exist (deliberately)

No template evaluation (M05), no ORM (M06), no LSP, no template *evaluation* (M05 — structure is preserved, not
evaluated), no ORM (M06), no storage, no incremental engine, no desktop, no
MCP, no AI. `cartograph-resolver` remains a doc-only crate.

## Open risks

- exFAT working volume: no hard links (cargo falls back to copying), AppleDouble
  `._*` sidecar files; mitigated via .gitignore + `make clean-sidecars`; see
  docs/development/external-storage.md.
- Confidence priors are uncalibrated until M08.
- Node identity is graph-local until M10.

## Session log

| Date | Session outcome |
|---|---|
| 2026-08-19 | M01 accepted (`cartograph-m01`), M02 executed: Python extractor into the shared fact model, route observations, 47 new tests, benchmarks, three smoke corpora. `RouteFramework` corrected to `RouteDeclarationStyle` after Flask's repo disproved the framework label. PR #3 opened; stopped per RULE 025. |
| 2026-08-19 | M00 checkpoint semantics repaired (ADR-0010: provisional rc tags vs accepted tags; premature cartograph-m00 retired, rc1 created at exact PR head). M01 executed: tree-sitter TS/TSX extractor, fact model, diagnostics, CLI `parse`, 22-category fixture corpus, 82 tests, criterion group, two real-repo smoke tests. PR opened; stopped per RULE 025. |
| 2026-08-18 | M00 executed end-to-end: workspace, domain model, graph, CLI, testkit, AgentOS vendored + adapted, governance docs, gates, CI, PR opened. Stopped per RULE 025. |
