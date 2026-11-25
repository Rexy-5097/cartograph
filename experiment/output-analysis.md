# Output analysis — Cartograph on Flagsmith

**Repository:** Flagsmith/flagsmith, pinned at `899aa383636d28d426e8e0645d0f06c9e63f60f0`.

**Engine:** `cartograph.exe` built from `upstream/main` `9ee0dea`, the latest verified build (SHA-256 `496abda6ad80104a40647dfe4907d8adc465dd51c62b4ff817c49344a56327a7`).

**Raw output:** `D:/WorkSpace/Projects/cartograph-reviews/2026-11-25-m18-browser-explorer/output/`. Nothing in it was edited.

| File | Command |
|---|---|
| `summary.txt` | `cartograph .` |
| `summary.json` | `cartograph . --json` |
| `match.json`, `match-breakdown.txt` | `cartograph match . --json`, then a read-only breakdown |
| `trace-text.txt`, `trace.json` | `cartograph trace <symbol> --path .` |
| `blast-text.txt`, `blast.json` | `cartograph blast FFAdminUser --path .` |

## Recorded metrics

| Metric | Value | Source |
|---|---|---|
| Analysis duration | 30.10 s cold (first run, OS file cache empty); 3.37–3.52 s warm `--json`; 3.4–3.7 s warm through the browser server | `summary.txt`, `summary.json`, `performance-results.md` |
| Files discovered / analysed | 2,755 (TS 546, TSX 784, Python 1,425) | summary |
| Files that failed to parse | 0 | `files_failed` |
| Files with diagnostics | 2, with 3 diagnostics, all `unparseable region` in TSX | summary |
| Symbols | 19,529 | summary |
| Routes observed | 108 | `routes_observed` |
| HTTP call sites observed | 908 | `http_observed` |
| ORM models | 57 | `orm_models` |
| Tables resolved | 44 | `tables_resolved` |
| Nodes | 1,004: function 895, class 62, table 44, route 3 | `--json` nodes |
| Edges | 1,092: `orm-access` 826, `http-call` 213, `queries` 44, `call` 9 | `--json` edges |
| Cross-language edges | **0** | `cross_language_edges` |
| Clusters (directory groups in the layout) | 117: one per parent directory of a located node, plus `<Table>` for the unlocated tables | scene payload `summary.clusters` |
| Refused | 695: ambiguous 4, no match 384, unsupported 307 | summary, `match.json` |
| Unsupported, by reason | `ClientPrefixUnresolved` 233, `ClientTargetsAbsoluteHost` 74 | `match.json` |
| ORM ambiguous (model names claimed by more than one class) | 0 | `match.json` `orm_ambiguous` |

## 1. What Cartograph actually detected

- **The Django backend:**
  - 108 route declarations;
  - 57 ORM models, 44 of which resolved to a table name. Each table name has an evidence sentence such as *"derived `code_references_scannedcodereferences` by Django default `<app_label>_<modelname>`; app label from the package's apps.py"*.
- **908 HTTP call sites.** **858 of them (94%) are in Python test files**: DRF's `APIClient` in `api/tests/…`.
- **The React/TypeScript frontend contributed only 9 call sites:**
  - 2 no-match, e.g. `GET /version`, `POST /api/event`;
  - 7 unsupported: four `ClientPrefixUnresolved` (Playwright helpers that build URLs at run time) and three `ClientTargetsAbsoluteHost` (`api.github.com`, `slack.com`, `app.launchdarkly.com`).

  The real frontend data layer is **RTK Query**: `createApi` / `builder.query({ query: … url … })` in 87 files, with a base URL configured at run time. Cartograph does not recognise it as an HTTP call site, so the frontend's actual requests are invisible to the analysis.

## 2. What the graph contains

1,004 nodes and 1,092 edges. Only artefacts that take part in a resolved relationship become nodes.

- **747 of the 1,004 nodes (74%) live in test-path files.** Most are test functions that call the API or touch a model.
- **There are three route nodes.** Each is an accepted match whose handler the handler index could not resolve: `/o/token/`, `create-change-request`, `swagger.json`. Every other accepted request ends at a handler symbol: 202 at a class (DRF views and viewsets) and 7 at a function.
- **The 9 `call` edges** are the client-wrapper relationships of ADR-0012.

## 3. Relationship types present

| Kind | Count | Provenance | Confidence (uncalibrated prior) |
|---|---|---|---|
| `orm-access` | 826 | orm-resolution (estimated) | 0.80 |
| `queries` (model → table) | 44 | orm-resolution | 0.80 |
| `http-call` | 213 | route-matcher (computed) | 0.78 / 0.72 / 0.64 |
| `call` | 9 | static-import (computed) | 0.90 |

There is no `import`, `inherits` or `references` edge. The resolver does not construct those kinds today.

## 4. Which relationships are demonstrably correct

Each claim below was checked against the cited line in the pinned clone, through the browser's source preview and with `sed -n` on the clone:

