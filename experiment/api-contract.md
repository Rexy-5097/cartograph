# Local API contract — `cartograph-serve` (experimental)

- **Base:** `http://127.0.0.1:<port>`. By default the server takes a free port and prints it at start; `--port` sets it.
- **Format:** every response is JSON (`application/json; charset=utf-8`) except static files.
- **Field names:**
  - Totals, languages and diagnostics are the pipeline's own types, so they keep its snake_case names (`files_failed`, `http_call_edges`).
  - Every type added for the browser is camelCase.

## Session and headers

| Rule | Detail |
|---|---|
| Token | Printed once at start, as `http://127.0.0.1:PORT/#token=<64 hex>`. The page moves it from the fragment into `sessionStorage` and removes it from the address bar. |
| `X-Cartograph-Session: <token>` | Required on every `/api/*` request, or 401 `no-session`. |
| `Host` | Must be `127.0.0.1:PORT` or `localhost:PORT`, on every request (static files included), or 421 `wrong-host`. |
| `Origin` | If present on `/api/*`, it must be `http://127.0.0.1:PORT` or `http://localhost:PORT`, or 403 `cross-origin`. CORS preflight is never answered. |
| Response headers | `Cache-Control: no-store`, `X-Content-Type-Options: nosniff`, `Referrer-Policy: no-referrer`, `Cross-Origin-Resource-Policy: same-origin`, `X-Frame-Options: DENY`, and `Content-Security-Policy: default-src 'self'; script-src 'self'; style-src 'self' 'unsafe-inline'; img-src 'self' data:; connect-src 'self'; frame-ancestors 'none'; base-uri 'none'; form-action 'none'` |

## Errors

```json
{ "kind": "staleSelection", "message": "That selection belongs to an earlier analysis.", "hint": "Select the relationship again on the current map." }
```

`kind` is either a `DesktopErrorKind` (the same names the Tauri shell uses) or a server kind.

| Status | Kinds |
|---|---|
| 400 | `bad-request` (a missing or non-numeric parameter) |
| 401 | `no-session` |
| 403 | `cross-origin` |
| 404 | `not-found` (endpoint, static file); `unknownEdge`, `unknownNode`; `notFound` (source not cited, outside the root, or missing) |
| 405 | `method` |
| 409 | `staleSelection`, `noAnalysis` |
| 421 | `wrong-host` |
| 500 | `internal`, `no-assets` (`dist-browser` not built) |

## Endpoints

All ids are graph-local numbers, valid only with the `analysis` they came from.

### `GET /api/status`

```json
{ "repository": "flagsmith", "phase": "ready", "analysis": 2, "running": null, "error": null, "experimental": true }
```

- `phase` is one of `idle`, `running`, `ready` or `failed`.
- `analysis` is the newest *completed* analysis.
- `error` is a `DesktopError` for the latest failed run, e.g. `noSupportedSources` with `<absolute>` in place of the path.

### `POST /api/analyze`

Starts a reanalysis of the same repository. There is no body, and no path can be given. The response is `202 {"started": <id>}`; if a run is already in progress, it returns that run's id. Poll `/api/status` until `analysis == started`.

### `GET /api/analysis`

The existing desktop `AnalysisPayload`, unchanged:

- `analysis`, `repository`;
- `summary{files, failed, nodes, edges, clusters}`;
- `scene{nodes[{id,x,y,cluster,label,kind}], edges[{id,source,target,kind}], clusters[{id,label}]}`.

### `GET /api/report`

`AnalysisReport`, from `report::compose`:

```
{ analysis, elapsedMs,
  totals: { files, files_clean, files_with_diagnostics, files_failed, symbols, routes, routes_composed,
            http_observations, exact, strong, ambiguous, no_match, unsupported, dynamic_resolved,
            dynamic_partial, orm_models, orm_tables, orm_ambiguous, nodes, edges, http_call_edges,
            client_call_edges, orm_edges, cross_language_edges, diagnostics },
  languages: { typescript, tsx, python },
  decisions: [ { clientFile, clientLine, clientRoute, status: "exact"|"strong"|"ambiguous"|"no-match"|"unsupported"|"other",
                 unsupportedReason?, candidates: [ { route, file, line, handler?, confidence,
                 methodCompatibility, pathCompatibility, reasons[] } ] } ],
  ormAmbiguous: [ name ],
  diagnostics: [ { file, severity, kind, message, line? } ],
  nodes: [ { id, file?, line? } ] }
```

`status: "other"` covers a future `MatchStatus` variant, since the enum is non-exhaustive.

### `GET /api/evidence?analysis=A&edge=E`

The existing `EvidenceRecord`:

```
{ analysis, edge, kind,
  source{id,label,kind}, target{id,label,kind},
  confidence, calibrated: false, provenance, deterministic,
  evidence, location{file, line, column?} }
```

### `GET /api/node?analysis=A&node=N`

`NodeDetail`:

```
{ analysis, node, label, kind, file?, line?,
  outgoing: [ {edge, kind, node, label, nodeKind} ],
  incoming: [ … ] }
```

### `GET /api/trace?analysis=A&node=N`

`TracePayload`:

```
{ analysis, from, maxDepth, stop?,
  steps: [ { edge, to, stop?, next: [ … ] } ] }
```

`stop` is `chain-end`, `max-depth` or `cycle`. This is the core's `trace` unchanged, with the default depth of 10.

### `GET /api/blast?analysis=A&node=N`

The existing `BlastPayload`:

```
{ analysis, target,
  reached: [ {node, depth, confidence, via} ],
  calibrated: false, routes: "representative" }
```

### `GET /api/source?analysis=A&file=F[&line=L]`

`SourceExcerpt`:

```
{ analysis, file, firstLine, highlight?, lines[], totalLines }
```

- `F` is repository-relative and percent-encoded.
- It is served only if some node or edge of analysis `A` cites `F`.
- At most 61 lines, centred on `L`, from a file of at most 2 MB.

### Static

`GET /` serves `browser.html` (or `index.html`); `GET /assets/*` serves the built JS and CSS. These need the right `Host` but no token, because they contain no repository data.

## Not in the API

These are deliberate omissions:
- choosing a repository or path;
- uploading anything;
- ASK and AI;
- writing settings;
- directory listing;
- reading an arbitrary file;
- diff.
