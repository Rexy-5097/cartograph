# Visual and usability review

All screenshots show the real Flagsmith analysis served by `cartograph-serve`. There are no mock figures and no synthetic edges. They are in `D:/WorkSpace/Projects/cartograph-reviews/2026-11-25-m18-browser-explorer/screenshots/`. Screenshots 01–07 were taken at 1440×900 in the built-in Chromium browser pane.

| # | File | Shows |
|---|---|---|
| 1 | `01-overview.jpg` | Repository overview: real totals, languages, relationships by kind, and the "no relationship crosses between languages" callout |
| 2 | `02-full-map-1004-nodes.jpg` | Full map: 1,004 nodes and 1,092 edges, layer colours, legend with counts |
| 3 | `03-edge-evidence.jpg` | A selected ORM edge in `EvidencePanel`: provenance, uncalibrated confidence, location, the evidence sentence, and an assumption ("LifecycleModel, outside the project") |
| 4 | `04-trace.jpg` | TRACE from a test: 3 paths, each stop explained, map highlight |
| 5 | `05-blast.jpg` | BLAST of `FFAdminUser`: 124 dependants at depth 1 |
| 6 | `06-unresolved.jpg` | Unresolved requests grouped by status and reason, with candidates |
| 7 | `07-large-graph-tests-hidden.jpg` | The same map with test code hidden by the path filter |
| 8 | `08-partial-diagnostics.jpg` | A partial analysis (fixture): 2 diagnostics, 0 failed files |
| 9 | `09-server-unreachable.jpg` | The error banner after the server stopped (before the header fix below) |
| 10 | `10-phone-width.jpg` | 375 px wide: no horizontal page scroll |
| 11 | `11-inspector-after-wrap-fix.jpg` | Inspector after the overflow fix |
| 12 | `12-blast-colours-after-review-fix.jpg` | BLAST after the review fix: affected nodes orange, others dimmed (overlay hidden for the shot) |
| 13 | `13-blast-panel-and-badges-after-review.jpg` | BLAST panel; nav badges counting what each view shows; "Python class" |

**Screenshot 05 predates a fix.** In it, the affected nodes still carry layer colours. The independent review found that the layer colour replaced the blast colours; screenshot 12 is after the fix.

## Can a new user answer the five questions?

1. **Which parts are connected?** Partly.
   - The overview and legend say exactly what kinds of relationships exist and how many.
   - The map shows the backend and the ORM densely connected.
   - The frontend appears in its own colour and visibly unconnected. The callout says this is what Cartograph could *establish*, not how the app works.
2. **How frontend requests reach backend handlers:** not for this repository, because the data has no such edge. The UI says so rather than hiding it. Unresolved explains every request it declined, and for Flagsmith those are almost all test requests. The real frontend uses RTK Query, which the engine does not recognise (`output-analysis.md` §1).
3. **Which models and tables are involved:** yes. The Database view, BLAST of a model, and TRACE down to tables answer this directly, with evidence on every hop.
4. **Why a connection exists:** yes. Each edge opens `EvidencePanel` with the evidence sentence, provenance, the confidence caveat and the location, and "Show source" opens the cited lines.
5. **Where the analysis is uncertain:** yes.
   - Confidence is always shown as an uncalibrated prior.
   - The Unresolved view groups 695 refusals by reason, in plain language.
   - Trace stops say why they stopped, including the documented class-based-view limit.
   - Diagnostics list unparseable regions.

## Checklist

| Aspect | Finding |
|---|---|
| Visual hierarchy | A clear three-column layout: views, canvas, inspector. The EXPERIMENTAL badge is always visible. Overview numbers are large; explanations sit beneath them. |
| Readability, typography | The desktop's type scale and dark palette. Monospace for code, paths and routes. Long test names wrap: fixed in this review. |
| Spacing | Consistent 6/12 px rhythm. The trace overlay is dense with 3+ paths. |
| Colour consistency | **Fixed during the run.** The legend swatches did not match the renderer's edge colours; swatches now read `edgeColor()` from the renderer. Layer colours were chosen apart from edge colours (ORM green, HTTP orange, queries teal). |
| Node differentiation | Colour by layer (default) or cluster, except in the blast view, where the renderer's target, affected and dimmed colours carry the result (**fixed after review**). Size by kind (renderer). Kind is shown in tooltip, inspector and search. The layer once called "Class / model" is now "Python class", since it includes views and serializers, not only ORM models. |
| Edge differentiation | Colour and width by kind, from the renderer. Derived vs inferred is **not** shown on the map, because the scene carries no provenance (ADR-0017); it is shown in the inspector. |
| Graph density | **The main problem.** The full map is a hairball (screenshot 2), because 74% of nodes are tests that fan into a few models. "Hide test code", layer and kind toggles and neighbourhood focus make it workable (screenshot 7). Labels overlap when zoomed into a dense cluster; the renderer's zoom-gated labels limit, but do not prevent, this. |
| Empty states | A count of "none" means *detected nothing*, and the left panel says so. The empty repository shows the engine's `noSupportedSources` message and hint. Empty trace and blast results say "none established". |
| Loading states | "Analysing…" in the header (`aria-live`), a spinner before the first analysis, and Refresh disabled while running. |
| Error states | A banner for server or load errors; inspector errors for lookups. **Fixed:** the header kept saying "Analysis #N ready" after the server stopped (screenshot 9); it now says "Server unreachable". **Fixed:** after a refresh from another tab or client, every selection was refused as stale with no way forward; the page now loads the newer analysis and says so. |
| Responsive | Breakpoints at 1100 and 820 px. At 375 px there is no horizontal page scroll; the nav scrolls sideways and the inspector stacks below (screenshot 10). The map is cramped on a phone. |
| Keyboard | Every control is a native button or input with a visible `:focus-visible` ring. Search → Tab → Enter selects a symbol. **Fixed:** focus fell to `<body>` after choosing a result; it now moves to the inspector. **The canvas itself is not keyboard-navigable** (Sigma); search, the lists in Trace, Blast, Unresolved and the inspector are the keyboard route to every node and edge. |
| Accessibility, screen readers | The map has `role="img"` with node and relationship counts. Views are a labelled `nav` with `aria-current`. Status is announced politely and errors with `role="alert"`. Lists are real lists. Swatches are `aria-hidden`. Reduced motion is respected. Not tested with an actual screen reader. |
| Performance | See `performance-results.md`. Lookups take milliseconds; reanalysis is about 3.5 s without blocking the UI. |

## Remaining usability issues (not fixed)

- Label overlap in dense, zoomed-in regions.
- The trace and blast overlays cover part of the map. They can be collapsed by switching view, but not moved.
- No legend entry explains node *size* (by kind).
- Routes appear as handler classes, not URLs, except for the 3 route nodes. This is a data limitation, explained in the Unresolved view.
- No deep links: ids are analysis-scoped, so the URL does not record a selection.
