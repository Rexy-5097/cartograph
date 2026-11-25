# Security and privacy validation

Each requirement from the brief is listed below with how it is met, how it was tested, and the result. "Tested" means a request was actually made against a running `cartograph-serve`, by `verify_api.py`, `probe_fixture.py`, curl or the browser.

| Requirement | Implementation | Test | Result |
|---|---|---|---|
| Bind `127.0.0.1` only | `TcpListener::bind(("127.0.0.1", port))` | `Get-NetTCPConnection -State Listen` shows only `127.0.0.1:47820` | PASS |
| Never upload source; nothing to third-party APIs | no outbound code path; ASK not exposed | code reading. No network call in the server or the new functions. The binary *links* `ureq` (limitation 18). | PASS by code; not by dependency graph |
| No arbitrary filesystem access | the only file reads are (a) static assets under the canonical `dist-browser` root and (b) `source::excerpt` | `/../Cargo.toml` → 404; `/api/source` with `../../secret.txt` → 404; Rust tests refuse `secret.txt`, `../`, `web/../`, `/etc/passwd` | PASS |
| Validate every requested path | static: reject `..`, empty segments and `\`, then canonicalise and require the root prefix. Source: the file must be cited by the analysis *before* any filesystem call, then canonicalised under the canonical repository root (symlinks resolved), capped at 2 MB and 61 lines. | the above, plus uncited `README.md` → 404 | PASS |
| Restrict operations to the repository and session | repository fixed in argv; no endpoint takes a path to analyse; every lookup carries an `AnalysisId`; stale → 409 | earlier id → 409 `staleSelection` | PASS |
| Protect state-changing operations from cross-origin use | `POST /api/analyze` (the only state change) needs the token in a custom header, which a cross-origin page cannot send without a preflight the server never answers. A foreign `Origin` → 403. | foreign Origin → 403; no token → 401 | PASS |
| DNS rebinding | `Host` must be `127.0.0.1:PORT` or `localhost:PORT` | `Host: evil.example:PORT` → 421 | PASS |
| Local session protection | a 64-hex per-launch token in the URL fragment, which never reaches a server log or a `Referer`; moved to `sessionStorage` and removed from the address bar on load | the address bar shows `http://127.0.0.1:PORT/` after load (checked in the pane); no token → 401 | PASS, with limitation 17 on entropy |
| Prevent unauthorised access to source and evidence | every `/api/*` needs the token, the right Host, and an allowed or no Origin | 401 / 421 / 403 as above | PASS |
| No absolute paths or environment variables leak | payloads carry the display name and repository-relative paths; errors from `validate` use `<absolute>` | empty-repository error: `no … files were found under \`<absolute>\``; display name `flagsmith`, not `.` or the full path; no endpoint reads the environment | PASS |
| Clean shutdown | the process holds no persistent state; Ctrl+C or termination ends it | after stop: connection refused, port released; the browser shows "Server unreachable" | PASS, abrupt (limitation 20) |
| Response hardening | CSP `default-src 'self'; script-src 'self'; connect-src 'self'; frame-ancestors 'none'; base-uri 'none'; form-action 'none'`, `X-Frame-Options: DENY`, `nosniff`, `no-referrer`, `no-store`, CORP same-origin | `curl -D -` on an API response and on `/`: all headers present | PASS |
| Request limits | 16 KiB head, 1 KiB body (read and discarded), 10 s read timeout; invalid or duplicate length, duplicate `Host` and `Transfer-Encoding` refused | the independent reviewer's raw-socket probes, then re-probes after the fixes (`$R/output/review-fix-probes.txt`) | PASS |
| No XSS from source or evidence | React renders text nodes only; no `dangerouslySetInnerHTML` in `src/browser/` | grep | PASS |

## Privacy (RULE 011–015, ADR-0005)

- **No source in logs.** The server prints three startup lines, including the URL with the token, and nothing per request.
- **Source text leaves the server only** as a bounded excerpt, to the same machine's browser, for a file the analysis cites.
- **No telemetry, no external fonts or CDNs.** The CSP `default-src 'self'` would block them.
- **No AI and no keychain access.**

## Independent review

An independent reviewer, a separate agent that did not write the code, reviewed the whole diff on 2026-11-25. It probed two throwaway servers with raw sockets.

