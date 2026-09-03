# ADR-0020 — MCP boundary and authorization model

**Status:** **Proposed — a decision framework, not a decision.** · 2026-09-03 ·
Governs M15 · Follows [ADR-0019](ADR-0019-trace-belongs-to-the-graph.md) ·
Constrained by [ADR-0005](ADR-0005-local-first-privacy.md) and
[ADR-0001](ADR-0001-rust-core-is-the-product.md)

> Every other Cartograph ADR is `Accepted`. This one is deliberately not, and
> that is the point of it: three M15 questions are **undetermined by repository
> evidence**, and settling them inside implementation code would make them
> invisible. This document presents the options and the evidence. The owner
> chooses; this ADR is then amended to `Accepted` recording what was chosen and
> why. **No option here is a recommendation.**

## Context

M15's scope is *"rmcp-based MCP server exposing graph queries
(map/trace/blast/diff) to Claude Code and Cursor. Session-scoped repository
authorization only"*, and its acceptance is *"MCP client can query the graph of
an authorized repo and nothing else; privacy gate extended to MCP; gates pass"*
(`agentos/milestones/M15-mcp-server.md`).

M15 Slice 1 ([ADR-0019](ADR-0019-trace-belongs-to-the-graph.md)) moved the trace
traversal into `cartograph-graph` and **explicitly deferred** three decisions,
recording that *"a later slice must not read agreement into its silence"*. This
ADR is where they are put to the owner.

The invariant that governs all of them is not in question and is not reopened
here:

> **MCP exposes only the repository authorised in the current session.**
> — [ADR-0005](ADR-0005-local-first-privacy.md), `ARCHITECTURE.md:176`,
> `SECURITY.md` ("MCP is session-scoped"), `docs/security/README.md`
> ("MCP session scoping | M15")

Four sentences state the invariant. **None states a mechanism**, and no code
implements one.

## Problem

Three questions must be answered before `rmcp` can be added, and each changes
what gets built:

| # | Question | Status |
|---|---|---|
| A | What does **"map"** denote as an MCP query? | undetermined |
| B | How does a repository become **session-authorized**? | undetermined |
| C | What **transport** does the server speak? | strong inference only |

---

## A. Map semantics

### The finding that makes this undecidable by reading

`README.md:200–206` defines the product's surfaces:

| Surface | Question | Milestone |
|---|---|---|
| **MAP** | What is this system? | M11 |
| **BLAST** | What breaks if I change this? | M12 |
| **DIFF** | What did this change do to the architecture? | M13 |

**TRACE is not in that table.** So the spec's list `(map/trace/blast/diff)` is
not a list of one kind of thing — it mixes three *product surfaces* with one
*CLI command*. "map" therefore cannot be resolved by analogy with its
neighbours.

Every occurrence of MAP as a noun in Cartograph-owned files denotes the
**desktop application**: `ROADMAP.md:38,66,104`, `CHECKPOINTS.md:884,937,1048`,
[ADR-0006](ADR-0006-sigma-before-custom-renderer.md)`:12,20`,
[ADR-0015](ADR-0015-layout-contract.md)`:42,122,126,142`,
`desktop/src-tauri/src/main.rs:1`. There is no non-visual "map query" anywhere
in the repository.

### Candidates

