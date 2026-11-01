---
id: ART-BUG-0002
title: "BUG-0002 A Django include() was recorded as a route, and the entries it mounts were served at the wrong path"
version: 1.0
status: completed
owner: qa-reviewer
created: 2026-10-30
modified: 2026-10-30
related_adr: None
related_standard: standards/testing.md
related_checklist: QG-007
related_workflow: bug_fix.md
related_agent: qa-reviewer
---

# Bug Report: BUG-0002

## Description

`extract_django_route` (`crates/cartograph-parser/src/python.rs`) recorded
every `path(pattern, view)` as a route, including
`path("api/instances/", include("plane.license.urls"))`. Its handler was the
callable `include`. A frontend call to `/api/instances/` then matched the mount,
with confidence 0.784 ("route declared no method"), instead of the view behind
it. The entries `include` mounts stayed at the path they state:
`InstanceEndpoint` at `/` rather than `/api/instances/`. So calls to the real
endpoints matched nothing, or matched an unrelated route that happened to share
the unprefixed shape.

Found out of sample by the 2026-10-30 large-codebase assessment of `908e289`
(finding F4) on Plane and Label Studio, two repositories the engine had never
been run on.

## Steps to Reproduce

1. Root conf: `urlpatterns = [path("api/instances/", include("project.license.urls"))]`.
2. `project/license/urls.py`:
   `urlpatterns = [path("", InstanceEndpoint.as_view()), path("admins/", InstanceAdminEndpoint.as_view())]`.
3. A TypeScript client calls `axios.get("/api/instances/")` and
   `axios.get("/api/instances/admins/")`.
4. Run `cartograph match --json`.
   - The first call is `strong` against handler `include`.
   - The second matches nothing.

## Observed vs Expected Behavior

- **Observed (908e289):**
  - Plane: both of its `http-call` edges pointed at `include`.
  - Label Studio: 54 of 188 did.
  - `routes_composed` was 0 in all four Django repositories of the
    assessment.
- **Expected:**
  - A mount is not a route.
  - Each entry is served under the prefixes that mount it, composed as Django
    composes them, by string concatenation.
  - A prefix that cannot be read refuses and leaves the entry as written
    (RULE 009).

## Diagnostics

- **Affected file(s):**
  - `crates/cartograph-parser/src/python.rs`: `extract_django_route`, and
    spreads into `urlpatterns`;
  - `crates/cartograph-parser/src/model.rs`: `UrlConfInclusion`,
    `IncludedUrlConf`, `FileAnalysis::url_inclusions`;
  - `crates/cartograph-resolver/src/urlconf.rs` (new): `UrlConfIndex`;
  - `crates/cartograph-pipeline/src/pipeline.rs`: applied where receiver
    composition does not apply.
- **Environment:** Windows 11, Rust 1.97, upstream `main` `908e289`.
- **Severity:** Major.
  - Precision: every edge to `include` is wrong by construction.
  - Recall: every entry under a mount sat at the wrong path.
  - Silent: the wrong edges carried ordinary confidence.

## Fix

- **Parser.** `path(p, include(x))` is recorded as a `UrlConfInclusion`, with
  the prefix and the target, and no route. The target is one of:
  - a dotted module;
  - a name;
  - an inline list.

  A `(target, app_name)` tuple takes its first item. Spreads into the
  module-level `urlpatterns` (`[*analytic_urls]`, `+= [*more]`) are mounts
  with an empty prefix; this is how Plane assembles its API. A `re_path`
  prefix is read only when it is an anchored literal (`^`, `^api-auth/`),
  which is what Django consumes. Anything else is unresolved.
- **Resolver.** `UrlConfIndex` composes the prefix an entry is served under
  from the module it sits in and the innermost mounted list around it. It
  refuses on:
  - an unresolved prefix;
  - a module or list mounted twice;
  - a cycle;
  - a target it cannot find (DRF `router.urls`).

  A refusal leaves the entry as written. Regex entries are never composed.
  See `docs/resolver/urlconf.md`.
- **Independent corroboration.** The committed ground truth already
  classifies a `path(…, include(…))` entry as UNSUPPORTED, rule OUT-10, because
  it "declares no handler and serves no request itself"
  (`benchmarks/draft_ground_truth.py`, `benchmarks/ground-truth/posthog.json`).
  That ground truth was drafted without access to Cartograph output.

