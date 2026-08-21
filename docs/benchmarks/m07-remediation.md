# M07 remediation — accuracy-first cross-stack resolution

**Status: INTERNAL.** The original M07 result is unchanged and remains the
record of what the accepted engine did: [m07-report.md](m07-report.md). This
document reports what the remediation changed and what it cost. Neither set of
numbers is for publication.

Baseline `5cdafca` on `feature/m07-real-validation`, itself based on
`cartograph-m06` (`ef26ae2`). Same seven repositories, same commits, same
ground truth, same denominators.

---

## The question

M07's validation was complete and its product question was not passed. It found
six chains, none beginning at a frontend file, and named three causes: frontends
call through client wrappers, router prefixes are composed at registration, and
model names were resolved without consulting imports.

This remediation addresses all three. It was measured against one criterion
written from source **before any code changed**
(`benchmarks/remediation/acceptance-target.json`): recover the chain from
Airflow's pool creation, and lose nothing.

## The answer

**Three complete chains now run from a React component to a database table**,
each verified line by line against source.

```text
ParseDagButton.tsx:30      ──call──►  useDagParsing.ts:30
  ──call──►  queries.ts:2546          ──call──►  services.gen.ts:4538
  ──http-call──►  dag_parsing.py:32 (reparse_dag_file)
  ──orm-access──►  DagPriorityParsingRequest (dagbag.py:267)
  ──queries──►  dag_priority_parsing_request
```

| Origin | Handler | Model | Table |
|---|---|---|---|
| `ParseDagButton.tsx:30` | `reparse_dag_file` | `DagPriorityParsingRequest` | `dag_priority_parsing_request` |
| `AddConnectionButton.tsx:29` | `post_connection` | `Connection` | `connection` |
| `AddPoolButton.tsx:28` | `post_pool` | `Pool` | `slot_pool` |

All three begin in `airflow-core/src/airflow/ui/src/` — hand-written React
components, not tests, not generated code, not a Python client. Every edge
carries confidence, provenance, evidence and a source location; every node is
the declaration it names; the path traverses through shared node ids.

Counting each component and its hook separately, six chains have a hand-written
frontend origin, out of 24 complete chains in the corpus.

---

## What the baseline actually was

**Five of M07's six chains were false positives.** They ran through
`providers/edge3/.../worker_api/routes/ui.py:135`, where `Job(...)` constructs a
*response datamodel* imported from a FastAPI models module — not
`airflow.jobs.job.Job`, the SQLAlchemy model of the same name. Import
resolution removed them.

The honest baseline is therefore **one** chain, not six: Onyx's `create_group`
building a `UserGroup` imported from `onyx.db.models`. It is real, it survives,
and it still does not begin at a frontend.

This correction makes M07's negative finding stronger, not weaker.

---

## Results

| | M07 baseline | Remediation |
|---|---|---|
| Route declarations — TP / FP / FN | 101 / 0 / 0 | **101 / 0 / 0** |
| Route precision / recall | 1.000 / 1.000 | **1.000 / 1.000** |
| `Queries` — TP / FP / FN | 48 / 0 / 0 | **48 / 0 / 0** |
| `Queries` produced / source-confirmed | 290 / 290 | **290 / 290** |
| `OrmAccess` edges | 11,843 | **11,464** |
| …unconfirmable against source | 286 | **126** |
| `HttpCall` edges | 1,458 | **1,737** |
| `Call` edges (new) | — | **1,445** |
| Complete chains | 6 (5 false) | **24** |
| …from a frontend file | **0** | **6** |
| `must_not_produce` violations | 0 | **0** |
| Edges missing an attribute | 0 | **0** |

379 `OrmAccess` edges were **removed**, every sample checked being a symbol
that merely shared a model's name. That is the point: this remediation
subtracted more relationships than most of its slices added.

### What is still unmeasured

`HttpCall` recall, `OrmAccess` recall and chain recall remain without an
independently established denominator, exactly as M07 reported. Nothing here
changes that, and the precision figures above must not be read as if it did.
Establishing those denominators is future work FW-10, still open.

---

## What changed, and why

### 1. Import resolution

A name at a use site is walked through the file's own imports to a declaration
— import, module, exported symbol, declaration — instead of being matched
against a project-wide map. Unresolvable modules, monorepo path collisions and
re-export cycles refuse with a reason. Documented in
[../resolver/imports.md](../resolver/imports.md).

*Found by:* 286 unconfirmable accesses at M07.
*Costs:* 379 accesses removed, all sampled ones verified wrong.

### 2. Configuration-object requests

`__request(OpenAPI, {method, url})` and `client.get({url})` are read. `url` and
`endpoint` only — never `path`, which is how every router in the corpus
describes a screen.

*Found by:* twelve TypeScript clients among 1,458 edges.

### 3. Router prefix composition

`@pools_router.post("")` on a router carrying `/pools`, mounted under one
carrying `/api/v2`, serves `/api/v2/pools`. Any constructor whose name ends in
`Router` counts. Documented in [../resolver/routers.md](../resolver/routers.md).

*Found by:* `OUT-5` in M07's supported subset.

### 4. The client wrapper as a node

An `HttpCall` edge is sourced at the function issuing the request, and calls
reaching that function become `Call` edges resolved through imports. Decided in
[ADR-0012](../adr/ADR-0012-the-client-wrapper-is-a-node.md).

---

## Costs, stated plainly

**175 Airflow `HttpCall` edges were lost** and 151 gained. The lost ones were
Python tests calling `test_client.get("/assets/aliases")`, where the client is
built with `base_url=f"{BASE_URL}{get_api_path(request)}"`. Before composition
those matched because the route was *also* stored as a fragment; a fragment
matching a fragment. The path is relative to a runtime-configured base, which
is M07's declared `SR-1` refusal category, so declining them is the behaviour
the subset already specified.

**A matcher rule was tried and reverted.** Three Superset edges join a frontend
unit test mocking `/test` to a backend unit test serving `/test` — one generic
segment, no method on either side. A rule refusing that also refused `/orders`
with no declared method, which M04 correctly accepts. Losing correct edges to
remove wrong ones is not a trade this remediation will make; the three remain a
known false-positive class.

**Analysis is 11% slower** — 68.4 s to 75.8 s across 58,225 files — and peak RSS
rose from 825 MB to 965 MB on PostHog. Import resolution is memoised per
(file, name); call resolution runs once over TypeScript files. Neither is
optimised, and the memory trend remains future work FW-9.

---

## Still not solved

| # | Finding |
|---|---|
| FW-1 | Onyx's frontend calls `/api/manage/...` while its backend composes `/manage/...`: the `/api` comes from a Next.js `rewrites()` mapping, a deployment-time mechanism |
| FW-2 | Onyx's global prefix is applied by a project-local helper, not `include_router`, so it does not compose |
| FW-3 | Django chains still break: the `HttpCall` target is a `Function` in the URL-conf file, the `OrmAccess` source a `Function` in the view file |
| FW-5 | 126 `OrmAccess` edges remain unconfirmable — names bound by star or conditional imports, which are permitted rather than refused |
| FW-6 | Class-based views and DRF routers still carry most of PostHog's and Superset's API |
| FW-7 | `session.query(Model)` and `select(Model)` remain unsupported |
| FW-10 | `HttpCall`, `OrmAccess` and chain recall still have no independent denominator |

full-stack-fastapi still produces zero edges: its ORM is SQLModel, out of scope,
and its frontend's URLs are now read but its backend routes compose through
`include_router(items.router)` — an attribute, not a plain name — which router
resolution does not follow.
