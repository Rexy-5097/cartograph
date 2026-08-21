# M07 — Full-stack verification on real repositories

**Status: INTERNAL.** The methodology below is first-generation and changed
twice while it was being used. Nothing here is a published benchmark result,
and no number in this document should be quoted outside the project.

Base checkpoint `cartograph-m06` (`ef26ae27143ff8a8f1e49b5a618ad5a615092cc8`).
Toolchain rustc 1.97.1, macOS 26.5.2 arm64, Python 3.14.4.

---

## The question, and the answer

M07 exists to answer one question:

> Does Cartograph correctly recover the complete frontend → HTTP → backend →
> ORM → table chain on real repositories, within the currently supported
> subset?

**No.** Across seven real repositories and 58,225 analysed files, Cartograph
produced **six** fully verified chains, and **none of them begins at a frontend
file**. Five start in Airflow's Python test suite and CLI, one in Onyx's Python
integration tests.

The individual links are accurate. The composition of them — the thing the
product exists for — was not demonstrated on any real repository.

| | |
|---|---|
| `HttpCall` edges produced | 1,458 |
| …whose client is a TypeScript file | **12** (0.8%) |
| …whose client is non-test TypeScript | **8** (0.5%) |
| Fully verified chains | **6** |
| …beginning at a TypeScript file | **0** |

The 1,446 remaining `HttpCall` edges are Python-to-Python: test suites calling
the API they test. Those are real relationships and the evidence for them is
sound, but they are not the cross-language claim.

### Why

Three causes, all previously documented, none discovered by surprise:

1. **Real frontends do not write URLs at call sites** (`OUT-6`). Every
   frontend in the corpus reaches its backend through a generated or
   hand-written client: `full-stack-fastapi` through a hey-api SDK, Airflow
   through openapi-typescript, Superset through `SupersetClient`, PostHog and
   Onyx through their own `api.ts` wrappers. The URL lives inside the wrapper,
   one interprocedural step away from the call. Cartograph reads call sites.
2. **Prefix composition** (`OUT-5`). `APIRouter(prefix="/manage")` means the
   path a decorator states is a suffix of the path the server answers on. Even
   a frontend that did write a literal URL would not match.
3. **Django's chain breaks at node identity** (`GAP-4`). The `HttpCall` edge
   targets a `Function` node located in the URL-conf file; the `OrmAccess`
   edge sources a `Function` node located in the view file. Under ADR-0011 a
   node is `(kind, name, file)`, so these are two nodes and the chain has no
   middle. Both Django repositories produced zero chains.

None of these is a bug to be fixed in M07. All three are recorded as future
work below.

---

## Corpus

Seven repositories, pinned to exact commits, analysed at their repository root.
No source is copied into this repository; only measurements and ground-truth
records are stored. Analysing a hand-picked subtree would let scope be tuned
against results, so the analyser sees the whole repository.

| Repository | Commit | Frontend | Backend | ORM |
|---|---|---|---|---|
| `apache/superset` | `8222db334054172bf44789c58ab3857e4567b952` | React + TS | Flask (+ Flask-AppBuilder) | SQLAlchemy |
| `fastapi/full-stack-fastapi-template` | `162344da111e833b30892728372ab95331f06873` | React + TS | FastAPI | SQLModel |
| `onyx-dot-app/onyx` | `8ee706ebaa32804ae7d674bfb779f1b43609b1e4` | Next.js + TS | FastAPI | SQLAlchemy 2.0 |
| `PostHog/posthog` | `2dd790a6e7c3d3ab25ff8f45f07b0d0250de4c25` | React + TS | Django (+ DRF) | Django ORM |
| `zulip/zulip` | `0ce8f6278cdeacdc9b26dba6b08a63e567df87ac` | TypeScript | Django | Django ORM |
| `apache/airflow` | `9b43d6abc0fc0766b1b1a886657bb6befc33ee1d` | React + TS | FastAPI + legacy Flask | SQLAlchemy |
| `Significant-Gravitas/AutoGPT` | `601093ddfe23a3d58a9c8f4a208bd49b203ee612` | Next.js + TS | FastAPI | Prisma |