## Verification Test

- **Regression tests:**
  - `crates/cartograph-resolver/tests/urlconf.rs`: 11 tests (Plane,
    Label Studio and the Django documentation's forms; refusals for
    ambiguity, regex and variable prefixes, unmodelled targets and cycles).
  - `crates/cartograph-parser/tests/python_extraction.rs`: 3 tests.
- **Mutation check.** With `include` recognition disabled:
  - 9 of the 10 resolver tests then present failed;
  - the parser's include test failed;
  - the survivor pins a refusal, not the fix.
- **Full suite.** `cargo test --workspace`: 1,200 passed, 0 failed, 32
  ignored. That is 1,186 on `908e289` plus 14.
- **Other gates.**
  - fmt and clippy `-D warnings`: clean.
  - AgentOS validator: 98/100.
  - `run_gates.py`: 9/9.
  - docs build and link check: clean.

## Out-of-sample measurement (release builds, `908e289` → `776509c`)

Seventeen repositories from the 2026-10-30 assessment, at the same SHAs, with
the same `cartograph match --json` command and limits (10 GB job memory,
1,800 s). Compared by edge identity (kind, source and target kind, name and
file), as a multiset. Every run exited 0, with no timeout and no memory-limit
hit.

| Repository | `http-call` edges | Lost | Gained | Edges to `include` | Production frontend calls accepted | No match | Routes composed |
|---|---|---|---|---|---|---|---|
| makeplane/plane `b98e1de` | 2 → 263 | 2 | 263 | 2 → 0 | 2 → 236 | 393 → 58 | 0 → 376 |
| HumanSignal/label-studio `75bdd80` | 188 → 400 | 63 | 275 | 54 → 0 | 5 → 12 | 313 → 124 | 0 → 106 |
| paperless-ngx/paperless-ngx `e240c89` | 0 → 9 | 0 | 9 | 0 | 0 → 0 | 840 → 831 | 0 → 18 |
| getsentry/sentry `cf76d0c` | 17 → 17 | 0 | 0 | 0 | 0 → 0 | unchanged | 0 → 0 |
| the other 13 (FastAPI, Flask and stress repositories, including Polar and Airflow) | unchanged | 0 | 0 | 0 | unchanged | unchanged | unchanged |

- **Every lost edge was wrong when read against source.**
  - Plane's 2 and 54 of Label Studio's pointed at `include`.
  - Label Studio's other 9 joined `GET /api/tasks/{id}/` to
    `TasksBulkCreateAPI`, which is served at
    `api/projects/<pk>/tasks/bulk/`. The same calls now reach `TaskAPI` at
    `api/tasks/<int:pk>/`.
  - All 9 had the "crossed" shape: an unknown client segment against a route
    literal, and a route parameter bound to a client literal. Label Studio
    now has no crossed matches.
- **Sample of the gained edges.** A seeded sample of 12 of Plane's accepted
  matches was read against the source: 11 agree, and 1 is wrong. The wrong
  one, `DELETE /api/workspaces/{*}/importers/{*}/{*}/` →
  `FileAssetViewSet`, is crossed. This is the fixer's reading, not an
  independent label, and it cannot establish a precision figure.
- **New false edges exposed.** Paperless gained 9 edges.
  - 5 are exact `POST /api/token/` test calls.
  - 4 are a test looping over `correspondents`, `tags` and
    `document_types` (`f"/api/{endpoint}/"`). They now match
    `/api/token/` because a single unknown segment is accepted against a
    literal, and the real routes, DRF routers, are invisible.
  - The matcher rule predates this change. Composing paths gave it
    candidates to accept.
  - Across Plane, Label Studio and Paperless, 14 accepted matches now rest on
    an unknown-against-literal segment (Plane 10, all production, 5 of them
    crossed; Paperless 4, all tests). Before the fix there were 9, all
    crossed, all in Label Studio.
  - Recorded as a separate defect for the matcher; not fixed here.
- **Unchanged.** ORM edges are unchanged in all 17. Peak committed memory is
  within 3.1 MB in every repository.
- **Not re-measured.** The committed M07 benchmark's Django repositories,
  PostHog and Zulip, are not mirrored on this machine.
  `m07-pass2-evaluation.json` counts 5 PostHog records as
  UNSUPPORTED_BUT_PRODUCED; whether the two OUT-10 `include` records are
  among them was not determined.