**What it confirmed sound:**
- Host checks: foreign, missing, `LOCALHOST` or `[::1]` → 421.
- Origin checks: foreign or `null` → 403; a cross-site form POST is refused.
- Token checks: none, or a same-length wrong token → 401; the comparison is constant-time.
- Static traversal: about 20 variants (`%2e%2e`, `%5c`, drive letters, UNC paths, the `\\?\` prefix, `%00`, `NUL`/`CON`, trailing dots) → 404.
- `source::excerpt`: uncited, `..`, a different case, a backslash, `%00`, an absolute path → refused; a stale id → 409; extreme line numbers are clamped.
- Malformed heads → 400. With 1,500 idle connections open, status still answered.
- Absolute paths are redacted in failure messages.
- No `dangerouslySetInnerHTML`.

**Its findings, and what was done:**

| # | Finding | Severity | Disposition |
|---|---|---|---|
| 1 | The blast view's affected-node colours were replaced by layer colours | important | **Fixed.** No layer colour in the blast view, and filters never hide a node of the active trace or blast. Seen in the browser (screenshot 12). |
| 2 | A late answer within one analysis could overwrite a newer selection | minor | **Fixed.** A per-kind request counter, and the on-screen analysis id held in a ref. |
| 3 | Nav badges disagreed with what their views highlight | minor | **Fixed.** Badges count the nodes each view highlights. "Class / model" is renamed "Python class"; the Database view highlights tables. The note says the grouping is presentation. |
| 4 | Trace path count silently capped at 200 | minor | **Fixed.** It says "200 or more … this list stops collecting at 200". |
| 5 | Status wording differed from the resolver's definitions | minor | **Fixed.** Reworded from `matching.rs`, including absolute hosts and method mismatch. |
| 6 | Request body not drained; `Content-Length: -5` and chunked bodies accepted | minor, confirmed | **Fixed.** The body is read and discarded; an invalid length, `Transfer-Encoding` or a duplicate length → 400. Re-probed: a 1,000-byte body gives a clean 202; −5, chunked → 400. |
| 7 | A panic in the analysis thread left the status "running" for ever | minor | **Fixed.** `catch_unwind` turns it into a failed analysis; a poisoned lock is recovered. |
| 8 | A failed first analysis showed both the spinner and the failure banner, with fast polling | minor | **Fixed.** No spinner after a failure; slow polling. |
| 9 | A source preview reads the file as it is now, not as analysed | minor | **Recorded** (known limitation 26) |
| 10 | With `--open`, the token is on the browser's command line, which other local users can read on Linux | minor | **Recorded** (known limitation 27) |
| 11 | Token doc said "256-bit" | nit | **Fixed.** Doc and test name now say 64 hex digits and about 128 bits of entropy. |
| 12 | `report.rs` claimed to follow `match --json` names | nit | **Fixed.** The doc states the real differences. |
| 13 | Duplicate `Host`: the last one won | nit, confirmed | **Fixed.** A duplicate `Host` → 400 (re-probed). |
| 14 | The callout keyed on cross-language edges | nit | **Fixed.** It keys on frontend-file decisions from the report (new `frontendRequests` with a test). |
| 15 | The class-based-view sentence is the front end's inference | nit | **Fixed.** It is labelled "Interface note" and phrased conditionally. |
| 16 | The trace test compared top-level hops only | nit | **Fixed.** A recursive comparison including `stop`, and an assertion that nesting is exercised. |
| 17 | The new `cartograph-desktop → cartograph-resolver` dependency was undocumented | nit | **Fixed.** Documented in `lib.rs` and ADR-0024. |
| 18 | `analyze_as` built a report and dropped it | nit | **Fixed.** A shared private `run(…, with_report)`; the desktop path composes no report. |

A second independent check of these fixes is recorded in `final-experiment-assessment.md`.

### Second independent check of the fixes

A different agent, which wrote neither the code nor the fixes, verified the fixes. It ran the tests and probed a throwaway server with raw sockets.

- **Confirmed fixed:** 5, 6 (including the drain arithmetic, bodies split across writes, and a truncated body), 7, 8, 11, 12, 13, 15, 16 and 18.
- **Found incomplete, and finished in a third round:**
  - **1:** edge-kind filters could still cut a trace or blast. They no longer apply in those views.
  - **2:** four remaining late-answer paths:
    - repeated "show source" clicks; source requests now have their own counter;
    - clearing the selection did not invalidate in-flight answers;
    - navigating away while a trace or blast was pending;
    - the stale catch-up compared with a captured id, not the on-screen one, and announced a reload even when none happened.
  - **3, 14, 17:** wording and condition residuals.
  - The doc comment on `tracePaths`, orphaned by the fix for 4.
  - `+3` was accepted as a length; the transfer-encoding message was misleading.
  - A recovered poisoned lock is now cleared.

  After the third round: `tsc` clean, vitest 112, both builds, server tests and pedantic clippy clean, desktop browser tests 5/5; re-probed `+3` and `identity` → 400 and a 1,000-byte body → 202; `verify_api.py` 42 PASS, 0 failures.
- **Accepted residuals** (minor, recorded):
  - error responses (400) are not preceded by draining an oversized body, so the client may see a reset after the 400 on a non-loopback-like stack;
  - `Host : x` (a space before the colon) is tolerated;
  - the layer label "Frontend (TS/JS)" also covers `.vue`.

The fixes from this third round were not put through a further independent check. They are small, and each was re-tested as listed above.
