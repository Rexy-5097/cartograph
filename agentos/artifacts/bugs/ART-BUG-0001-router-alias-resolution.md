---
id: ART-BUG-0001
title: "BUG-0001 A router imported under an alias lost its inclusion and every prefix composed through it"
version: 1.0
status: completed
owner: qa-reviewer
created: 2026-10-29
modified: 2026-10-29
related_adr: None
related_standard: standards/testing.md
related_checklist: QG-007
related_workflow: bug_fix.md
related_agent: qa-reviewer
---

# Bug Report: BUG-0001

## Description

`resolve_router` (`crates/cartograph-resolver/src/routers.rs`) resolved an
imported router to the module that declares it, then looked it up there by
the name it was *imported as*. With
`from polar.customer.endpoints import router as customer_router`, it asked
`endpoints.py` for `customer_router`; that module declares `router`. The
inclusion was silently dropped, and with it every prefix composed through it —
on Polar, the `/v1` under which the whole public API is served.
`Resolution::Declared` carried no declared name, so no caller could do better.

Found out of sample on polarsource/polar `773cfcf`, a repository the authors
never tested, by the diligence audit of 2026-10-28 (`c34d98f`). Airflow names
its routers uniquely, which is why the benchmark never caught it.

## Steps to Reproduce

1. Declare `router = APIRouter(prefix="/customers")` with a route in one module.
2. In another, `from <module> import router as customer_router`, then
   `api = APIRouter(prefix="/v1")` and `api.include_router(customer_router)`.
3. Run `cartograph match` over both: the route is served at `/customers/…`,
   not `/v1/customers/…`, and a client call to `/v1/customers/…` matches nothing.

## Observed vs Expected Behavior

- **Observed:** on Polar, 79 `http-call` edges, 22 of them wrong: 20 back-office
  test calls joined to public handlers whose `/v1` prefix was missing, and 2
  test object literals with a `url:` key. 2,143 calls matched no route,
  including 380 of 401 front-end `/v1/…` calls.
- **Expected:** a route mounted through an aliased import is served at its
  composed path. A prefix that cannot be read still refuses rather than
  composing as empty (RULE 009).

## Diagnostics

- **Affected File(s):** `crates/cartograph-resolver/src/routers.rs` (`resolve_router`),
  `crates/cartograph-resolver/src/imports.rs` (`Resolution::Declared`)
- **Environment details:** Windows 11, Rust 1.97.1, upstream `main` `aa9462a`
- **Severity:** Major. Silent: a route at the wrong path produces confident
  wrong edges (Polar's back-office tests at 0.98) rather than an error.

## Fix

`Resolution::Declared` gains `name`, the name the declaration has in its own
file. Python and `TypeScript` resolution fill it on every hop, including
aliasing re-exports, and `resolve_router` looks routers up by it. Callers that
match `Declared { .. }` are unaffected.

## Not fixed here, and why: the application's own prefix

`app = FastAPI()` is not recorded as a router, so a prefix passed to
`app.include_router(…, prefix=…)` is not composed; the route keeps its own
router's prefix. Recording `FastAPI()` as the outermost router was implemented,
measured and withdrawn: test files build their own applications and mount
production routers on them, so those routers looked mounted twice and lost
their composed prefix. On Significant-Gravitas/AutoGPT `601093d`, relative to
the alias fix alone, that cost 15 correct edges (8 became ambiguous, 7 matched
nothing) for 14 gained. Composing an application's prefix needs a model of
which application serves which call — the same model a test client's binding
needs. A test pins today's refusal.

## Verification Test

- **Regression Test Path:** `crates/cartograph-resolver/tests/routers.rs`, section
  "Aliases (out of sample: polarsource/polar)": three tests that fail without
  the fix and pass with it — the Polar shape; an alias naming one of two routers
  in a module; an alias through an aliasing re-export — and one that pins the
  application limitation above.
- **Test Command:** `cargo test -p cartograph-resolver --test routers`
  (16 passed; with the fix set aside, the three alias tests fail).
- **Full suite:** `cargo test --workspace --no-fail-fast` — 1,186 passed, 0
  failed, 32 ignored (1,182 on clean `main`, plus four). fmt and clippy
  `-D warnings` clean; AgentOS validator 98/100; `run_gates.py` 9/9.

## Out-of-sample measurement (release builds, before → after)

Polar `773cfcf`, 4,720 files analysed:

| | Before | After |
|---|---|---|
| `http-call` edges | 79 | 1,736 |
| Cross-language edges | 4 | 322 |
| Calls with no matching route | 2,143 | 253 |
| Ambiguous calls (no edge drawn) | 60 | 293 |
| Back-office test → public handler (wrong) | 20 | 0 |
| Front-end test object with a `url:` key (wrong) | 2 | 1 |

- The remaining `url:` edge (`proxy.test.ts`, `{ url: '/start' }` joined to the
  back-office `POST /start`) is a separate defect: object literals read as
  calls. Its twin disappeared only because its route moved under `/v1`.
- A random sample of 20 of the 1,736 edges, each read against its call site and
  the route it landed on: 20 correct. The fixer's own reading, not an
  independent label; 20 cannot establish the ≥ 97% target. It shows no visible
  error class among the new edges.
- The rise in ambiguity is correct refusal. 119 of the 293 have two exact
  candidates, 110 of those a date-versioned handler (`list` and
  `list_v2027_01` both serve `GET /v1/products/`, chosen at run time by a
  header). The rest are routers whose prefix still does not compose, and
  parameterised paths fitting more than one route.

The benchmark corpus's FastAPI repositories, the only ones this code reaches:

| Repository | Before → after |
|---|---|
| apache/airflow `9b43d6a` | unchanged: 3,350 edges, 175 `http-call` — the committed M08 dataset still reproduces exactly |
| onyx-dot-app/onyx `8ee706e` | unchanged: 1,076 edges |
| fastapi/full-stack-fastapi-template `162344d` | unchanged: 1 edge (prefix from `settings.API_V1_STR`; front end through a generated client) |
| Significant-Gravitas/AutoGPT `601093d` | 434 → 441 `http-call`: 7 gained, 0 lost — `codex_test.py` calls now reach the Codex router under `/codex` |

Superset (Flask), PostHog and Zulip (Django) do not reach this code.
AutoGPT was analysed without one test file whose path exceeds Windows' limit,
in both runs alike.