Three were chosen because they contain patterns known to be out of scope:
`full-stack-fastapi` (SQLModel), AutoGPT (Prisma), Airflow (a monorepo with two
coexisting web frameworks and a generated client). Superset was kept even
though its API is almost entirely Flask-AppBuilder `@expose`, which nothing in
Cartograph reads.

---

## Method

**Scope was declared before measuring.** `benchmarks/supported-subset.json` was
committed in `70e5ede`, before the analyser had been run against any corpus
repository. Its first version defined "supported" as the set of patterns
`cartograph-parser` recognises, which makes recall 1.0 by construction — a
pattern the analyser misses would have been defined out of its own denominator.
It was rewritten around framework capability families before any result
existed, with a `known_implementation_gaps` section predicting seven specific
misses in advance so that none could later be reclassified as out of scope.

**Ground truth is independent of output.** It is drafted by
`benchmarks/draft_ground_truth.py`, which is handed a filesystem path, cannot
import the analyser, and never reads its output. Its route patterns describe
what Flask, FastAPI and Django serve, deliberately over-approximating, so a
declaration Cartograph cannot see is still visible. Sampling is stratified by
declaration form and fixed in advance.

**Two kinds of check.** Ground-truth checks cover a sample. Source checks
re-read the corpus and verify *every* produced edge — a precision figure
computed only over sampled edges would leave unexamined false positives outside
the ratio.

**Denominators.** Recall is over in-scope ground-truth records only.
`UNSUPPORTED`, `SAFE_REFUSAL` and `AMBIGUOUS` records are excluded from the
denominator and their counts are reported beside it, so the exclusion is
visible and checkable. `benchmarks/evaluate.py` asserts that the recall
denominator equals the in-scope record count, and QG-009 asserts it again.

---

## Results

### Pass 1 → Pass 2

Pass 1 measured the accepted M06 engine and changed nothing. Five defects were
then fixed, each reduced to a fixture. Pass 2 re-ran the entire corpus.

| | Pass 1 | Pass 2 |
|---|---|---|
| Route declarations — TP / FP / FN | 97 / 0 / 4 | **101 / 0 / 0** |
| Route precision / recall / F1 | 1.000 / 0.960 / 0.980 | **1.000 / 1.000 / 1.000** |
| `Queries` (model → table) — TP / FP / FN | 35 / 0 / 13 | **48 / 0 / 0** |
| `Queries` precision / recall / F1 | 1.000 / 0.729 / 0.843 | **1.000 / 1.000 / 1.000** |
| `Queries` edges produced / contradicted by source | 254 / 0 | 290 / 0 |
| `OrmAccess` edges produced | 11,957 | 11,843 |
| …from a non-Python file | **33** | **0** |
| …naming a symbol the file imports from elsewhere | 399 | 286 |
| `HttpCall` edges | 1,463 | 1,458 |
| Fully verified chains | 6 | 6 |

Supporting counts (Pass 2): 112 route records and 13 ORM records classified
`UNSUPPORTED`; 26 ORM records classified `SAFE_REFUSAL`; 1,886 client calls
resolved to `Ambiguous` and produced no edge; 13 `must_not_produce` assertions,
0 violated; 0 edges missing confidence, provenance, evidence or a location.

### What these numbers do and do not cover

The 1.000s are narrow, and reading them as "the engine is correct" would be
wrong. Precisely:

- **Measured with precision *and* recall**: route declaration extraction (101
  in-scope records), and model → table resolution (48 in-scope records).
- **Measured with full-coverage precision only**: `OrmAccess` and `HttpCall`.
  Every produced edge was checked against source; no ground-truth recall
  denominator exists for either.
- **Not measured at all**: `HttpCall` recall, `OrmAccess` recall, and chain
  recall beyond the six found. Four of the seven predicted implementation gaps
  (`GAP-2` SQLAlchemy `session.query`, `GAP-3` inherited abstract bases,
  `GAP-6` project-wide name collisions, `GAP-7` custom managers) would surface
  in `OrmAccess` recall, which this pass does not compute. **Their absence from
  the tables above is a gap in the evaluation, not evidence of their absence
  from the engine.**

---

## Defects found and fixed

