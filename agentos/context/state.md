# State — Cartograph

> Layer: project · **Update at the end of every session** · Machine-readable
> twin: [../artifacts/project-state.yaml](../artifacts/project-state.yaml)

## Current position

| Field | Value |
|---|---|
| Last accepted milestone | **M16 — ASK** (accepted 2026-09-22) |
| Status | M00–M16 **ACCEPTED**; M17 **in progress, not accepted** |
| Branch | `main` |
| Next permitted milestone | M17 — **unlocked**; it is the final milestone |
| Last accepted checkpoint | `cartograph-m16` (immutable, commit `c9cde1b`) |
| Spec | Frozen V3, August 2026 |
| Version | 0.1.0 — the move to 1.0.0 is M17's release step and has not happened |

> `current_milestone` in the ledger reads **M17**, with `status: IN_PROGRESS`.
> `cartograph_cli::version::MILESTONE` is asserted equal to it by a unit test,
> so the two must move together — in a pull request, because the constant is
> product code and a bookkeeping commit may not touch it. The ledger said the
> pair advances in the first M17 pull request; it did not move in #69–#72 and
> advances in PR #73. Unlike M11–M16, it moves while the milestone is still
> being built, so `cartograph version` reports M17 before all of M17 exists.
> The milestone identifier is not the semantic version: that stays 0.1.0
> until the v1.0 release step.

## M17 — in progress

Scope, from [M17-public-launch.md](../milestones/M17-public-launch.md): v1.0
stable, the technical writeup, and a VitePress documentation site.

- **Technical writeup** — merged in #70, after the traceability remediation in
  #69 that traced every figure it uses to a committed artifact.
