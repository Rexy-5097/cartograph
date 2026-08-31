# Changelog

All notable changes to Cartograph are recorded here.

Format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).
Versioning follows [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added — M11, deterministic layout in the Rust core

First M11 slice. No UI: Tauri, React, Sigma.js and Zustand are **not** added
here. The slice establishes the boundary those will later consume, because
[ADR-0001](docs/adr/ADR-0001-rust-core-is-the-product.md) and
[ADR-0006](docs/adr/ADR-0006-sigma-before-custom-renderer.md) both place layout
in the core and give the frontend `{id, x, y, cluster}` to draw and nothing
else. Building the renderer first would have formed that boundary in the wrong
place.

- **`cartograph_graph::layout`** — positions every node of an architecture
  graph. `layout(&graph, &params) -> Layout`, where a `LayoutNode` is exactly
  `{id, x, y, cluster}` and a `Cluster` is `{id, label}`. No name, kind,
  confidence, provenance, evidence or location appears in the payload, and a
  test asserts the serialised key set so that adding one fails the build.
- **Clustering by directory.** A located node joins the cluster named by its
  repository-relative parent directory; a node without a location — a table, an
  external service, an environment variable — joins a cluster named for its
  kind. Cluster ids are assigned by sorting labels, so they follow content
  rather than traversal order.
- **Per-cluster Fruchterman–Reingold relaxation** inside a frame whose area
  equals the cluster's population, with cluster centres on a phyllotaxis spiral
  spaced relative to the largest cluster. Cross-cluster edges deliberately
  exert no force; see [ADR-0015](docs/adr/ADR-0015-layout-contract.md).
- **Determinism, earned rather than hoped for.** Ordering is by semantic
  identity and never by `NodeId`; every map is a `BTreeMap`; edge pairs are
  de-duplicated into a sorted set; the iteration count is fixed; initial
  positions come from an FNV-1a hash written out in the module rather than from
  a random number generator or `DefaultHasher`. Two independent analyses of one
  tree therefore lay out identically — which is **not** stable cross-run node
  identity, and does not disturb ADR-0014.
- **19 contract tests and a real-repository test.** Totality (one record per
  node, no duplicates, no strangers), well-formedness (finite, inside the
  requested extent, every cluster resolvable), the empty / one-node / coincident
  / disconnected / zero-iteration cases, insertion-order and edge-order
  independence, a duplicated edge not pulling harder, a golden layout, and the
  serialised payload's key set.

### Measured — M11 layout baseline

Layout time only. **This says nothing about M11's "10k nodes at 60 FPS"
criterion**, which concerns a rendering application that does not exist yet.

| Graph | Time |
|---|---|
| 100 nodes, 1 directory per 100 | 1.57 ms |
| 1,000 nodes | 15.8 ms |
| 10,000 nodes | **1.06 s** |
| 2,000 nodes over 200 directories | 10.6 ms |
| 2,000 nodes over 1 directory | **758 ms** |

Distribution matters more than size: relaxation is quadratic within a cluster
and does no work between clusters.

### Known limitations — M11 layout

- Cross-cluster edges do not attract, so a frontend calling a backend is not
  drawn nearer to it.
- A repository that keeps everything in one directory is the pathological case.
- Real-repository evidence is thin: the analyser handles TypeScript, TSX and
  Python, and this repository is mostly Rust, so its own graph is 10 nodes. The
  pinned corpora are not vendored and were unavailable; the test accepts
  `CARTOGRAPH_LAYOUT_REPO`.
- `serde_json` writes an f64 exactly but its parser is not always correctly
  rounded — one fixture coordinate returns 1 ULP high. Recorded in the
  round-trip test rather than fought.

Milestone **M10 — incremental analysis engine**. First slice: the parse cache.
No product behaviour changes; the analyser is byte-identical to
`cartograph-m09`.

### Added

- **Content-addressed parse cache.** Per-file facts are keyed by
  repository-relative path and a hash of the file's bytes, and reused when the
  bytes have not changed. `Analyzer::dispatch` is a pure function of
  `(path, language, source)`, so a cache hit cannot produce a different answer
  than a fresh parse.
- **Canonical graph equality** (`cartograph-testkit::canonical`) — the relation
  `incremental == clean` is stated in. Compares node and edge *semantic*
  identity, including confidence, provenance, evidence and locations, as sorted
  multisets. Excludes `NodeId`/`EdgeId` (graph-local, ADR-0011), insertion
  order and `created_at`.
- **Deterministic repository-mutation harness** and a differential suite: edit,
  create, delete, rename, backend route change, dynamic-URL base change, ORM
  table rename, a file becoming malformed and then repaired, invalid UTF-8,
  revert/recreate cycles, and two independent mutations in one update. Each
  asserts the incremental graph equals a clean rebuild of the mutated tree.
- **Cache instrumentation** behind `tracing` — files seen, reused, parsed and
  evicted. Counts only, never file contents. Not a CLI contract.

### Measured

Warm re-run of an unchanged tree, in process, graphs verified equal:
full-stack-fastapi 159 ms → 35 ms (4.5×); zulip 16,980 ms → 5,625 ms (3.0×);
zero files reparsed in both. A warm run now spends nearly all its time above
the parse layer.

### Known limitations

- **Resolution is not incremental.** `ModuleIndex`, `RouteIndex`,
  `ExportedConstants` and `OrmAnalysis` each read every file and are rebuilt on
  every run. This is what makes the equality invariant hold by construction
  here, and it is the remaining bottleneck.
- **No persistence.** The cache lives in one process, so the CLI — which exits
  after each run — cannot yet benefit. No on-disk format, no file watching.
- Stable node identity across runs is still deferred (ADR-0011).
- No latency target is claimed.

## [0.1.0] — 2026-08-22

The first public release. Milestone M09.

The engine was validated at M07 and measured at M08; M09 turns it into a CLI an
external developer can install and use. **No resolver logic changed** — the
analyser is byte-identical to the accepted M08 state, and all seven benchmark
repositories reproduce that state's graph edge for edge.

### Added

- **`cartograph .`** — the default command. Analyses a repository and reports
  what it found under three headings, in vocabulary chosen not to overclaim:
  **observed** (the source says this), **resolved** (the resolver determined
  this), **refused** (the resolver declined to claim this). Refusals are printed
  at the same weight as results, because a tool that shows its successes
  prominently and its refusals in small type is misrepresenting its own
  reliability. The summary never says *verified*.
- **`cartograph trace <symbol>`** — follows one symbol's relationships across
  the stack, walking the graph outward and reporting each hop's confidence,
  provenance, evidence and locations. The path comes out of the graph; nothing
  is re-derived in the CLI.
  - An ambiguous symbol lists its candidates and exits `6`. Nothing is chosen —
    picking one would make the answer depend on iteration order.
  - A fork prints `branch 1 of 2`, because two successors printed in sequence
    read as `A → B → C` when the graph says `A → B` and `A → C`.
  - A break is marked with why: chain end, depth limit, or cycle. Where the
    chain stops in a file that also holds refused observations, those are shown
    and labelled as being in the same file rather than attributed to the symbol.
- **A versioned JSON contract** — `schema_version` `1.0`, published as
  [a schema file](docs/architecture/cartograph-output-1.0.schema.json) that the
  test suite executes against real output rather than merely describing. Every
  serialised edge carries source, target, kind, confidence, provenance, evidence
  and location.
- **Documented exit codes** — `0` success, `2` usage, `3` input, `4` analysis,
  `5` symbol not found, `6` ambiguous symbol, `7` partial analysis with
  `--strict`. `1` is deliberately unused, so an unexpected `1` is always a
  defect. Every code is tested through the real binary.
- **Release distribution** — a workflow that builds a native binary per platform
  *on that platform*, smoke-tests it there, and attaches it to the release; plus
  an npm launcher that forwards argv, stdio and the exit code and performs no
  analysis in JavaScript.
- **Golden output fixtures** — the summary, trace, error and JSON documents are
  pinned byte-for-byte with machine-specific values normalised, so a change to
  what a user sees appears in review as a diff of the output itself.

### Fixed

- **`cartograph version` reported milestone `M01`** — it had, from M01 through
  M08. The milestone is now checked against `agentos/artifacts/project-state.yaml`
  by a unit test, so it cannot go stale quietly again; the version, commit and
  target triple are derived rather than written down.
- **Absolute filesystem paths leaked into `--json` output** via the `root`
  field. Serialised output now names the repository and never the machine.
- **A closed pipe crashed the process.** `cartograph . | head -5` panicked;
  output is now written once and a broken pipe exits `0`.
- **Diagnostics were miscounted.** A file that both failed and explained why was
  excluded from the diagnostic-file count, so four diagnostics across four files
  were reported as being across two.
- **`repository` was a string in three commands and an object in two.** Caught
  by the schema conformance test, unified on the object.
- **Symbolic links are no longer followed** during discovery, so analysis cannot
  pull source from outside the path the user named.

### Known limitations

Memory scales with repository size — 920 MB on a 32,000-file repository — and
there is no incremental mode, file watching or caching between runs. That is
M10's goal; v0.1.0 records the baseline rather than solving it. `trace` walks
outward only. Only TypeScript, TSX and Python are read.
[The full list](docs/releases/v0.1.0.md#known-limitations).

## [Unreleased]

### Added — M08, confidence calibration and measurement

**Confidence is not a probability.** It is an uncalibrated prior selected by
evidence class: seven distinct values across 14,932 edges, 79% of them at
exactly 0.80. Grouping by confidence and grouping by evidence are the same
partition, so a ten-bin reliability diagram leaves six bins empty. Report:
[docs/benchmarks/m08-report.md](docs/benchmarks/m08-report.md).

- **An independent per-edge labeller** — `benchmarks/m08/label_edges.py`
  re-derives every relationship from source, including a second implementation
  of router prefix composition written from FastAPI's semantics rather than
  from the resolver's code. 11,221 of 14,932 edges verified; **1 false
  positive**.
- **Calibration measured** — ECE 0.1819 development, 0.1825 holdout, MCE 0.35,
  all in the direction of **under**-confidence. The split is computed from edge
  identity and never from the label.
- **The recall M07 left open** — `HttpCall` 117/129 (0.907), `OrmAccess`
  1401/1883 (0.744), end-to-end chain **3 of 5**, each complete inside a scope
  the result names.
- **Results bound to their inputs** — corpus, subset, measurement, labeller and
  the labelled records themselves. QG-009 refuses a result whose integrity
  checks do not pass, and `make gates` now runs the calibration tests.

### Known limitations recorded, not resolved

**24.9% of edges cannot be verified from source**, concentrated where the
evidence is weakest, so every accuracy figure is an upper bound. The three
low-confidence bands have **zero** verified observations, so overconfidence is
not measurable. One false positive is left unfixed deliberately: a function-local
`class Element` in a PostHog test shadows a real ORM model, and the measurement
is more honest counting it than patching it.

### Added — M07 remediation, accuracy-first cross-stack resolution

**Three complete chains now run from a React component to a database table**,
on Airflow, each verified line by line against source. `ParseDagButton.tsx:30`
reaches `dag_priority_parsing_request`; `AddConnectionButton.tsx:29` reaches
`connection`; `AddPoolButton.tsx:28` reaches `slot_pool`. Every edge carries
confidence, provenance, evidence and a location, and the path traverses through
shared node ids rather than matching names. Report:
[docs/benchmarks/m07-remediation.md](docs/benchmarks/m07-remediation.md).

- **Import resolution** — a name is walked through its file's own imports to a
  declaration, for Python and TypeScript. Unresolvable modules, monorepo path
  collisions, re-export cycles and two barrels exporting one name all refuse
  with a reason. [docs/resolver/imports.md](docs/resolver/imports.md).
- **Configuration-object requests** — `__request(OpenAPI, {method, url})` and
  `client.get({url})` are read. `url` and `endpoint` only, never `path`, which
  is how every router in the corpus describes a screen.
- **Router prefix composition** — `@pools_router.post("")` under `/pools` under
  `/api/v2` serves `/api/v2/pools`. Any constructor whose name ends in `Router`
  counts. Composition can only make a path more complete: an unknown prefix
  never becomes an empty one. [docs/resolver/routers.md](docs/resolver/routers.md).
- **The client wrapper is a node** — an `HttpCall` edge is sourced at the
  function issuing the request, and calls reaching it become `Call` edges
  resolved through imports, never matched by name
  ([ADR-0012](docs/adr/ADR-0012-the-client-wrapper-is-a-node.md)).

### Fixed — 379 false ORM accesses, including five of M07's six chains

Resolving model names without consulting imports attributed any symbol sharing
a model's name to that model. Onyx's Airtable connector imports `Document` from
`connectors/models.py`, a Pydantic `BaseModel`, while the SQLAlchemy `Document`
lives in `db/models.py`. **Five of the six chains M07 reported were false**,
all running through an Airflow response datamodel named `Job` rather than
`airflow.jobs.job.Job`. The honest M07 baseline is one chain, not six — a
correction that makes its negative finding stronger.

Route and `Queries` metrics are unchanged at 1.000 precision and recall, all
290 `Queries` edges remain source-confirmed, and no `must_not_produce`
assertion is violated.

### Added — M07, full-stack verification on real repositories

**A negative result, and it is the point.** Across seven pinned real
repositories and 58,225 files, Cartograph produced six fully verified
frontend-to-table chains and **none of them begins at a frontend file**. Of
1,458 `HttpCall` edges, 12 have a TypeScript client. The rest are Python test
suites calling their own API. Real frontends reach their backend through
generated client wrappers, so no URL appears at the call site — the single
largest obstacle to the product's central claim, now measured rather than
suspected. Full report: [docs/benchmarks/m07-report.md](docs/benchmarks/m07-report.md).

- **A pinned corpus** — `benchmarks/corpus.json`: Superset, full-stack-fastapi,
  Onyx, PostHog, Zulip, Airflow, AutoGPT, at exact commits, analysed at their
  repository root so scope cannot be tuned against results. No corpus source
  enters this repository.
- **Scope declared before measuring** — `benchmarks/supported-subset.json`,
  committed before the analyser had run against anything. It predicts seven
  specific implementation gaps in advance so a miss found later cannot be
  reclassified as out of scope.
- **Ground truth independent of output** — drafted from source by an instrument
  that is handed a filesystem path, cannot import the analyser, and never reads
  its output. Its patterns describe what the frameworks serve, not what
  Cartograph recognises.
- **Graph export** — `cartograph match --json` now emits the graph it built, so
  a chain is verified by traversing shared nodes rather than inferred from
  matching names.
- **A benchmark that resists its author** — `benchmarks/evaluate.py` runs ten
  checks against the ways these numbers could be flattered, and QG-009 now
  enforces the artefacts: pinned commits, declared scope, refusal records,
  denominators equal to the in-scope record count, and results bound to the
  digest of the ground truth they were scored against. Eleven attacks were
  written against that gate; all eleven are detected.

### Fixed — five defects the corpus exposed

- **`Model` read as Django.** Django, Flask-SQLAlchemy and Flask-AppBuilder all
  use a base named `Model`. Testing Django first meant every Superset and
  Airflow model was searched for a `Meta.db_table` it does not have while its
  `__tablename__` sat a line below; Superset resolved 3 tables out of 33.
  Flavour is now read from what the class declares. `Queries` recall 0.729 → 1.000.
- **ORM discovery ran over TypeScript.** `new Theme({config})` in a `.tsx` file
  produced an `OrmAccess` edge to a Python model of the same name — 33 across
  the corpus, now zero.
- **Model names resolved without imports.** Airflow's architecture diagrams call
  `User("DAG Author")` having imported `User` from the `diagrams` package; 218
  were attributed to the Flask-AppBuilder model.
- **Route decorators required a listed receiver.** Superset's four
  `@health_blueprint.route(...)` declarations were invisible. A blueprint may be
  bound to any name; the evidence is a literal URL path in the first argument.
- **TypeScript route declarations read as client calls.** `app.get('/_health',
  (c) => …)` is Express and Hono declaring a route; three in PostHog's Node
  services were matched to a Python endpoint in a different service.

### Added — M06, ORM and database resolution

**The chain is complete.** `CheckoutButton.tsx → POST /api/orders →
create_order() → Order → orders` now resolves end to end, every hop carrying
confidence, provenance and evidence.

- **Model discovery** — SQLAlchemy (`Base`, `DeclarativeBase`, `db.Model`) and
  Django (`models.Model`), module scope only. A class is a model only when it
  inherits a recognised base; `OrderBase` is not `Base`.
- **Table resolution** — `__tablename__` and `Meta.db_table` read exactly. **No
  table name is ever derived from a class name**: Django's default needs an app
  label that is not in the model file, so those refuse. Ownership is decided by
  span containment, because every Django model nests a class called `Meta`.
- **Handler → model access** — a deliberate subset of Django manager methods
  and model construction. Unrecognised methods, non-models, module-scope
  accesses and shadowed names all produce nothing.
- **Edges** — `OrmAccess` (handler→model) and `Queries` (model→table) kept
  distinct, provenance `orm-resolution`, deduplicated so one fact stated three
  times is one edge.
- **Raw SQL is not linked to models.** A table name in SQL text is not evidence.
- **Tests** — 58 new (28 ORM, 5 full-stack chain, 5 graph identity); 335
  workspace tests. **Benchmarks** — `orm` criterion group.

**Two parser facts added** because M06 needed them and guessing would have been
worse: class-body attribute assignments (so `__tablename__` is read rather than
inferred from whichever string appears in the class), and class base names.

**Graph identity fixed ([ADR-0011](docs/adr/ADR-0011-node-identity-within-a-graph.md)).**
M04 and M06 each created a `Function` node for the same handler, so every edge
was correct and the chain was still unwalkable. `ArchitectureGraph::node_for`
now returns one node per (kind, name, file).

**Real repositories:** SQLAlchemy 37 models/35 tables/228 edges · Django 869
models/34 tables/1957 edges · zero regressions in M04/M05 decisions.

### Added — M05, dynamic URL and endpoint resolution

**Resolving constructed URLs without guessing.** M04 refused any client URL it
could not read literally; M05 resolves as much of one as the source determines.

- **Symbolic value model** — `Literal`, `Unknown`, `EnvVar`, `Unsupported`,
  `Concat`, with no way to flatten an unresolved value into text. Unresolved
  values render as `{unknown}`, `{env:NAME}`, `{unsupported}`, and a test
  proves none can render as `undefined`, `null` or an empty string.
- **Expression evaluation** — template literals and f-strings, `+` chains,
  module constants resolved transitively with a depth limit so cycles
  terminate, and environment reads across `process.env`, `import.meta.env` and
  `os.environ`. Calls, indexing and operators are `Unsupported`, never
  approximated.
- **Cross-file constants** — only *exported* constants are visible to
  importers, resolved through a restricted relative-path join.
- **Environment variables are never read.** Only the name is recorded; a test
  uses `PATH` to prove no value leaks into the graph.
- **Integration** — a resolved URL becomes an ordinary `UrlObservation` and
  flows through M03 and the M04 matcher unchanged, so every M04 rule applies
  automatically. A test pins that a resolved URL still cannot be swallowed by
  a bare catch-all.
- **CLI** — `cartograph match` resolves dynamic URLs and reports how many were
  fully or partially determined.
- **Tests** — 54 new (18 symbolic, 21 evaluator, 15 end-to-end); 297 workspace
  tests total. **Benchmarks** — `evaluation` criterion group.

**Real-repository result, stated plainly:** M05 resolved **zero additional
matches** across the four corpora tested, and changed **no decision**. The
evaluator runs (63/12/15/6 URLs partially resolved) but those projects build
URLs from runtime-configured SDK clients rather than module constants, so the
prefix is genuinely unknown and refusing is correct. The capability is proven
by fixtures; its real-world reach is limited by a pattern M05 cannot resolve.

### Added — M04, cross-language route resolution

**The first semantic join, and the first `HttpCall` edges.** A TypeScript or
Python client call is now matched against Python route declarations and, when
the evidence supports it, becomes a graph edge carrying confidence, provenance
and the case for itself.

- **Candidate generation** — routes indexed by segment count, wildcards held
  separately. Method is deliberately not an index key, since an undeclared
  method is compatible with everything. Client calls can never be indexed as
  routes; uncanonicalisable routes are never indexed at all.
- **Method rules** — exact within a declared set; disjoint methods are not
  candidates at all; an undeclared method on either side is a candidate but
  never exact, and is **never read as GET**.
- **Path rules** — static segments compare exactly; a server parameter accepts
  a concrete client value; a client's unknown value is aligned but undetermined,
  and against a server literal is possible-but-unverified. A trailing wildcard
  consumes one or more segments; a wildcard elsewhere is refused.
- **Discriminating evidence** — an accepted match must share at least one
  static segment. Found by running the matcher over real repositories: a bare
  catch-all route was producing 138 false edges in Flask's own repository, and
  a `/{name}` route was capturing every one-segment call.
- **Ambiguity produces no edge.** Several plausible routes is an unanswered
  question; choosing arbitrarily would make every downstream confidence number
  inherit a guess. Candidates are retained for inspection.
- **Explicit refusals** — an unresolved template prefix (M05 unlocks these), an
  uncanonicalisable path, and a client call to an absolute host.
- **Edges** — `HttpCall` from the client file to the route's handler function,
  with provenance `route-matcher`, the specification's uncalibrated priors
  (0.98 exact, down to 0.65 wildcard, × 0.8 for an undeclared method), and
  evidence naming both sides and the rules that fired.
- **CLI** — `cartograph match <path> [--json]` reports every decision including
  the refusals, because a resolver is only as trustworthy as what it declines
  to claim.
- **Tests** — 38 matching, 12 end-to-end cross-stack fixtures (the
  specification's CheckoutButton → create_order chain among them); 236
  workspace tests total.
- **Benchmarks** — deterministic `matching` group covering each outcome,
  index construction and scaling.

**Deliberately not added.** No LSP: route matching joins observations, not
symbols, and M02 already extracts the handler syntactically. No OpenAPI
confirmation yet — named in the milestone definition, deferred so the
route-matching core landed first. No ORM, no constant propagation, no new
dependency of any kind.

### Added — M03, canonical route normalisation

**Both sides of the eventual join now speak one language.** `cartograph-resolver`
canonicalises a route declaration or a client call URL into a `CanonicalRoute`.

- **Canonical model** — `Segment` (`Static` / `Parameter` / `Dynamic` /
  `Wildcard`), `Methods` (`Unknown` or `Declared`), `CanonicalPath` with
  scheme, authority, query, fragment and trailing-slash preserved,
  `NormalizationStatus`, `NormalizationNote` and `RouteProvenance`.
- **Dialects** — colon (`:id`), curly (`{id}`, `{id:int}`), angle (`<id>`,
  `<int:id>`, `<uuid:id>`, `<slug:s>`), path converters and `*`/`**` catch-alls,
  and templates. Converter order differs between Flask/Django and Starlette;
  both are read correctly. All parameter dialects share the shape `/orders/{*}`.
- **Methods** — case-insensitive, deduplicated, deterministically ordered. A
  missing method is `Unknown`, **never defaulted to GET**.
- **URL decomposition** — scheme, authority, query and fragment separated from
  the path. Query parameter names never become path parameters.
- **Templates** consumed structurally, never evaluated; a leading substitution
  flags the prefix unknown rather than pretending `` `${BASE}/orders` `` is
  `/orders`.
- **Refusals over guesses** — Django `re_path` regexes and dynamic paths
  (`'/' + param`) are `Unsupported` with the raw form retained. "Cannot safely
  normalise" is a usable fact; "probably this route" is not.
- **CLI** — `cartograph normalize <path> [--json]` shows each observation's raw
  form beside its canonical form, and states in its own output that client
  calls are not matched against routes.
- **Tests** — 59 canonicalisation tests asserting exact canonical output, a
  large share of them negative; 191 workspace tests total.
- **Benchmarks** — deterministic `canonicalization` criterion group.

**The M04 boundary is the point of this milestone.** Canonicalisation is
per-observation: a `CanonicalRoute` has five fields and none can reference
another observation, so no match can be expressed. `same_shape` and
`compatible_method` are structural predicates — two unrelated services can
share a shape — and answering "could these be comparable?" is not answering
"do these connect?". No edge, no confidence, no cross-language claim exists.

`HttpMethodHint::from_name` became public so the resolver and parser share one
method-name table rather than maintaining two that could disagree.

No new external dependency: the resolver uses only `cartograph-parser` and
`serde`, with URL decomposition hand-written against the standard library.

### Added — M02, Python extraction

**A second language into the same fact model.** `cartograph-parser` now
extracts Python with tree-sitter, as a peer of the TypeScript extractor behind
one `Analyzer::dispatch`; both share `src/syntax.rs` so a span means the same
thing in both languages.

- **Imports** — `import x`, `import x as y`, `from m import a`, aliases,
  relative (`.`, `..`) and wildcard forms. Specifiers stay unresolved strings.
- **Symbols** — functions, `async def`, classes, methods, nested functions,
  module-scope variables, with enclosing-symbol tracking. **No invented export
  semantics**: Python symbols are `NotApplicable` rather than `NotExported`,
  except where a module declares `__all__`, which is a real export list.
- **Decorators** — recorded as observations on the symbol they decorate.
- **Calls** — `foo()`, `obj.foo()`, `pkg.mod.fn()`, `ClassName()` (recorded
  with `is_new = false`; Python has no `new`). Subscript and call-of-call
  callees are skipped rather than guessed.
- **Strings** — literals including triple-quoted, f-strings with substitution
  structure preserved verbatim, `+` concatenation chains. `f"/orders/{id}"`
  stays symbolic; evaluating it is M05's work.
- **Route observations** — a new shared `RouteObservation` carrying declaration
  style, declared methods, the path *verbatim* and the handler. Verb decorators
  (`@app.get`), method-list decorators (`@app.route(..., methods=[…])`) and
  URL-conf entries (`path()`, `re_path()`) are all recognised. Declared methods
  are a list because Flask genuinely declares sets; an empty list means the
  source did not say and is never defaulted to GET.
- **HTTP observations** — `requests`/`httpx`/`session`/`client`/`http`/
  `aiohttp` verbs, plus any receiver with a path-like first argument. Dict and
  environment lookups (`config.get`, `os.environ.get`) are excluded.
- **CLI** — `cartograph parse` detects Python automatically and walks mixed
  `.ts`/`.tsx`/`.py` trees in one pass, reporting route counts explicitly as
  observations rather than endpoints.
- **Tests** — 47 Python integration tests over a 32-category fixture corpus;
  132 workspace tests total, with every M00/M01 test still green.
- **Benchmarks** — deterministic `py_extraction` criterion group.

**Correction driven by evidence.** The route fact originally carried a
`RouteFramework` (`FastApi`/`Flask`/`Django`). Running the extractor over
Flask's own repository produced twenty routes labelled FastAPI, because Flask
2.x supports `@app.get(...)` with identical syntax — one of them
`@app.get("/result/<id>")`, carrying Flask's `<id>` dialect under a FastAPI
label. The field is now `RouteDeclarationStyle`, naming the syntax actually
matched, and M03 is directed to canonicalise from the path syntax itself.

Dependency added: `tree-sitter-python` 0.25 — the Python grammar, required by
this milestone's parsing work. No LSP, no Pyright, no other new dependency.

### Added — M01, TypeScript extraction

**The first real analyzer.** `cartograph-parser` now extracts syntactic facts
from TypeScript and TSX using tree-sitter (the runtime is contained in one
private module; the public API is Cartograph's own language-neutral fact
model):

- **Imports** — default, namespace, named (+aliases), type-only, side-effect.
- **Symbols** — functions (+async), classes, methods, module-scope variables,
  interfaces, type aliases, enums; enclosing-symbol tracking; export status
  including `export {…}` clauses and anonymous `export default` expressions.
- **Call sites** — `foo()`, `a.b.c()`, `new Foo()`, argument counts.
  Computed/unreliable callees are skipped, not guessed.
- **Strings** — literals (escapes as written), template literals with full
  substitution structure preserved for M05's evaluator (never pre-resolved),
  flattened `+` concatenation chains.
- **HTTP observations** — `fetch`/`axios.*`/known-client verb calls recorded
  as *syntactic observations* with method hints (never defaulted) and
  literal/template/dynamic URL shapes. Observations are explicitly not
  `HttpCall` edges; the resolver creates edges at M04.
- **Diagnostics** — error-tolerant extraction: broken regions become
  structured, source-text-free diagnostics; the rest of the file (and the
  repository walk) continues. Non-UTF-8 and unreadable files fail softly.
- **CLI** — `cartograph parse <path> [--json]` walks a file or tree and
  reports facts and diagnostics. `analyze` remains absent until M09.
- **Tests** — 28 parser integration tests over a fixture corpus (all 22
  mandated syntax categories), 4 new CLI tests; 82 workspace tests total.
- **Benchmarks** — Criterion `ts_extraction` group (single small file,
  synthetic 50/500-function modules, 100-file synthetic project), fully
  deterministic in-memory inputs.

**Checkpoint policy repair (ADR-0010).** Provisional checkpoints
(`cartograph-mNN-rcK`, exact PR head) are now distinct from accepted
checkpoints (`cartograph-mNN`, exact merge commit, created only at human
acceptance). The premature `cartograph-m00` tag was retired after a safety
determination and replaced by `cartograph-m00-rc1`.

Dependencies added: `tree-sitter` 0.26, `tree-sitter-typescript` 0.23 —
required by this milestone's actual parsing work. No LSP, no Python grammar,
no future-milestone dependency.

### Added — M00, engineering foundation

**Rust workspace** with six crates:

- `cartograph-core` — the domain model. Node and edge kinds from the frozen
  specification, `Confidence`, `Provenance`, `Evidence`, `SourceLocation`,
  `CommitId`. Edges cannot be constructed without provenance, confidence and
  evidence; the invariant is enforced by the type system rather than by review.
- `cartograph-graph` — the architecture graph over `petgraph::StableDiGraph`,
  with traversal in both directions. No petgraph type appears in the public API.
- `cartograph-cli` — the `cartograph` binary. `cartograph version`, with
  `--json`.
- `cartograph-testkit` — fixtures and builders, including the specification's
  cross-stack chain as fixed test data.
- `cartograph-parser`, `cartograph-resolver` — reserved and deliberately empty.
  They arrive at M01–M02 and M03–M06 respectively.

**Validation**

- 50 tests across unit and integration levels.
- Criterion benchmark harness with graph-construction and traversal benchmarks.
  No result is published; the harness exists so M10 has a baseline to compare
  against.
- Workspace lints: `unsafe_code` forbidden, `missing_docs` warned, clippy
  `all` + `pedantic`.

**Governance**

- AgentOS v1.0.0 vendored under `agentos/` and adapted for Cartograph.
- Quality gates QG-001 – QG-008 with an executable runner.
- Milestone definitions M00 – M17, machine-readable project state, checkpoint
  system.
- Nine architecture decision records.
- GitHub Actions CI, pull request template, issue templates, CODEOWNERS.

### Deliberately not included

No parsing, no resolution, no LSP, no storage, no desktop application, no MCP,
no AI, no incremental analysis. Those belong to later milestones and adding them
early would break the gate model that makes rollback possible.

### Known limitations

- Node identity is graph-local and not content-addressed. Handles are not
  portable between graphs. Content-addressed identity arrives at M10.
- Confidence values are the specification's uncalibrated starting priors. No
  accuracy claim rests on them until M08 fits them against a labelled benchmark.
- `Evidence` is a validated string. Structured evidence arrives at M04, when
  there is a real resolver to describe.
- The repository's default location is an exFAT volume, which has no hard links;
  Cargo falls back to copying its incremental cache. See
  `docs/development/external-storage.md`.
