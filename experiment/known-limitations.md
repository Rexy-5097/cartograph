# Known limitations

These are stated plainly. None of them is hidden in the UI.

## Of the data, for this repository (engine, not browser)

1. **No frontend-to-backend edge.** Flagsmith's frontend calls its API through RTK Query (`createApi` / `builder.query`) with a base URL set at run time. Cartograph does not recognise this as an HTTP call site, so there are 0 cross-language edges, and the main story a full-stack explorer should tell is absent.
2. **Most HTTP edges come from tests.** All 213 come from DRF test clients in `api/tests`. They show which handler a test request reaches, not how the product's UI talks to the backend.
3. **DRF router routes are not modelled.** 384 test requests find no candidate route, mostly `/api/v1/projects/…` and `/api/v1/environments/…`, which are registered with `DefaultRouter`.
4. **74% of nodes are test code.** The engine does not mark test code; the browser's "hide test code" is a path pattern, labelled as such.
5. **The 179 HTTP matches at 0.64 confidence** match on static segments only (run-time path values are not determined). Each one was not hand-verified.
6. **Class-based views end a trace.** Cartograph reaches the view class but does not follow into its methods (documented). The trace panel says so.
7. **Confidence values are uncalibrated priors.** They are never presented as probabilities.

## Of the browser experiment

8. **No frame-rate measurement in the browser.** The pane was hidden, so animation frames were throttled. The only renderer numbers are the existing M11 desktop benchmark.
9. **Not tested with a screen reader.** ARIA roles and labels were checked in the DOM only. **The map canvas is not keyboard-navigable**; search and the lists are the keyboard route.
10. **Dense regions overlap labels** when zoomed in. The trace and blast overlays cover part of the map.
11. **"Layer" is a presentation grouping** (file extension and node kind), not an architectural fact. The Frontend view shows the 12 TypeScript nodes Cartograph found, not the frontend's real size.
12. **No deep links or bookmarks.** Ids are analysis-scoped by design (ADR-0011, the desktop's stale guard).
13. **One repository per server**, fixed at start. There is no repository picker: a browser cannot supply a path safely.
14. **ASK (AI explanations) is not exposed**, by design (ADR-0021 C3, RULE 012).
15. **Single user, local.** One analysis runs at a time. A second `POST /api/analyze` while one runs returns the running id.
16. **The Tauri window was not exercised by hand.** Its front end and shell compile, and its tests pass.

## Of the server's security (details in `security-validation.md`)

17. **The token is derived from the standard library's `RandomState`.** It prints 64 hex digits but carries about 128 bits of entropy from the OS. It is adequate for a loopback session, but a direct OS random source is the right choice before acceptance.
18. **The binary links `ureq` and the keychain stores** through `cartograph-desktop → cartograph-ask`, although it never calls them. "No egress" holds by *code*, not by *dependency graph*. ADR-0024 F requires fixing this before acceptance.
19. **One thread per connection, with no connection cap.** A local process could exhaust threads; the 10 s read timeout limits slow clients only.
20. **Ctrl+C ends the process abruptly.** There is no persistent state to corrupt, and the OS releases the socket (verified), but in-flight requests are cut.
21. **Static assets need the right `Host` but no token.** They contain no repository data.
22. **The token lives in `sessionStorage`** for the tab's lifetime. Any script on that origin could read it; the CSP allows only same-origin scripts.

## Of governance

23. **`cartograph serve --open` is not implemented as a CLI subcommand.** The CLI's own tests forbid it, and ADR-0021 C3 would be broken. The experience is `cartograph-serve --open`, a separate binary. Whether it becomes a CLI command, a separate binary or a Tauri feature is an owner decision (ADR-0024).
24. **ADR-0024 is Proposed, not accepted.** `ARCHITECTURE.md` is unchanged. Nothing is merged, pushed or released.
25. **Four pre-existing desktop issues** found in the audit are reported, not fixed: the Tauri `AnalysisId` race, grant-before-analysis ordering, the `openapi-spec` slug drift, and the absolute path kept in front-end state.

## Found by the independent review

26. **A source preview reads the file as it is now**, not as it was when analysed. If a file is edited after the analysis, the marked line can point at different code. There is no warning yet; refreshing the analysis realigns it.
27. **With `--open`, the token is on the command line** of the process that opens the browser, and often on the browser's own command line. On Linux, other local users can read that from `/proc/*/cmdline`. On a shared machine, start without `--open` and paste the URL instead.
28. **Error responses do not drain an oversized body first.** A request with a body over 1 KiB gets its 400, then the connection may reset before a slow client reads it. Normal requests drain correctly.