- **Documentation site** — merged in #73, built from `docs/` and link-checked
  on every pull request. It is
  [published to GitHub Pages](https://rexy-5097.github.io/cartograph/) from
  `main`. #82 gave it a home page and a theme of its own.
- **v1.0 stable** — not begun. The workspace version is 0.1.0 and no `v1.0.0`
  or `cartograph-m17` tag exists.
- #71 and #72 corrected stale resolver documentation.

## What exists

- **M16: ACCEPTED** — ASK, delivered as twenty-two reviewed pull requests
  through the fork, #46 through #67 — implementation slices interleaved with
  the ADR decisions each one needed first. Scope recorded in
  [ADR-0021](../../docs/adr/ADR-0021-ask-boundary-and-citation-contract.md).

  **The one condition no offline test can prove has now been observed once.**
  A developer ran the `#[ignore]`d harness at
  `crates/cartograph-desktop/tests/live_groq.rs` on their own machine, against
  a real key held in the Windows Credential Manager, through the production
  path and nothing else: `GrantedRepository::establish`, `KeychainStore`,
  `ask::explain_with_os_keychain`, `GroqProvider` over `UreqTransport`. What
  the run printed, verbatim:

  | | |
  |---|---|
  | credential supplied | yes |
  | repository grant established | yes |
  | ASK opt-in enabled | yes |
  | credential available | yes |
  | model | `openai/gpt-oss-120b` |
  | HTTP + parse + validate | yes |
  | elapsed | 1.977211s |
  | explanation items | 2 |
  | every item cited | yes |
  | citations verbatim in the bundle | yes |
  | frontend payload safe | yes |
  | credential deleted afterward | yes |
  | ASK opt-in restored to off | yes |
  | test result | ok — 1 passed, 0 failed |

  **What that establishes.** A real provider answered through the shipped
  assembly, and both halves of the citation contract held against a live
  answer rather than a fixture: every item carried a citation, and every
  citation was re-checked byte for byte against the evidence bundle that was
  actually sent. The serialised payload did not contain the repository
  identity. The `Drop` guard removed the credential and switched ASK back off,
  and the harness re-read both to confirm it.

  **What it does not establish, and is not claimed.** One request, one
  machine, one model, one subject tree — the parser fixtures, chosen because
  they are small and public. It is not a measurement of answer quality, not a
  claim about any other repository, and not evidence about latency: 1.977211s
  is a single sample. The harness is developer-only and `#[ignore]`d, so CI
  never runs it and no automated check depends on it.

  **No production credential surface was introduced, deliberately.** Nothing
  the product ships can put a key into the keychain: no settings panel, no
  key field, no environment-variable path, no CLI path, no configuration
  entry. That gap is the frozen v1 scope, and the provisioning step therefore
  lives in the ignored test rather than in the product. RULE 013 holds on
  every path — with no key and no network the feature still returns the
  derived evidence.

  **Two earlier attempts failed in the developer's shell, not in the
  product.** The first delivered a credential carrying NUL bytes, because
  Windows PowerShell does not connect two native commands with an operating
  system pipe: it decodes the child's stdout and re-encodes it, adding a UTF-8
  BOM and a line ending. UTF-16LE-encoded ASCII is valid UTF-8, so nothing
  downstream objected until `http` refused a control character in a header
  value. The second delivered six bytes beginning with `ESC`, a terminal's
  bracketed-paste marker. Both were located with a synthetic sentinel and no
  credential, and both were fixed by changing shells rather than code — the
  successful run used interactive Git Bash, whose pipe is byte-transparent.
  No production code was changed in response to either, and the harness still
  refuses malformed input rather than repairing it.

  **Accepted on its stated criterion** on 2026-09-22, checkpointed as
  `cartograph-m16` on merge commit `c9cde1b`. The pair advanced in PR #67 and
  the tag followed it, so `cartograph version` reports M16 at the tagged
  commit.

  **Not delivered:** no way for an ordinary user to install a key. A settings
  panel is a frozen v1 non-goal and an environment-variable path is forbidden,
  so `CredentialStore::set` has no production caller and the only provisioning
  route is the ignored test. Anyone without one gets the degraded path, which
  is the path RULE 013 requires to be fully useful. Also outstanding: on
  Windows a DNS failure is reported as an unreadable reply rather than an
  unreachable host, found during this milestone's diagnosis and left as a
  follow-up.

- **M15: ACCEPTED** — the MCP server, delivered as three reviewed slices through
  the fork (PRs #38, #41, #43), a test-isolation fix (#44) and #45 for the
  version prerequisite. Scope recorded in
  [ADR-0020](../../docs/adr/ADR-0020-mcp-boundary-and-authorization.md).

  The milestone puts the graph where an agent already is. `cartograph-mcp`
  speaks MCP over stdio and exposes exactly four tools — `map`, `trace`,
  `blast`, `diff` — over **one** repository identity, granted in argv when the
  server is launched.

  **Accepted on its stated criterion**, and the client was a real Claude Code
  session rather than a hand-written harness. `map` returned the A4 view
  byte-identically across runs; `trace` walked TypeScript → Python over an
  http-call at confidence 0.784, then `orm-access` to `Order` and `queries` to
  the table `orders`, reaching a table in three hops with evidence and
  provenance on every step; `blast` returned three dependents at depths 1–3
  carrying weakest-link confidence; `diff` over the two granted trees returned
  *added 3, removed 0, changed 0, unchanged 8*.

  **"And nothing else" was tested, not assumed.** Four distinct ungranted trees
  were refused across `map`, `trace` and `diff`, and **both** cross-repository
  `diff` directions were refused. `trace` against an ungranted tree refused on
  authorization rather than reporting an unknown symbol — so authorization
  precedes analysis at the client boundary too.

  **Privacy held.** No response carried an absolute path, a parent path, the
  opaque identity or a token, and the refusal echoes none of its input. An
  ungranted tree that *exists* and an ungranted path that does *not* produced
  byte-identical refusals, so existence is not disclosed either.

  **Not delivered:** no Rust grammar — the workspace compiles only the
  TypeScript and Python tree-sitter grammars, so a Rust repository maps empty
  and no Rust symbol can be traced; no desktop consumer of MCP; no reusable
  packaging for a third party; and analysis latency on a large repository
  remains a follow-up concern rather than a solved one.

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
| 2026-10-28 | On `feature/m17-site-perf` (stacked on `feature/m17-site-logo`), the landing page's motion made cheap. Measured first with a frame-timing probe that drives headless Chrome over the DevTools protocol, before and after. The main thread was busy 345–445 ms of every second at rest, and 66–100% of frames were late with the CPU slowed 4×. Causes found: the scroll handler interleaved reads and writes (a forced layout per element per frame), the rising panels' inherited CSS variable restyled most of the page per frame, canvases drew at 120 Hz with ~560 `fillText` glyphs, the dot fields recomputed fixed geometry per dot per frame, inline-SVG loops ran off screen, and the marquee called `getAnimations()` every frame. After the fixes: 99–144 ms/s at rest, and 0–0.3% of frames late at rest with the CPU slowed 4×. M17 is unchanged: in progress, not accepted. |
| 2026-10-28 | On `feature/m17-site-logo`, the site's generic "share" icon gave way to Cartograph's own mark: a square node and a round node joined by an edge bent into a check, inside survey brackets. It was chosen from three concepts the owner compared. It builds itself, answers hover and keyboard focus, and idles only on the landing page, which has a pause control. It is in the header, the hero badge, the docs navigation (new) and the favicon. Checked in the preview: hover, focus-visible, "Pause motion" (every animation `none`), no idle on docs pages, both docs themes, favicon at 16/32/64 px. On the same branch, a loading screen in the order the owner set out: the mark builds in the middle, the name writes out, the name retracts to the mark, and the mark alone moves into the header, where the name writes out again before the globe and headline arrive. It plays on a full load of the home page only, is skipped by any input, by "Pause motion" and by reduced motion, and a head-script timer clears it after 8 s if anything fails. Found while verifying: the hand-off landed 0.5 px off because `offsetLeft` rounds; fixed, it lands exactly. M17 is unchanged: in progress, not accepted. |
| 2026-10-28 | The orbit labels still looked like they shook after #89 was deployed. Found on `fix/m17-orbit-pill-glyphs`: far-side labels were drawn under the globe, so its flickering glyphs crossed their text (9.6% of a far pill's pixels changed per frame), and near-side pills were only 92% opaque. Every label is now an opaque pill drawn last, with the far ones dimmed; at most 0.06% of any pill's pixels change per frame. M17 is unchanged: in progress, not accepted. |
| 2026-10-28 | On `feature/m17-site-idle`, the home page's two dot fields gained idle motion: a radar sweep in the field, a sonar pulse in the dome, and a crosshair that surveys the dots while the pointer is away. Nothing new is drawn over content in a clearing: 0 pixels inside either content box over 24 samples each. The first pulse faded before it reached the dense ring and lit at most 4% of the dots; fixed, it peaked at 78% before its glow was narrowed. Found while verifying: the app's preview browser can deliver no animation frames for seconds after a navigation (0 frames to a bare `requestAnimationFrame` counter), which looks like a canvas that never draws. M17 is unchanged: in progress, not accepted. |
| 2026-10-27 | Fixed on `feature/m17-site-orbit-pills`: the globe's orbit labels shook inside their pills. Root cause, measured in the preview: canvas text snaps to whole device pixels vertically while the pill's outline moves smoothly. Each label is now one pre-drawn image placed on whole device pixels. M17 is unchanged: in progress, not accepted. |
| 2026-10-27 | Home page reworked on `feature/m17-site-statement`. The governing principle is now set large in an animated box over the halftone field, and the footer dome's empty half holds "Try it": the install command and links. The hero's drifting constellation became a rotating globe of code glyphs. Edges run from it to the taglines, it spins faster on scroll, and its orbit carries the benchmark figures the README publishes. A frame first took 9–14 ms to draw in the preview, and about 2 ms after glyphs were batched by font and colour. Rejected along the way: a full visual redirection away from the reference site, and a static mark in place of the constellation. M17 is unchanged: in progress, not accepted. |
| 2026-10-27 | Found why cartograph.co.in still showed the old site after #82–#84 merged. Vercel serves the domain, last deployed 26 Oct 20:09 UTC from a build for `/`, and no Vercel project is connected to this repository. The docs workflow deploys only to GitHub Pages, which is current. On `fix/docs-site-base`, the site base is now read from `CARTOGRAPH_SITE_BASE` (default `/cartograph/`), so a host serving a domain's root can build from `main`. Built and link-checked at both bases. Connecting the domain to this repository's build needs whoever owns the Vercel project or the DNS. Not done here. M17 is unchanged: in progress, not accepted. |
| 2026-10-27 | Development environment set up on Windows 11 from a zip of a contributor's clone. The workspace, desktop, benchmark and docs checks all passed there. Live docs site audited: no broken links among its 52 pages, 58 assets, 626 anchors and 55 external links. PR #82 (home page and theme) merged. An AgentOS review of it (`ui-reviewer`, `docs-reviewer`, QG-002) found WCAG 2.1 AA failures: focus-ring and text contrast, five invisible tab stops, an unpausable marquee, and a panel clipped at 761–960px. It also found a development README that did not say where the theme lives. All are fixed on `feature/m17-site-review`. The same branch then gave the home page motion (scroll-linked, written without an animation library, stopped by reduced motion and a "Pause motion" control), and replaced bare addresses with named links. A third branch, `feature/m17-site-icons`, stacked on it: the step icons build themselves and then idle, and the halftone caption is typed into a drawn box. Found, not fixed: the root `AGENTOS.md` gives the validator as 99/100, but a clone scores 98/100, because `agentos/.gitignore` excludes `production_validation/productivity_metrics.csv`. M17 is unchanged: in progress, not accepted. |
| 2026-08-31 | Development environment reconstructed on Windows 11 from GitHub alone (no SSD, no prior machine). Found M09 merged but never accepted, and a Windows privacy defect that made its gates fail: redaction gated on `Path::is_absolute()`, which misses drive-less rooted paths, leaking them into serialised JSON and stderr. Fixed under PR #16 with six regression tests; analyser untouched. M09 accepted at `e8416f9` (`cartograph-m09`), folding PR #16 in per the M07 precedent. M10 unlocked and **not** started, per RULE 025. |
| 2026-08-19 | M01 accepted (`cartograph-m01`), M02 executed: Python extractor into the shared fact model, route observations, 47 new tests, benchmarks, three smoke corpora. `RouteFramework` corrected to `RouteDeclarationStyle` after Flask's repo disproved the framework label. PR #3 opened; stopped per RULE 025. |
| 2026-08-19 | M00 checkpoint semantics repaired (ADR-0010: provisional rc tags vs accepted tags; premature cartograph-m00 retired, rc1 created at exact PR head). M01 executed: tree-sitter TS/TSX extractor, fact model, diagnostics, CLI `parse`, 22-category fixture corpus, 82 tests, criterion group, two real-repo smoke tests. PR opened; stopped per RULE 025. |
| 2026-08-18 | M00 executed end-to-end: workspace, domain model, graph, CLI, testkit, AgentOS vendored + adapted, governance docs, gates, CI, PR opened. Stopped per RULE 025. |
