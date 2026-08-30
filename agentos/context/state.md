# State — Cartograph

> Layer: project · **Update at the end of every session** · Machine-readable
> twin: [../artifacts/project-state.yaml](../artifacts/project-state.yaml)

## Current position

| Field | Value |
|---|---|
| Active milestone | **M09 — CLI v0.1.0** (accepted; this tree implements M09) |
| Status | M00–M09 **ACCEPTED**; M10 unlocked, no code written |
| Branch | `main` |
| Next permitted milestone | M10 — **unlocked**; M11 locked until M10 is accepted |
| Last accepted checkpoint | `cartograph-m09` (immutable, commit `e8416f9`) |
| Spec | Frozen V3, August 2026 |

> `current_milestone` in the ledger stays at M09 because it names what this
> tree implements — `version::MILESTONE` is checked against it by a unit test.
> M10 is unlocked through `next_allowed_milestone`; the field advances with the
> first M10 code, not with M09's acceptance.

## What exists

- **M09: ACCEPTED** — the first developer-facing release. Six commands over
  one shared pipeline, a versioned `--json` document (`schema_version` 1.0)
  with a conformance suite, documented and tested exit codes, and an npm
  launcher that reimplements nothing.

  The accepted state is PR #15 **plus** PR #16. Moving development to Windows
  found a privacy defect: redaction was gated on `Path::is_absolute()`, which
  on Windows requires a drive or UNC prefix, so a path rooted on the current
  drive (`\rooted\secret`) was reproduced in full — in the serialised
  `repository.requested` field and in the human-readable error. On Unix the
  two predicates are equivalent, so CI never saw it. `discovery::is_rooted`
  (`is_absolute() || has_root()`) now owns the question at both call sites.
  `c578b42` was **not** tagged: its gates fail on Windows.

  **The analyser is byte-identical to `cartograph-m08`**, so M09 inherits
  M08's accuracy limits rather than improving on them. 497 tests, 9/9 gates
  and the full CLI surface pass on `x86_64-pc-windows-msvc`; zulip
  (`0ce8f627`, 1,539 files) and full-stack-fastapi (`162344da`, 156 files)
  both analyse clean with no path leakage. Windows is now validated for
  **development**, not for release artifacts — only the Linux and
  Apple-Silicon binaries are built and smoke-tested by CI.

- **M08: ACCEPTED** — a measurement baseline, not completed calibration
  science. Confidence is an uncalibrated prior selected by evidence class,
  not a probability. ECE 0.1819/0.1825, MCE 0.35, all under-confidence;
  overconfidence is unmeasurable because the low bands have no verified
  observations. Recall measured where M07 left it open: HttpCall 117/129,
  OrmAccess 1401/1883, chains 3 of 5. **24.9% of edges are unverifiable and
  every figure is an upper bound.** No production threshold changed.

- **M07: ACCEPTED** — the acceptance criterion is met four times over. Four
  complete chains run from hand-written React components in Airflow's UI,
  through generated clients, to database tables, traversing shared graph
  nodes with confidence, provenance, evidence and a location on every edge.
  M07 began as a negative finding and that record stands unaltered; what
  followed was remediation, not revision. **Recall for HttpCall, OrmAccess
  and chains remains UNMEASURED**, and only Airflow yields frontend chains —
  both open limitations, recorded rather than hidden.
- **M07 remediation** — import resolution, configuration-object requests,
  router prefix composition and client-wrapper call edges. Three complete
  chains now run from a hand-written React component to a database table on
  Airflow. 379 false ORM accesses removed, including five of M07's six
  reported chains. Route and Queries metrics unchanged at 1.000.
- **M07: real-repository validation** — a pinned seven-repository corpus,
  scope declared before measuring, ground truth authored by an instrument that
  never sees analyser output, two full passes, and eleven attacks on the
  benchmark's own machinery. Route extraction and model-to-table resolution
  both reach precision 1.000 and recall 1.000 on their in-scope ground truth.
  **Six fully verified chains across seven repositories, none beginning at a
  frontend file** — real frontends call through generated client wrappers, so
  no URL appears at the call site. Five engine defects found by reading output;
  ten future-work items recorded and none built.
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
| 2026-08-31 | Development environment reconstructed on Windows 11 from GitHub alone (no SSD, no prior machine). Found M09 merged but never accepted, and a Windows privacy defect that made its gates fail: redaction gated on `Path::is_absolute()`, which misses drive-less rooted paths, leaking them into serialised JSON and stderr. Fixed under PR #16 with six regression tests; analyser untouched. M09 accepted at `e8416f9` (`cartograph-m09`), folding PR #16 in per the M07 precedent. M10 unlocked and **not** started, per RULE 025. |
| 2026-08-19 | M01 accepted (`cartograph-m01`), M02 executed: Python extractor into the shared fact model, route observations, 47 new tests, benchmarks, three smoke corpora. `RouteFramework` corrected to `RouteDeclarationStyle` after Flask's repo disproved the framework label. PR #3 opened; stopped per RULE 025. |
| 2026-08-19 | M00 checkpoint semantics repaired (ADR-0010: provisional rc tags vs accepted tags; premature cartograph-m00 retired, rc1 created at exact PR head). M01 executed: tree-sitter TS/TSX extractor, fact model, diagnostics, CLI `parse`, 22-category fixture corpus, 82 tests, criterion group, two real-repo smoke tests. PR opened; stopped per RULE 025. |
| 2026-08-18 | M00 executed end-to-end: workspace, domain model, graph, CLI, testkit, AgentOS vendored + adapted, governance docs, gates, CI, PR opened. Stopped per RULE 025. |