| | **A1 — analysis summary** | **A2 — the graph itself** | **A3 — the render scene** | **A4 — a filtered/aggregated view** |
|---|---|---|---|---|
| Lives in | data in `cartograph_pipeline::Totals`; shape in `cartograph-cli/src/json.rs` (`Summary`) | `cartograph_graph::ArchitectureGraph` | `cartograph_desktop::scene::Scene` | **nowhere — does not exist** |
| Question it answers | *"can Cartograph analyse this repository, and what did it find?"* (`summary_cmd.rs:5–6`) | "what artefacts and relationships exist" | *"what is this system?"* — MAP's own question ([ADR-0015](ADR-0015-layout-contract.md):42) | *"what is this system?"*, answered in text: clusters, boundary crossings, per-language counts |
| Reusable by another client | **Yes** — `Summary::from_totals` is a field-for-field copy of `Totals`, a library type | **Yes** | Yes, but see below | n/a — would be built |
| Client-to-client dependency | **No** — read `Totals` directly, never the CLI type | **No** | **Yes** — `cartograph-desktop` is a peer client | **No**, if built in `cartograph-graph` or `-pipeline` |
| Deterministic | Yes except `duration_ms` (wall clock) | Yes | Yes ([ADR-0015](ADR-0015-layout-contract.md) promises determinism, not stability) | must be specified as such; nothing enforces it yet |
| Existing tests | CLI goldens (`summary.json`, `summary.txt`) | extensive, across graph/pipeline | `cartograph-desktop` scene tests, [ADR-0017](ADR-0017-render-scene.md) | **none** |
| Output size (measured this session) | ~20 scalar fields | **Airflow 3,104 nodes / 3,350 edges → 2,054,974 bytes of JSON**; Zulip 1,308 / 1,469 → 747,079 bytes | node count + x/y/cluster/label/kind per node | bounded by design — the only candidate whose size is a choice |
| Privacy | `Repository { name, requested }` already redacts rooted paths to `<absolute>` | node/edge locations are repository-relative by construction (`SourceLocation` refuses absolute paths) | same as A2 plus coordinates | same as A2 |
| MCP compatibility | fits any response budget | **exceeds any sane response budget unfiltered** | x/y coordinates are meaningless to a text agent | the best fit for an agent, and the only one requiring new analysis-shaped code |
| Extraction work required | **None** if built from `Totals`. If the CLI's *rendered* summary is wanted, `summary_cmd` must be extracted — it is in a binary crate | **None** | Would need lifting out of a peer client, or duplicating [ADR-0017](ADR-0017-render-scene.md)'s composition | **new work, not extraction** — and it is *analysis*, so RULE 002 puts it in the core, not in `cartograph-mcp` |

**A4 is not a variant of the others.** Nothing in the repository produces it.
`summary_cmd`'s `count_by_kind` / `count_by_provenance` / `count_by_confidence`
are the nearest thing, and they key on **presentation label strings**, so kinds
sharing the `"edge"` fallback label merge — a rendering decision baked into a
count, not an API. Choosing A4 makes M15 larger than the other three: it adds a
new analysis capability, and RULE 002 puts that in `cartograph-graph` or
`cartograph-pipeline`, never in the MCP client.

### Why the repository does not choose

A1 is the cheapest and answers a *different question* from the one MAP is
defined by. A3 answers MAP's actual question and is architecturally
unavailable. A2 is literally "the graph" and is two megabytes on a real
repository. A4 answers the question well and does not exist. Nothing in the
specification, the ADRs, the roadmap or the code prefers one, and each produces
a materially different tool.

**There is no authoritative mapping in this repository from M15's word "map" to
any of A1–A4.** That is the finding, not a gap in the search: `README.md`'s
surface table, `ROADMAP.md`, `ARCHITECTURE.md`, the M15 specification and every
ADR were read, and none makes the connection.

---

## B. Session authorization

### What exists, precisely

The repository has **no** session identity, repository identity, authorization,
request context or session lifecycle. It has four adjacent things, and the
distinction between them matters:

| Concept | Where | What it actually is |
|---|---|---|
| **Validation** | `cartograph_desktop::repository::ValidatedRepository` | exists · is a directory · is readable. Its own doc: *"Constructing one is the **only** way to reach `session::analyze`, so an unvalidated path cannot reach the analyser by mistake."* **Not authorization**, and desktop-owned. |
| **Generation identity** | `cartograph_desktop::evidence::AnalysisSession { id: AnalysisId, graph }` | a **staleness** token: the payload carries the id, a lookup must present it, a mismatch is refused as `StaleSelection`. Right *technique* — stamp, present, refuse — solving a different problem. Desktop-owned. |
| **Repository naming** | `cartograph_pipeline::{Repository, repository_name}` | `name` from `root.canonicalize().ok()` falling back to `file_name`; `requested` redacted to `<absolute>` when rooted. A **name**, explicitly *"deliberately not the path"*. |
| **Path redaction** | `cartograph_pipeline::discovery::is_rooted` | the M09 Windows privacy predicate, *"Public because it is a **rule**, not a helper"* |