Each was found by reading output, not by a failing test. Each has a fixture.

1. **`Model` is not a Django base** — `crates/cartograph-resolver/src/orm.rs`.
   Django, Flask-SQLAlchemy and Flask-AppBuilder all use a base named `Model`.
   Testing Django first meant every Superset and Airflow model was searched for
   a `Meta.db_table` it does not have while its `__tablename__` sat a line
   below. Superset resolved 3 tables out of 33. Flavour is now decided by what
   the class declares; a class declaring neither says so. Superset 3 → 32.
2. **ORM discovery ran over TypeScript** — `orm.rs`. `new Theme({config})` in a
   Superset `.tsx` file and `new Event(...)` in a Zulip web module produced
   `OrmAccess` edges to Python models sharing the name. 33 edges, now 0.
3. **Model names resolved without imports** — `orm.rs`. Airflow's architecture
   diagrams call `User("DAG Author")` having imported `User` from the
   `diagrams` package; 218 were attributed to the Flask-AppBuilder model. An
   imported name must now come from a module the project contains. Names with
   no import binding them are left alone, because star and conditional imports
   are ordinary and refusing them would trade these false positives for false
   negatives.
4. **Route decorators required a listed receiver** —
   `crates/cartograph-parser/src/python.rs`. Superset's four
   `@health_blueprint.route(...)` declarations were invisible: the only route
   misses left in the corpus. A blueprint may be bound to any name; the
   evidence is a literal URL path in the first argument, the same rule the
   TypeScript side uses to tell `svc.get("/orders")` from `cache.get(key)`.
5. **TypeScript route declarations read as client calls** —
   `crates/cartograph-parser/src/typescript.rs`. `app.get('/_health', (c) =>
   ...)` is Express and Hono *declaring* a route. Three such declarations in
   PostHog's Node services were matched to a Python health endpoint in an
   unrelated service. An inline handler argument now marks a registration — but
   only for an unknown receiver, since Node's `http.get(url, callback)` is a
   real request.

### Defects found in the benchmark instrument, not in Cartograph

Recorded because three of them corrected numbers in Cartograph's favour, which
is the direction that deserves the most scrutiny.

- GAP-1 was being charged against Django `path()` entries, which have no
  decorator receiver.
- Django handlers were read as "the next `def` in the file", which attributed
  an unrelated admin helper to a PostHog route.
- ORM flavour was read from the base name, mislabelling all 32 Superset and all
  61 Airflow models as Django.
- **A multi-line `class SqlaTable(` did not close the previous class's scope**,
  so its `__tablename__` was recorded against `SqlMetric` forty lines earlier.
  Cartograph was right and the ground truth was wrong; one record was affected.
- **Route declarations inside docstrings were counted as routes.** Onyx and
  AutoGPT document their dependencies with `@router.post("/tokens")` in a
  docstring. Cartograph, which parses, ignored all three; the instrument
  charged it three false negatives for correctly declining to extract
  documentation.
- An AutoGPT test calling `resolve_sandbox_path(...)` was read as a URL-conf
  entry, because the callee's name ends in `_path` and its argument begins with
  a slash. URL-conf helpers are now matched by name, not by suffix.

---

## Known false positives, not fixed

**286 `OrmAccess` edges (2.4% of 11,843)** name a symbol that the access file
imports from a module which is not the model's own. The import rule added in
fix 3 removes the third-party cases; these are project-internal modules where
distinguishing a re-export from a different symbol of the same name requires
resolving the import, which no milestone has claimed. They are reported rather
than suppressed.

**A route registered with a named handler** — `router.get('/items',
handleItems)` — remains indistinguishable from `client.get(url, config)`. There
is a test asserting this boundary rather than papering over it.

---

## Ambiguity and refusal

Both behave as the specification requires, on real input.

- **1,886** client calls resolved to `Ambiguous` and produced **no edge**,
  Airflow contributing 1,358 of them. Candidates are retained and reported.
- **`full-stack-fastapi` produced zero edges from 23 routes and 63 client
  calls.** Its frontend is entirely a generated SDK whose calls read
  `(options.client ?? client).post({ url: '/api/v1/...' })` — a parenthesised
  callee with an options object, refused twice over. Its 13 SQLModel classes
  produced no model, no table and no access, satisfying all 13
  `must_not_produce` assertions.
