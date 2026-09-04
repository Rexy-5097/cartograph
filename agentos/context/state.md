# State — Cartograph

> Layer: project · **Update at the end of every session** · Machine-readable
> twin: [../artifacts/project-state.yaml](../artifacts/project-state.yaml)

## Current position

| Field | Value |
|---|---|
| Last accepted milestone | **M14 — GitHub PR integration** (accepted 2026-09-03) |
| Status | M00–M14 **ACCEPTED**; M15 **implemented on `main`, not accepted** |
| Branch | `main` |
| Next permitted milestone | M15 — **unlocked**; M16 locked until M15 is accepted |
| Last accepted checkpoint | `cartograph-m14` (immutable, commit `caba5ef`) |
| Spec | Frozen V3, August 2026 |

> `current_milestone` in the ledger reads **M15** and stays there.
> `cartograph_cli::version::MILESTONE` is asserted equal to it by a unit test,
> so the two must move together — in a pull request, because the constant is
> product code and a bookkeeping commit may not touch it. The pair advances
> before the checkpoint for the same reason it did at M14: tagging a commit
> whose `cartograph version` still reported the previous milestone would
> mislabel it. M16 stays locked until M15 is accepted.

## What exists

- **M14: ACCEPTED** — GitHub PR integration, delivered as two reviewed slices
  through the fork (PRs #33, #34), a correction the first live run forced (#36),
  and #37 for the version prerequisite.

  The milestone puts the review where the decision is made. `cartograph diff
  --markdown` renders it; a workflow obtains two trees, runs the binary and
  posts what it returns. **No service, no backend, no analysis logic in YAML** —
  the business-model line, *"runs the binary in the user's own CI — no server,
  no hosting cost"*, is intact.

  **Accepted on its stated criterion**: the Action runs on Cartograph's own PRs.
  On [#35](https://github.com/Rexy-5097/cartograph/pull/35), two
  `pull_request_target` runs went green through all ten steps — the first logged
  `created comment 5522919367`, the second `updated comment 5522919367`. The
  comment was **created 08:32:23Z and updated 08:35:41Z**, and the count of
  comments carrying the marker stayed at **exactly 1**. A third dispatch on #37
  behaved identically.

  **The workflow came from `main`, provably.** Head `81c0285` carries its own
  copy whose step 8 reads *"Check out the head tree"*; `main` reads *"Fetch the
  head tree"*; the run executed **Fetch**. A fork cannot alter the workflow that
  reviews it.

  **`pull_request_target` is required, not convenient.** On `pull_request` a
  fork's token is read-only, so `pull-requests: write` cannot be granted and the
  comment 403s — the criterion would be unreachable for a fork contribution.
  Safety comes from never executing pull-request code: the binary is built from
  the base commit at step 5, the head arrives at step 7, and `build.rs` exists
  only in `cartograph-cli`, so the head is not on disk when the only build
  script runs. The analyser reads files and never executes them.

  **The first live run failed, and the failure improved the design.**
  `actions/checkout` now refuses a fork's head under `pull_request_target`
  without `allow-unsafe-pr-checkout: true`. The opt-in was not taken; #36
  fetches the head as a tarball, which unpacks with **no `.git`, no remote and
  no stored credential**. "The head is data" became a property of the
  filesystem rather than of intent, and the guardrail stays armed for whoever
  edits the file next.

  **Scopes are `contents: read` and `pull-requests: write`**, reported by the
  runner itself on both runs. No token and no runner-local absolute path
  reached a comment or a log. GitHub's 65,536-character limit is handled by
  dropping whole lines and saying so — established on a 2 MB synthetic review,
  not on a live pull request.

  **Not delivered:** no reusable `action.yml` for third parties; no desktop
  integration; and **a populated relationship table has never been observed
  live**, because every dogfood pull request changed only Markdown or Rust, so
  each review correctly read *"No architectural change."* A workflow also
  cannot validate a change to itself — the trigger reads from the base branch,
  so #34 and #36 were each reviewed by the version they replaced. HARD GATE 4
  is a product-usage clock and is the owner's to start; its 30-day period began
  with M13's acceptance and has not elapsed.

- **M13: ACCEPTED** — structural diff, delivered as three reviewed slices
  through the fork (PRs #30, #29, #31, plus #32 for the version prerequisite).

  The milestone answers "what changed architecturally between two branches?",
  and it could not begin until a prerequisite ADR-0014 had deferred twice was
  met: **stable cross-run identity**. Two branches are two independent
  analyses, so nothing about a handle survives between them — `NodeId` restarts
  at zero in each graph. Identity is `(kind, name, file)`, with the line
  deliberately excluded so an edit above an artefact does not rename it. Two
  independent analyses of Airflow `9b43d6abc0fc` assigned identical identities
  to all **3,104** nodes, and Zulip `0ce8f6278cde` to all **1,308**.

  **Accepted on its stated criterion**: a diff of two real branches listing
  added, removed and changed edges with evidence. Airflow `ed68491d8b` to
  `9b43d6abc0` gives **20 added, 5 removed, 3,304 unchanged**, and the result
  is legible as a real refactor — `DagsFilters` stopped calling two hooks
  directly and those calls appear in new components. Every reported edge
  carries kind, provenance, confidence, location and its observation.

  **The changed case needed real source to exercise.** The upstream revision
  pair produced none, so one declared HTTP method was removed from a client
  call: the same artefact still requests the same endpoint, and the evidence
  *about the method* weakens. Same source identity, same target identity, same
  kind; confidence **0.98 → 0.784**, which is exactly the undeclared-method
  factor. 0 added, 0 removed, 1 changed, 3,323 unchanged.

  **A rename is remove-plus-add, by policy**, because matching renames means
  guessing from similarity and a wrong guess claims one artefact became
  another. A move within a file is matched.

  **The scope line's second half is delivered as an API, not as a view.**
  `layout(prev_graph, new_graph, prev_positions)` holds artefacts where the
  reader last saw them: across the same Airflow pair, **all 3,071 survivors
  kept their positions while a plain layout would have moved every one**, and
  2,257 of those artefacts had been renumbered between the analyses. No desktop
  client consumes it yet, so the benefit is not visible in MAP.

  **Inherited, not fixed:** `node_for` merges artefacts sharing
  `(kind, name, file)`, so two same-named methods in one file were already a
  single node before identity existed. The diff cannot distinguish what graph
  construction never distinguished. `provenance` and `file` changes remain
  unit-tested rather than observed on a real corpus. HARD GATE 4 is a
  product-usage clock and is the owner's to start.

- **M12: ACCEPTED** — blast radius, delivered as three reviewed slices
  through the fork (PRs #24, #25, #26, plus #28 for the version prerequisite).

  The milestone answers "what breaks if I change this?" by **reverse
  reachability over one graph**, and the decision it turned on is arithmetic:
  path confidence is the **weakest step, not the product**
  ([ADR-0018](../../docs/adr/ADR-0018-blast-radius-confidence.md)). Multiplying
  would score a five-hop chain of 0.98 edges at 0.90 and one of 0.80 edges at
  0.33 — ranking a long certain chain below a short doubtful one, and measuring
  depth rather than evidence. A node is scored by its best-supported route.

  **Accepted on its stated criterion**: cross-stack impact sets with evidence.
  On Airflow (`9b43d6abc0fc`), the Python class `Connection` reaches **767
  artefacts at depth 5, nine of them TypeScript** — React components, the
  generated client, the query hooks — through the HTTP boundary. All 767 `via`
  ids resolve to edges carrying kind, provenance, confidence and location.
  Zulip (`0ce8f6278cde`) gives 113 at depth 1 for `Stream`. Core, CLI and
  desktop agree entry for entry.

  **The clients decide nothing** (RULE 002): each calls `blast_radius` once.
  No cross-run identity was introduced — reverse reachability runs over one
  graph, and `id.rs` is untouched, so ADR-0014's gate still stands for M13.
  A selection held across a re-analysis is refused as `StaleSelection` rather
  than answered, because `NodeId` restarts at zero and would otherwise name a
  real but different artefact.

  **Confidence is still an uncalibrated prior** — `calibrated: false` travels
  with every payload, and M08's ECE of 0.18 is unchanged. One representative
  route is reported per artefact, not all of them. Where many edges converge,
  a particular route edge can be hard to click, and a node can be geometrically
  unclickable — which is why Slice 3 added find-by-name.

- **M11: ACCEPTED** — MAP, the desktop application, delivered as four reviewed
  slices through an open-source fork (PRs #18, #19, #21, #22, plus #23 for the
  version prerequisite).

  M11 added a **second client, not a second implementation**. ADR-0001 claimed
  the CLI, desktop and MCP server are peers of one core; that was not quite
  true, because the sequence turning a directory into a graph lived inside the
  CLI *binary*, which nothing can depend on. It now lives in
  `cartograph-pipeline`, so the claim is true rather than aspirational.

  Layout is computed in Rust and **copied to the renderer without arithmetic** —
  asserted with bit equality on both sides, because a tolerance would pass a
  frontend transform. Sigma draws and computes nothing. The evidence record is
  a **lookup** rather than part of the render payload, so ten thousand nodes do
  not carry data nothing draws. A selection cannot outlive its graph: `EdgeId`
  restarts at zero per analysis, so a stale id would otherwise resolve to a
  *different* relationship, and the Rust side refuses it by generation token.

  **Accepted on two criteria.** MAP answers "what is this system?" — judged
  PASS by the project owner on Apache Airflow (`9b43d6abc0fc`): 3,104 nodes,
  3,350 edges, 313 clusters, with 175 HttpCall edges from TypeScript into
  FastAPI handlers and 60 tables. And 10,000 nodes at 60 FPS — six foregrounded
  runs, median 6.70–10.00 ms and p95 11.10–14.30 ms, both inside the 16.67 ms
  budget every time.

  **Not every corpus produces a map that rich.** Zulip gives 1,308 nodes but
  **zero routes** and one table, because it registers URLs through
  `rest_path(...)`, which the resolver does not recognise; full-stack-fastapi
  gives 2 nodes. Both are inherited M07/M08 resolver coverage, not rendering —
  the same desktop draws all three faithfully. Worst frames of 31–47 ms are
  real, occasional stutter. The Windows MSI bundle cannot be produced because
  `tauri.conf.json` declares only a `.png` icon.

- **M10: ACCEPTED** — incremental analysis, delivered as correctness first.
  A parse fact cache keyed by content hash, recorded semantic read-dependency
  tracking through the resolver, a per-file Stage C ORM access cache with
  dependency-aware reuse, and failure-safe publication. The invariant is
  `incremental_result == clean_rebuild_result`, checked by canonical semantic
  graph equality with the clean rebuild **retained as oracle**.

  Measurement chose the target: Stage C was 894.8 ms of a 950.9 ms cold resolve
  on zulip (94.1%), and is now 4.34 ms warm. The other derived structures were
  measured and deliberately not cached — ExportedConstants 1.8 ms, ModuleIndex
  0.29 ms, RouteIndex 0.032 ms, RouterIndex 0.005 ms. The claim is **work
  avoided** (1,067 hits / 0 misses on an unchanged tree), not milliseconds;
  `<250 ms` was never adopted as a criterion and is not claimed as met.

  **Its scope line is wider than what shipped.**
  [ADR-0014](../../docs/adr/ADR-0014-m10-scope-reconciliation.md) (Accepted)
  records that content-hash node identity, notify file watching and the local
  graph database are **deferred, not delivered**. Stable cross-run identity
  carries a forward obligation: **M13 requires it unconditionally**; M12 does
  not, by its stated acceptance. ADR-0014 states the gate that work must pass.

  622 tests, 9/9 gates and 98/100 on Windows. Both caches are process-local, so
  the CLI cannot yet benefit across invocations. The zulip and airflow mutation
  evidence is **recorded, not re-run at acceptance** — the pinned mirrors live
  outside the repository and were unavailable on the acceptance machine, so all
  ten opt-in harnesses were skipped.

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
