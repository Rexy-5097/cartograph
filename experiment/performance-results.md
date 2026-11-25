# Performance results

**Machine:** the development laptop (Windows 11, Intel Core 7 240H), `rustc 1.97.1`, Node 24.13. The server is a release build. The browser is the built-in Chromium pane of the Claude desktop app.

**Repository:** Flagsmith at `899aa38`: 2,755 files, 1,004 nodes, 1,092 edges, 117 clusters.

**Raw logs:** `$R/output/verify-api.txt`, `reanalysis-timing.txt`, `regression.txt`, where `$R` is the external experiment folder.

## Measured

| Measure | Result | How |
|---|---|---|
| First analysis (cold OS file cache, CLI text mode) | 30.10 s | `cartograph .`, first run after the clone |
| Warm analysis (CLI `--json`) | 3.37–3.52 s | `summary.json` `duration_ms`, and a rerun |
| Reanalysis through the server, request → ready | 3,612 / 3,832 / 3,532 ms | 3 consecutive `POST /api/analyze` + `/api/status` polling at 50 ms; the engine's own `elapsedMs` was 3,480 / 3,704 / 3,395 |
| Scene payload `/api/analysis` | 239 KiB, 3–18 ms to fetch | the same script |
| Report `/api/report` | 377 KiB, 16–27 ms | the same script |
| Evidence lookup `/api/evidence` | median 1.0 ms, p95 18.8 ms (n=61) | `verify_api.py`, a new TCP connection per request |
| Node lookup `/api/node` | median 1.2 ms, p95 17.8 ms (n=41) | the same |
| Server memory | 17–19 MiB working set at rest; 79 MiB peak during analysis | `Get-Process` before and after the reanalysis runs |
| Browser JS heap, map open | 14.5 MiB | `performance.memory` |
| Front-end bundle | 398 KiB JS (113 KiB gzip), 18 KiB CSS | `npm run build:browser` |

Lookups never wait for a reanalysis. The server publishes a finished analysis behind an `Arc`, and lookups clone it and drop the lock.

Measured:
- While analysis #9 ran, 138 evidence lookups were made for analysis #8.
- 137 returned 200, with a median of 15.3 ms and a maximum of 16.5 ms.
- The last one was made after #9 was published, and was correctly refused as stale (409).

## Not measured, and why

- **Frame rate on the 1,004-node map in the browser.** A `requestAnimationFrame` sample during zooming returned 4 frames in about 3.6 s. The browser pane was **hidden**, and Chromium throttles animation frames in a hidden view, so that number describes the throttling, not the renderer. **No frame-rate claim is made for the browser.**

  The existing renderer benchmark (`docs/benchmarks/m11-renderer.md`) measured the *same* Sigma configuration in the Tauri window on 10,000 nodes and 11,444 edges: median frame 6.7–10.0 ms, p95 11.1–14.3 ms. Flagsmith's graph is a tenth of that size. During the interactive session (pane visible), panning and zooming on the full map were visibly smooth, but that is an observation, not a measurement.
- **A browser-tab run of the 10k fixture.** Not run. The browser build loads only server payloads, not `scene-10k.json`.
- **Search latency.** Search is the desktop's `findNodes` over the in-memory scene. It was not timed separately and is visually instantaneous on 1,004 labels.

## Design choices for large graphs

- **Source previews are fetched only on request**: one file window at a time, at most 61 lines.
- **The scene and report are fetched once per analysis.** Node detail, evidence, trace, blast and source are fetched on demand.
- **Filters use Sigma reducers** (hidden and colour attributes), so toggling a layer does not rebuild the Graphology graph.
- **"Hide test code" and "Focus neighbourhood"** are progressive-exploration tools. On Flagsmith, hiding tests removes 747 of the 1,004 nodes.
- **Analysis runs on a worker thread.** The UI polls status (600 ms while running, 5 s when idle) and stays interactive on the previous analysis until the new one is ready.