- **Zulip declares 74 Django models and 1 literal `db_table`.** Exactly one
  table was resolved. The other 73 are `TableName::Unresolved` because Django's
  default name needs an app label that is not in the model file — the required
  refusal (`SR-6`), at scale, on a real repository.
- **PostHog resolved 47 tables from 110 models** for the same reason.

---

## Performance baseline

Cold, single run, release build. Not optimised; recorded as a baseline.

| Repository | Files | Nodes | Edges | Analysis | Wall | Peak RSS |
|---|---|---|---|---|---|---|
| superset | 6,063 | 563 | 613 | 4.7 s | 5.2 s | 132 MB |
| full-stack-fastapi | 156 | 0 | 0 | 0.06 s | 0.1 s | 9 MB |
| onyx | 5,747 | 1,018 | 1,048 | 3.7 s | 4.1 s | 111 MB |
| posthog | 32,335 | 5,885 | 7,620 | 42.6 s | 44.7 s | 825 MB |
| zulip | 1,539 | 1,158 | 1,345 | 2.2 s | 2.5 s | 76 MB |
| airflow | 8,654 | 2,167 | 2,658 | 7.6 s | 8.5 s | 218 MB |
| autogpt | 3,731 | 86 | 307 | 2.9 s | 3.2 s | 96 MB |
| **Total** | **58,225** | | | | **68.4 s** | |

Roughly 850 files/second. PostHog's 825 MB peak is the number to watch: every
`FileAnalysis` is held in memory at once, because dynamic URL resolution needs
the whole project's exported constants before any URL is resolved. At ten times
PostHog's size that design does not hold.

---

## Security validation

The benchmark machinery does not execute repository code. Its only
subprocesses are the `cartograph` binary, `rustc` and `git`; it imports nothing
from the corpus; it makes no network calls; the analyser reads no environment
variable of the analysed project. 58,225 files of untrusted public source were
parsed and 7/7 repositories completed — malformed input produces diagnostics,
not a failed run.

---

## Reproducing

```
python3 benchmarks/run_benchmark.py --mirror <corpus-dir> --raw <raw-dir> \
    --pass-number 2 --out benchmarks/results/m07-pass2-measurements.json
python3 benchmarks/evaluate.py \
    --measurement benchmarks/results/m07-pass2-measurements.json \
    --mirror <corpus-dir> --out benchmarks/results/m07-pass2-evaluation.json
```

Each mirror must be at the exact commit in `benchmarks/corpus.json`; the runner
verifies this and refuses otherwise. Results record the corpus digest, the
supported-subset digest, a digest per ground-truth file, the analyser's own
commit and the binary's digest.

---

## Future work

None of these is started. Each is a consequence of a measurement above.

| # | Finding | Would need |
|---|---|---|
| FW-1 | Frontends call through client wrappers, so no URL appears at the call site — the single largest cause of zero frontend-rooted chains | Interprocedural resolution of a wrapper's URL argument |
| FW-2 | `APIRouter(prefix=…)`, `Blueprint(url_prefix=…)` and `include()` compose the served path at registration | Resolving registration sites, not just declaration sites |
| FW-3 | Django chains break because the route's handler node and the view's handler node differ by file | Resolving a URL-conf view reference to its defining file |
| FW-4 | `views.create_order` as a handler name cannot equal `create_order` | The same resolution as FW-3 |
| FW-5 | 286 `OrmAccess` edges cannot be confirmed without resolving imports | Python import resolution |
| FW-6 | Class-based views and DRF routers carry most of PostHog's and Superset's API | A registration-based route model |
| FW-7 | `session.query(Model)` and `select(Model)` are unsupported because M02 records argument counts, not identities | Recording call-argument identities |
| FW-8 | Models inheriting a project-local abstract base are not recognised | Following inheritance across files |
| FW-9 | Peak RSS scales with project size because every `FileAnalysis` is retained | Streaming, or a two-phase constant pass |
| FW-10 | `OrmAccess` and `HttpCall` recall are unmeasured | Ground-truth records for access sites and client calls |
