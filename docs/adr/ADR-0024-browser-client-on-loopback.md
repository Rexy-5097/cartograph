# ADR-0024 — A browser client served on loopback (experimental)

**Status:** Proposed · 2026-11-25 · Not accepted · Evidence: the M18 browser
experiment on branch `experiment/m18-browser-explorer` (`experiment/`).
Would amend [ADR-0005](ADR-0005-local-first-privacy.md) ("no server-side
features in v1") and the deferral of a web server in `ARCHITECTURE.md` §"Deferred
technology". Would sit beside [ADR-0016](ADR-0016-desktop-boundary.md) (desktop
boundary) and [ADR-0020](ADR-0020-mcp-boundary-and-authorization.md) (MCP
boundary, which rejected loopback HTTP *for MCP*).

> **Nothing in this ADR is accepted architecture.** `ARCHITECTURE.md` is
> unchanged. The code that motivates it lives in an excluded package
> (`experiment/browser-explorer/server`), an additive set of
> `cartograph-desktop` functions, and a separate browser build of the desktop
> front end. If the owner rejects this ADR, the experiment is deleted with the
> branch and no accepted component changes.

## Context

Cartograph has three clients of the Rust core: the CLI, the MCP server and
the Tauri desktop application (ADR-0001). The desktop shows the map, edge
evidence and blast radius. It does not show TRACE, refusals (the match
decisions the resolver declined), per-file diagnostics or source.

The M18 experiment asked whether Cartograph's real output on an unfamiliar
repository can be explored in an ordinary browser tab, reusing the desktop
renderer, without a second analysis engine. It was run on Flagsmith (randomly
drawn; 2,755 files, 1,004 nodes, 1,092 edges) and the browser's counts, graph
and evidence were checked against `cartograph --json` for the same commit:
all 21 counts, every node and edge, and 61 sampled evidence records agree
(`experiment/regression-test-results.md`).

Constraints this has to respect, found in the audit
(`experiment/existing-frontend-audit.md`):

1. ADR-0005 / `ARCHITECTURE.md` defer any server; Axum is deferred until a
   cloud component is paid for.
2. ADR-0020 §C weighed and rejected loopback TCP/HTTP for MCP ("no listener
   code, no port convention").
3. ADR-0021 C3: the CLI and the MCP server must stay incapable of network
   egress by their dependency graph. `cartograph-desktop` depends on
   `cartograph-ask` (which holds `ureq` and the keychain stores).
4. The CLI has a test forbidding a `serve` or `ui` subcommand
   (`crates/cartograph-cli/src/main.rs`, the `forbidden` list).
5. RULE 011/013/015: no source leaves the machine, full usefulness offline,
   no source or secrets in logs.

## Decision (proposed)

**A. A separate binary, not a CLI subcommand.** `cartograph-serve` is its own
package. It depends on `cartograph-desktop`, exactly as the Tauri shell does,
and calls only its functions. The CLI is unchanged and keeps its `serve`
prohibition, so ADR-0021 C3 holds for the CLI and MCP. For the experiment the
package is excluded from the workspace (like `desktop/src-tauri`).

**B. The core stays the only analysis.** New read-only functions live in
`cartograph-desktop` and delegate to the core: `report::compose` (counts,
match decisions, diagnostics and node locations from the pipeline's own
`Analysis`), `trace::walk` (`cartograph_graph::trace::trace`, ADR-0019),
`inspect::node` (a node's in and out edges) and `source::excerpt`. The front
end decides presentation only. Its layer grouping by file extension and its
"hide test code" filter by path are labelled as presentation in the UI.
This adds one dependency edge, `cartograph-desktop → cartograph-resolver`, for
the `MatchResult`/`MatchStatus` types of decisions the pipeline already
returns; only accessors and formatting are used, no matching logic. A pipeline re-export would remove the
edge, and is preferable before acceptance.

**C. Loopback with a per-launch session.**
- Bind `127.0.0.1` only. One repository, fixed in argv. No request names a
  path to analyse.
- A per-launch random token is delivered in the URL fragment and sent back in
  the `X-Cartograph-Session` header on every `/api` request. The fragment
  never reaches a server log or a `Referer`.
- Refuse a foreign `Host` (421, against DNS rebinding) and a foreign `Origin`
  (403). Never answer CORS preflight.
- Strict CSP (`script-src 'self'`, `connect-src 'self'`, `frame-ancestors
  'none'`), `no-store`, `nosniff`, `no-referrer`.

**D. Source only where the analysis cites it.** A source excerpt is served
only for a file that some node or edge of the current analysis cites. The path
is canonicalised and must stay under the canonical repository root, and at
most 61 lines of a file of at most 2 MB are returned.

**E. Ids are scoped to an analysis.** The server allocates the `AnalysisId`
under its lock before an analysis runs. Every lookup carries the id, and a
stale one is refused with 409 `staleSelection`. This removes the id race the
audit found in the Tauri shell, where it is unreachable only because the UI
serialises requests.

**F. ASK is not exposed.** No AI endpoint, no opt-in write, no keychain
access. The binary still *links* `ureq` and the keychain stores through
`cartograph-desktop`. Before acceptance, either `cartograph-desktop` gains a
feature gate that removes `cartograph-ask`, or the server depends on a smaller
crate, so that "no egress" holds by dependency graph and not only by not
calling it.

## Alternatives

- **`cartograph serve` in the CLI.** Rejected: breaks ADR-0021 C3 via
  `cartograph-desktop → cartograph-ask → ureq` and the CLI's own `serve`
  prohibition.
- **Static export (`--html`).** No server, no listener threat. But no
  re-analysis, and every source excerpt would have to be embedded in advance,
  which copies source into a file that can be shared by accident (RULE 011).
  Worth keeping as a later, narrower option.
- **An HTTP framework (Axum, tiny_http).** Rejected for the experiment: the
  dependency policy asks what concrete problem it solves now. The
  standard-library server is about 700 lines. If accepted, a small audited
  server crate should be reconsidered.
- **Only the Tauri app, extended.** Equally possible. The browser build reuses
  the same `GraphView`, `EvidencePanel`, scene builder, search and blast code,
  so the new views could move into Tauri too. This ADR does not decide
  between them.

## Consequences

- A local listener is a new attack surface that Tauri IPC does not have. The
  session token, Host/Origin checks and fixed repository are the mitigations.
  `experiment/security-validation.md` lists what was tested and what was not.
- The token comes from the standard library's `RandomState`: 64 hex digits
  carrying about 128 bits from the operating system. Acceptance should replace
  it with a direct OS random source. With `--open` the token also appears on
  the browser's command line, which other local users can read on some
  systems.
- A thread per connection with a 10-second read timeout and a 16 KiB head
  limit. There is no connection cap: a local process can exhaust threads.
  Acceptable for a loopback developer tool, but recorded.
- The front end gains an optional `view` prop on `GraphView`, ignored by the
  desktop. Desktop tests and build are unchanged (111 tests pass).
- New maintenance: a second front-end entry (`browser.html`), a second Vite
  config and the server package.

## Status and what acceptance needs

Proposed only. An independent review of the experiment found no critical
issue; its 18 findings and their disposition are in
`experiment/security-validation.md`. Acceptance needs the owner's approval,
a security review of whatever is finally proposed, the dependency-graph fix in **F**, a decision between
**A** and the Tauri alternative, and an update to `ARCHITECTURE.md` by the
documented process. Until then nothing from the experiment merges to `main`.