Two existing behaviours bear directly on any path-based scheme:

- `discovery::walk` **skips symlinks outright** (`if file_type.is_symlink() { continue; }`), so the analyser never follows a link out of the tree.
- `desktop::repository::validate` **deliberately follows a symlinked root**, because *"a symlinked repository … is a normal way to keep a checkout elsewhere"*.

So containment *inside* a tree exists; the identity of the *root* does not.

### Candidates

| | **B1 — path prefix** | **B2 — canonical identity** | **B3 — launch-time grant** | **B4 — handshake tool** |
|---|---|---|---|---|
| Mechanism | session holds an authorized path; a request path must be that path or beneath it | session holds `canonicalize()`d root (and/or repository name); a request is compared after canonicalisation | the authorized repository is fixed by the process's argv at start | the client calls an `authorize` tool once, then queries |
| Security property | simple, but string-prefix comparison is defeated by `..`, by a symlinked root, and on Windows by case and 8.3 forms | resolves `..` and symlinks before comparison | **strongest** — the server literally cannot be told about another repository | scoping depends on the server's own state machine being right |
| Traversal / symlink | must be paired with canonicalisation to be sound; `is_rooted` does **not** canonicalise | `canonicalize` is already used, but only opportunistically (`.ok()`, fallback) — it fails on non-existent paths and needs a defined failure mode | avoided entirely at the boundary | same as whichever of B1/B2 backs it |
| Session lifecycle | undefined | undefined | **process lifetime = session lifetime**; unambiguous | needs explicit begin/end and re-authorization rules |
| `diff` implications | two trees are two paths — **a single authorized path cannot serve `diff` as the CLI does it** (`diff_cmd` runs `pipeline::run` on two separate paths) | same problem, unless identity is per-tree | same problem, unless argv grants two | can grant two, at the cost of a larger trusted surface |
| Multi-repository | prefix set possible | identity set possible | argv list possible | explicitly dynamic |
| Failure semantics | undefined — no MCP refusal vocabulary exists (the CLI has exit codes 0/2/3/4/5/6/7 and `ErrorCode` slugs; MCP has no analogue) | same | same | same |
| Testability | high — pure function over paths | high, but touches the filesystem | high | needs a state machine test |
| Privacy | a refusal must not echo the rejected path (RULE 015; `no_error_message_from_a_rooted_path_contains_its_parents`) | same | same | same |
| Complexity | lowest | low | lowest at the boundary, moves the problem to the launcher | highest |
| Dependencies | none | none | none | none |

**None of these is excluded by evidence, and none is selected by it.** B3 is the
only one whose scoping property is structural rather than enforced by code, but
"structural" is an argument, not a repository fact.

### Single or multiple authorized repositories

A separate question from *how* authorization is established, and the evidence
does not settle it either.

| | **One repository per session** | **Several per session** |
|---|---|---|
| Textual support | ADR-0005 and `ARCHITECTURE.md:176` both say *"the repository"*, singular; `SECURITY.md` likewise | the M15 spec says *"an authorized repo"*, which does not exclude a set |
| Security property | the smallest possible surface: one root, one answer | every added root widens what a compromised or confused client can read |
| `diff` | **cannot serve `diff` as the CLI does it** without defining a second tree (revision? sibling path?) | serves `diff` naturally |
| Lifecycle | trivially defined by the session | needs add/remove semantics and a bound |
| Failure behaviour | any other path is refused — one rule | a set membership test — still one rule, larger state |

Neither reading is contradicted by evidence. The choice is load-bearing because
it decides whether `diff` is on M15's MCP surface at all.

### The `diff` coupling