- **TRACE from `test_get_feature_code_references__multiple_scans_exist__returns_latest_per_repository`** (`api/tests/unit/projects/code_references/test_unit_projects_code_references_views.py:344`):
  - the HTTP call at line 344 is `admin_client_new.get(f"/api/v1/projects/{project.pk}/features/{feature.pk}/code-references/")`. It matches the `path("projects/<int:project_pk>/features/<int:feature_pk>/code-references/", views.FeatureFlagCodeReferenceDetailAPIView.as_view())` that starts at `api/projects/code_references/urls.py:13`, whose view is declared at `views.py:74`;
  - the ORM accesses `ScannedCodeReferences.objects.create` (line 314) and `VCSRepository.objects.create` (line 308) are at the cited lines;
  - the table names follow Django's default naming.

  All three branches are correct at the cited lines.
- **BLAST of `FFAdminUser`** (`api/users/models.py:123`): 124 dependants at depth 1. A sample of rows (`create_superuser` in `api/users/services.py:28`, `_get_user` in `api/users/serializers.py:47`, `authenticate` in `api/integrations/slack/authentication.py:25`) each contains an `FFAdminUser.objects…` access at the cited line.
- **The evidence equals the engine's.** On 61 evidence records sampled across the id range, the browser API's evidence, provenance, confidence, file and line are byte-equal to `--json` (`regression-test-results.md`).

**What was not checked:**
- Each of the 213 HTTP edges was not verified by hand. The 0.64-confidence matches (179 of them) carry *"client value(s) at position(s) … not determined"*: the test URL has run-time segments, so the match is by static segments only and could point at the wrong one of two similar routes.
- The engine's correctness was not re-validated here. That is M17's job.

## 5. What remains unresolved

- **Frontend to backend: nothing.** Zero cross-language edges. The map shows the frontend and the backend as disconnected regions, and the UI says so explicitly instead of implying a connection.
- **384 no-match:** mostly test requests to DRF **router**-generated URLs (`/api/v1/projects/…` 131, `/api/v1/environments/…` 106). DRF routers (`router.register`, in 15 files) and djoser auth routes are not modelled as route declarations.
- **307 unsupported:** 233 with an unresolved prefix (f-strings and `reverse()` results), 74 absolute hosts (third-party APIs such as Google Analytics, Slack, GitHub).
- **4 ambiguous:** several routes fit equally, so no edge is produced.
- **3 diagnostics** in two TSX files (`unparseable region`). Neither file is otherwise lost; 0 files failed.

## 6. Which results are suitable for visualisation

- **Good:**
  - the ORM layer (models, tables and their accessors), which is dense, correct and evidence-backed;
  - TRACE from a test or view down to tables;
  - BLAST of a model, e.g. 124 dependants of `FFAdminUser`;
  - backend request handling *from tests*;
  - the unresolved list, which is the most informative view for this repository.
- **Poor:**
  - the full map, which is a hairball until test code is hidden: 1,004 nodes, mostly tests clustered by directory;
  - any "frontend → backend" story, because the data has none.

  The browser addresses this with a clearly labelled *path heuristic* to hide test code, layer and kind filters, and a neighbourhood focus. None of these change the data.

## 7. Output fields that are insufficient for a good user experience

1. **The scene carries no file or line** (ADR-0017, deliberately). Layer grouping, test filtering and "where is this" needed node locations. They were added to a separate *report* payload, not to the scene.
2. **No test/production distinction.** The engine does not mark test code, and 74% of nodes are tests. The browser's filter is a path pattern and is labelled as such.
3. **No architectural layer.** "Frontend / API / backend / model / database" is a presentation grouping by file extension and node kind, not an analysis result.
4. **Refusals are not linked to graph nodes.** A no-match decision names a client file and line, but no node. The unresolved view can show *where* the refusals are, but not attach them to the map.
5. **Route nodes vs handlers.** 105 of the 108 observed routes have no node of their own: an accepted match ends at the handler class or function. A "routes" view therefore lists handlers, not URLs, unless the evidence text is read.
6. **No clusters API beyond the directory label.** Clusters are directory names, which is useful but carries no meaning about modules or applications.
7. **Confidence values are uncalibrated priors.** They are shown with that caveat and never as probabilities.

## 8. Backend changes made, and changes worth considering

Made in this experiment: **additive and read-only**, in `cartograph-desktop`. No change to parser, resolver, graph or pipeline semantics.

- `report::compose`: totals, match decisions, ORM-ambiguous names, diagnostics and node locations, from the pipeline's own `Analysis`.
- `trace::walk`: wraps `cartograph_graph::trace::trace`.
- `inspect::node`: a node and its in and out neighbours.
- `source::excerpt`: a cited-file-only, bounded source window.
- `session::analyze_report_as`: returns the report alongside the existing payload; `analyze_as` is unchanged.

Not made, but these would most improve this repository's picture. They are engine work for a milestone, not for the browser:

1. **RTK Query `createApi` / `builder.query` recognition** in the TS parser. This is the single biggest gap: without it the frontend half of the system is invisible.
2. **DRF `DefaultRouter.register` route synthesis.** It would turn most of the 384 no-match decisions into edges or honest ambiguities.
3. **A test-code flag** on nodes, from discovery, instead of a client-side path pattern.
4. Linking a refusal to the node that made the call, when one exists.
