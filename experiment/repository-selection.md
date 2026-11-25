# Repository selection — M18 browser experiment

**Selected: [Flagsmith/flagsmith](https://github.com/Flagsmith/flagsmith)** at
`899aa383636d28d426e8e0645d0f06c9e63f60f0` (default branch `main`, committed
2026-11-24), BSD-3-Clause. It was the first unit in a seeded random order, and
it passed the fitness check stated before the draw.

Full working files, outside this repository:
`D:/WorkSpace/Projects/cartograph-reviews/2026-11-25-m18-browser-explorer/selection/`
(`candidate-pool.md`, `eligible.json`, `draw-procedure.md`).

## 1. Exclusions applied before building the pool

- **Named in the brief:** CVAT, Plane, Label Studio, Paperless-ngx.
- **Already exposed or sealed for Cartograph:**
  - every repository in the sealed holdout register and the validation exposure ledgers (`2026-10-30-correctness-recovery/`);
  - every candidate from the large-codebase assessment, with any decision, plus the cartobench manifests.

  This includes the clean four (InvenTree, Mathesar, Khoj, Skyvern) and the sealed five (Wagtail, Prefect, Feast, Mealie, DocsGPT). None of them was accessed; only their names were matched.

In total, 36 repositories are excluded by name. The full table with sources is in `candidate-pool.md` §1.

## 2. Pool and eligibility

- **Method.** Research only, through read-only `gh api` calls: repository metadata, languages, last commit, recursive tree, licence, and single files. No candidate was cloned or run while the pool was built.
- **Result.** 61 repositories were checked. 23 were ELIGIBLE and 38 INELIGIBLE; checking stopped at the first criterion a repository failed.

| ID | Criterion |
|---|---|
| C1 | React + TypeScript: at least about 100 non-vendored `.ts`/`.tsx` files and a React app |
| C2 | Python backend on Django, FastAPI or Flask, with real endpoints |
| C3 | ORM models (Django ORM, SQLAlchemy/SQLModel declarative) |
| C4 | Frontend-to-backend HTTP calls visible in committed source |
| C5 | Multiple application modules |
| C6 | Meaningful tests and docs |
| C7 | Clear OSI licence (the licence file was read when the API said NOASSERTION) |
| C8 | Active: a default-branch commit within about 6 months; not archived |
| C9 | At least about 100 stars |
| C10 | A real product, not a template or tutorial |
| C11 | About 150–3,000 Python+TS files; hard ceiling about 5,000 |

Flagsmith's row in the pool:
- 9,583 stars; BSD-3-Clause.
- Python 63%, TypeScript 32%.
- Django + DRF (`api/app/urls.py`).
- Django ORM: 62 files import `django.db.models`.
- React + TS: 784 `.tsx` files.
- Client side: RTK Query plus a legacy fetch helper.
- 392 test files under `api/tests`, and Playwright e2e tests.
- Caveat recorded *before* the draw: it is at the upper edge of the size target.

## 3. Random draw

The procedure was written to `draw-procedure.md` at 2026-11-25T09:38:46Z, before the draw was run.

- **Units.** The five `suitenumerique/*` repositories share one skeleton, so they were collapsed into one unit. That leaves 19 units, sorted alphabetically.
- **Seed text.** `cartograph-m18-draw:9ee0dea0b666e001ae176aa6857fc8c0d206ccaf`. The hash in it is `upstream/main` when the experiment began (the merge of PR #111). Anyone can recompute it, and I did not choose it.
- **Seed.** SHA-256 of that text: `b9c3923d3af7245ee55eec08f2a71d50f6d27cce736d553ef7141baa9706eb1a`.
- **Method.** Python `random.Random(int(seed_hex, 16)).shuffle(units)`.
- **Shuffled order.** Flagsmith/flagsmith → mozilla/pontoon → suitenumerique/\* → papermerge/papermerge-core → hotosm/drone-tm → keephq/keep → pinterest/querybook → topoteretes/cognee → apache/hamilton → kobotoolbox/kpi → OWASP/Nest → mozilla/fx-private-relay → bitmakerla/estela → eosphoros-ai/DB-GPT → sartography/spiff-arena → photonixapp/photonix → microsoft/magentic-ui → SecObserve/SecObserve → mediacms-io/mediacms.
- **Fitness check, fixed in advance:**
  1. a shallow clone succeeds and is pinned;
  2. `cartograph <repo> --json` completes;
  3. the graph has at least 500 nodes.

  It deliberately sets **no** requirement on which relationships are found, so the draw cannot favour flattering repositories.

**Result:** the first unit, Flagsmith, passed all three checks: clone pinned, analysis complete, 1,004 nodes. No unit was skipped.

## 4. Size and suitability

| Measure | Value |
|---|---|
| Tracked files (clone) | 4,350 |
| `.py` / `.ts` / `.tsx` / `.js` | 2,008 / 546 / 784 / 151 |
| Files Cartograph analysed | 2,755 (TS 546, TSX 784, Python 1,425) |
| Graph | 1,004 nodes, 1,092 edges |

Unfamiliarity: Flagsmith appears in no earlier Cartograph assessment, ledger or benchmark (checked against the exclusion sources above). Its source tree contains its own `AGENTS.md`/`CLAUDE.md`; those were treated as data and never followed.

## Reproduce

```powershell
git clone --depth 1 https://github.com/Flagsmith/flagsmith.git flagsmith
git -C flagsmith fetch --depth 1 origin 899aa383636d28d426e8e0645d0f06c9e63f60f0
git -C flagsmith checkout 899aa383636d28d426e8e0645d0f06c9e63f60f0
```