This is the sharpest concrete consequence. `cartograph_graph::diff::diff(before,
after)` takes **two graphs**, and `diff_cmd` builds them from **two paths**. A
session model that authorizes exactly one repository either cannot offer `diff`
over MCP, or must define what the second tree is — a git revision of the same
repository, a second authorized path, or something else. The specification says
`diff` is in scope and says authorization is "session-scoped repository
authorization only". Those two sentences are not reconciled anywhere.

---

## C. Transport

### Confirmed by repository evidence

- **No transport code of any kind exists.** No socket, listener, port, HTTP
  client or server anywhere in `crates/` — a search for
  `TcpListener|listen(|bind(|localhost|--port` returns only the resolver's
  URL-*parsing* tests and its unrelated `Scope::bind`.
- **No async runtime.** `tokio` is named in the frozen stack at **M04** and was
  never added; `pipeline::run` is synchronous, and
  `cartograph-desktop/src/session.rs` records that it is *"a synchronous call
  with no interruption point"*.
- **RULE 013** — *"The application is fully useful with zero API keys and no
  network."* **ADR-0005** — no source leaves the machine, default and
  unconditional.
- The frozen stack's rejected-for-v1 list includes Axum, and `PROJECT_RULES.md`
  non-goal 7 is *"Any cloud component before a user asks for one and offers to
  pay."*

### Strong inference — stdio

Three facts converge:

1. `cartograph-cli` already contracts **stdout = result, stderr = diagnostics
   and progress** (`main.rs:23–24`) — exactly the partition a JSON-RPC-over-stdio
   server needs.
2. The npm launcher already *"forwards argv, **stdio** and the exit code"*
   (`CHECKPOINTS.md:1271`), so a launch mechanism that inherits stdio exists.
3. No network code exists to build on, and none is wanted.

### Why this is still not confirmed

**Stdio is the strongest inference available and is not a specification
requirement.** The M15 spec names Claude Code and Cursor but **no transport**.
RULE 013 constrains *requiring* a network; it does not forbid a loopback
listener, and a loopback listener sends no source off-machine either. No ADR
states a transport. Choosing stdio would be a decision, not a reading — and
recording it as though it were a reading is precisely the failure this ADR
exists to prevent.

| Option | Evidence for | Evidence against | Label |
|---|---|---|---|
| **C1 — stdio** | the three facts above | none — but no positive requirement either | **strong inference** |
| **C2 — loopback TCP / HTTP** | rmcp supports it; both named clients support it | no listener code, no port convention, no frozen-stack HTTP server, non-goal 7 (*"Any cloud component before a user asks for one and offers to pay"*) | **plausible, unsupported** |
| **C3 — Unix socket** | local-only by construction | Windows is a first-class target — M09's privacy defect was a Windows finding, and CI builds Windows | **plausible, weakened by evidence** |

C1 is an **inference**, not a requirement. C2 and C3 are admissible options
recorded so that choosing C1 is visibly a choice.

---

## D. Dependency boundary

**Confirmed repository architecture** (not a decision):

- `cartograph-cli` is a **binary** crate: `Cargo.toml` declares
  `[[bin]] name = "cartograph"` and no `[lib]`; `cargo test -p cartograph-cli
  --lib` answers *"no library targets found in package `cartograph-cli`"*.
  **Nothing can depend on it**, which is what forced ADR-0019.
- Current shape: `core ← graph ← pipeline ← {cli, desktop}`.
- [ADR-0001](ADR-0001-rust-core-is-the-product.md) and RULE 002: the MCP server
  is a *client* and holds no analysis logic.
- [ADR-0016](ADR-0016-desktop-boundary.md) decision 2 already names the MCP
  server *"a client of that shared pipeline, on equal footing with the CLI"*.
- `rmcp` is named for **M15** and `tokio` for **M04** in
  `agentos/context/tech_stack.md`; **neither is in `Cargo.toml` or
  `Cargo.lock`**. The dependency policy permits adding a named crate at the
  milestone that needs it, so neither crate needs its own ADR — the
  *architecture around them* does.

The intended shape, which no evidence disproves:

```
cartograph-core
      ↑
cartograph-graph        (trace · blast · diff)
      ↑
cartograph-pipeline     (run · run_with_cache · Analysis)
      ↑
cartograph-mcp          ← rmcp + tokio live here and nowhere else
      ↑
Claude Code / Cursor
```

`cartograph-mcp` must **not** depend on `cartograph-cli`, and `-core`, `-graph`
and `-pipeline` must stay free of any MCP or transport dependency.

**Proposed consequence of the decisions above** (not yet decided):

- a new `cartograph-mcp` crate is the only place `rmcp` and `tokio` appear;
- `cartograph-core`, `-graph` and `-pipeline` stay dependency-neutral, because
  nothing about a traversal or a pipeline run is async;
- library-plus-thin-shell, following `cartograph-desktop`. Note that
  [ADR-0016](ADR-0016-desktop-boundary.md) decision 3 put the Tauri shell
  *outside* the workspace **because of the system webview**, a constraint `rmcp`
  does not have — so an MCP binary can remain a workspace member and be covered
  by `cargo test --workspace`.

## E. Query surfaces

| Query | Implementation | Reusable API | Ready for MCP | Blocked by |
|---|---|---|---|---|
| **map** | none | — | **No** | decision A |
| **trace** | `cartograph_graph::trace` (Slice 1) | `trace`, `resolve_symbol`, `near_misses` | Yes | — |
| **blast** | `cartograph_graph::blast` (M12) | `blast_radius` | Yes | — |
| **diff** | `cartograph_graph::diff` (M13) | `diff(before, after)` | Yes, mechanically | decision B — it needs two trees |

One quality consequence to record rather than discover later:
[ADR-0019](ADR-0019-trace-belongs-to-the-graph.md) kept `refusals_near` in the
CLI, because refusals live in `Analysis::results` and reaching them from the
graph would invert the dependency direction. **An MCP `trace` therefore answers
without refusals** unless a later slice lifts them into `cartograph-pipeline`,
which already depends on the resolver. That is a Slice 3 quality question, not a
blocker.

## Decision order

```
A (map)  ────────────────► shapes Slice 2 scope: an extraction is needed
                            only if "map" means the CLI's rendered summary

B (auth) ────────────────► Slice 2 cannot start; it IS Slice 2's content
   └── B ↔ diff: a one-repository session must define diff's second tree,
       or diff leaves M15's MCP surface

C (transport) ───────────► Slice 3 only; does not block Slice 2, but does
                            shape whether cartograph-mcp is one crate or two

D (dependency boundary) ─► follows from C; no dependency is added before it
```

- **B must precede Slice 2**, because Slice 2 *is* the authorization
  abstraction. There is nothing else for Slice 2 to contain.
- **A must precede Slice 2 only if the answer is A1-as-rendered**, which would
  add a `summary_cmd` extraction; A1-from-`Totals`, A2 and A3 need no
  extraction and can be settled before Slice 3.
- **C and D must precede Slice 3**, and no `Cargo.toml` change may happen
  before them.

## Implementation impact

**Slice 2** (after B): an authorization type in a library crate whose
construction is the only route to a query — the `ValidatedRepository` technique
applied to authorization rather than validation; the refusal semantics; privacy
tests extending the existing gate. **No `rmcp`, no `tokio`, no transport, no
tool registration.**

**Slice 3** (after A, C, D): `crates/cartograph-mcp/` with the first `rmcp` and
`tokio`; root `Cargo.toml` members and `[workspace.dependencies]`; `Cargo.lock`;
tool registration for the agreed query set; a CI job; `README.md` and
`docs/`.

## Security implications

The invariant is unchanged and non-negotiable: **MCP exposes only the
session-authorized repository.** Whichever option is chosen must make an
unauthorized request *fail*, and the failure must not echo a rooted path or its
parent directories (RULE 015). The existing privacy gate is the floor:
`no_json_document_contains_an_absolute_machine_path`,
`a_rooted_drive_less_path_never_reaches_serialised_output` (both
`cartograph-cli/tests/cli.rs`) and
`no_error_message_from_a_rooted_path_contains_its_parents`
(`cartograph-desktop/tests/lifecycle.rs`) are what "privacy gate extended to
MCP" must mean concretely. `SourceLocation` already refuses absolute paths at
construction, and `discovery::walk` already skips symlinks — neither is
sufficient on its own to scope a *root*.

## Validation requirements

M15's acceptance — *"MCP client can query the graph of an authorized repo **and
nothing else**"* — is a claim about a live client, so a real Claude Code or
Cursor session is the evidence, at the standard M14's dogfood set. One measured
constraint to design against: on this machine an Airflow analysis takes
**3m42s** and Zulip **51s**, and every CLI query re-analyses from scratch. A
session answering repeated queries will need `pipeline::run_with_cache` and
`IncrementalState`, or a client will time out.

## Consequences

Recording these as open costs a slice of elapsed time and buys the thing the
project's rules are built around: a decision that was made rather than defaulted
into. RULE 019 requires an ADR for a significant architecture change, and RULE
009 — unknown values remain unknown — applies to the repository's own design as
much as to its graph.

Until this ADR is accepted, **no dependency may be added and no MCP code
written**. Slice 2 is blocked on B; Slice 3 is blocked on A, C and D.

## Decision rule

Until an owner selection is recorded in this ADR, or an authoritative
repository source is found that settles a question, **no implementation may
treat any option here as chosen.** In particular none of the following is a
decision:

> "most likely" · "obvious" · "industry standard" · "usual MCP behaviour" ·
> "strongly implied" · "what every other server does"

RULE 009 — unknown values remain unknown — is a rule about the graph, and it
applies to the project's own design for the same reason: a plausible-looking
guess recorded as a fact is worse than a gap, because nobody goes back to check
it.

## Owner Decision Required

Four choices. `Owner choice` stays **UNDECIDED** until the owner fills it in;
this ADR then moves to `Accepted` recording what was chosen and why.

| Decision | Options | Repository evidence | Consequence | Owner choice |
|---|---|---|---|---|
| **A — what "map" is** | A1 analysis summary · A2 the graph · A3 the render scene · A4 a filtered/aggregated view | **None selects one.** MAP is defined by `README.md` as *"what is this system?"* and realised only as the desktop app. TRACE is absent from that table, so the spec's list is not homogeneous. | A1 needs no extraction; A2 is 2 MB on Airflow; A3 is owned by a peer client; A4 is new analysis work in the core | **UNDECIDED** |
| **B-mechanism — how authorization is established** | B1 path prefix · B2 canonical identity · B3 launch-time grant · B4 handshake tool | Four one-sentence invariants state *that* MCP is session-scoped; **none states a mechanism**, and no code implements one | Determines all of Slice 2. B3 is structural; B1/B2/B4 are enforced by code | **UNDECIDED** |
| **B-cardinality — how many repositories per session** | one · several | ADR-0005, `ARCHITECTURE.md:176`, SECURITY.md say *"the repository"* (singular); the spec says *"an authorized repo"* | Decides whether **`diff` can be on M15's MCP surface at all** | **UNDECIDED** |
| **C — transport** | C1 stdio · C2 loopback TCP/HTTP · C3 Unix socket | No transport is named anywhere. C1 is a strong inference from three facts; it is **not** a requirement | Shapes `cartograph-mcp` and gates the first `rmcp`/`tokio` dependency | **UNDECIDED** |

An inference may be recorded in the discussion above — C1 is — but an inference
never belongs in the `Owner choice` column.

## Alternatives

- **Decide inside the implementation** — rejected; it is what this ADR exists
  to prevent, and it would leave three architectural choices invisible in a
  diff.
- **Three separate ADRs** — rejected: the three questions interact (B ↔ diff,
  A ↔ Slice 2 scope, C ↔ crate shape), and
  [ADR-0016](ADR-0016-desktop-boundary.md) is the precedent for one *boundary*
  ADR carrying several decisions.
- **Amend ADR-0019** — rejected: it is accepted, it records an extraction, and
  it explicitly warns that *"a later slice must not read agreement into its
  silence."* Amending it would blur the line it drew.
