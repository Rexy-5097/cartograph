# ADR-0020 — MCP boundary and authorization model

**Status:** Accepted · 2026-09-03 · Governs M15 · Follows
[ADR-0019](ADR-0019-trace-belongs-to-the-graph.md) · Implements
[ADR-0001](ADR-0001-rust-core-is-the-product.md) · Enforces
[ADR-0005](ADR-0005-local-first-privacy.md)

## Decisions recorded on acceptance

Four choices, selected by the project owner. This ADR was `Proposed` while they
were open; it now records what was weighed and what was chosen.

| | Decision | Selected |
|---|---|---|
| **A** | What "map" denotes as an MCP query | **A4** — a purpose-built, filtered and aggregated representation answering *"what is this system?"* |
| **B — mechanism** | How a repository becomes session-authorized | **B2** — canonical repository identity |
| **B — cardinality** | How many repositories a session may hold | **One**, exactly |
| **C** | Transport | **C1** — stdio |

Two things remain **deliberately open**, recorded under *Deferred* rather than
guessed at: **how the canonical identity is derived**, because the repository
contains no such algorithm, and **how a session receives its initial grant**.
Neither may be settled by an implementer's inference — see *Decision rule*.

## Context

M15's scope is *"rmcp-based MCP server exposing graph queries
(map/trace/blast/diff) to Claude Code and Cursor. Session-scoped repository
authorization only"*, and its acceptance is *"MCP client can query the graph of
an authorized repo and nothing else; privacy gate extended to MCP; gates pass"*
(`agentos/milestones/M15-mcp-server.md`).

Slice 1 ([ADR-0019](ADR-0019-trace-belongs-to-the-graph.md), merged at
`5d88405`) moved the trace traversal into `cartograph-graph` and **explicitly
deferred** these questions, recording that *"a later slice must not read
agreement into its silence."* This ADR is where they were put to the owner and
answered.

The invariant they serve is not reopened:

> **MCP exposes only the repository authorised in the current session.**
> — [ADR-0005](ADR-0005-local-first-privacy.md), `ARCHITECTURE.md:176`,
> `SECURITY.md`, `docs/security/README.md` ("MCP session scoping | M15")

## Decision

### A. "map" is a purpose-built aggregated view — A4

MAP answers *"what is this system?"* (`README.md`'s surface table). The MCP
`map` query answers that question in a form an agent can read: a compact,
deterministic, **filtered and aggregated** representation of the graph.

It is explicitly **none of these**:

| Not | Why not |
|---|---|
| the raw `ArchitectureGraph` | **2,054,974 bytes of JSON on Airflow** (3,104 nodes / 3,350 edges); Zulip 747,079. Unusable as a response, and dumping it answers no question |
| `cartograph_desktop::scene::Scene` | owned by a **peer client**, and its x/y coordinates are meaningless to a text agent. Consuming it would invert the client boundary |
| `json::Summary` / `pipeline::Totals` | answers a **different question** — *"can Cartograph analyse this repository, and what did it find?"* (`summary_cmd.rs:5–6`) — and must not be silently relabelled as MAP |

**A4 does not exist yet.** Nothing in the repository produces it.
`summary_cmd`'s `count_by_kind` / `count_by_provenance` /
`count_by_confidence` are the nearest thing, and they key on **presentation
label strings**, so kinds sharing the `"edge"` fallback merge — a rendering
decision baked into a count, not an API.

**A4 is therefore a new capability, and it is analysis.** RULE 002 and
[ADR-0001](ADR-0001-rust-core-is-the-product.md) put it in a **reusable library
layer** — `cartograph-graph` or `cartograph-pipeline`, whichever its eventual
shape needs — and **not** in `cartograph-cli` (a binary crate; nothing can
depend on it) and **not** in `cartograph-mcp` (a client holds no analysis).

Its concrete data model is **not designed here**. This ADR records the decision
and the placement constraint; the shape is the subject of its own slice, and
therefore sits **outside the authorization slice**.

### B. Authorization is by canonical repository identity, one per session

#### The security invariant

> **One MCP session has exactly one authorized repository identity. Every graph
> query is bound to that identity. A query naming any other repository is
> refused before any analysis runs.**

Three parts, each load-bearing:

1. **Exactly one.** Not a set, not a default, not "the first one seen".
2. **Bound, not checked once.** Every query carries the binding; authorization
   is not a gate passed at startup and then forgotten.
3. **Refused *before* analysis.** No parsing, no file reads, no graph
   construction for an unauthorized repository. A refusal that arrives after
   the analyser has walked the tree has already done the thing it was meant to
   prevent.

#### Identity is not a filesystem path

The same repository can be named `.`, `../cartograph`, `D:\dev\cartograph`,
through a symlink, or with different case on Windows. A prefix comparison over
raw paths is defeated by `..`, by a symlinked root, and on Windows by case and
short-name forms.

The repository's own behaviour is split on exactly this point, which is why the
distinction must be stated rather than assumed:

- `discovery::walk` **skips symlinks outright**
  (`if file_type.is_symlink() { continue; }`), so the analyser never follows a
  link *out of* a tree;
- `desktop::repository::validate` **deliberately follows a symlinked root**,
  because *"a symlinked repository … is a normal way to keep a checkout
  elsewhere"*.

Containment inside a tree exists. Identity of the root does not.

#### What the repository provides — and does not

Investigated, with the result stated plainly rather than worked around:

| Candidate mechanism | Present? | Finding |
|---|---|---|
| Git remote / commit identity | **No** | `gix` is named in the frozen stack at M07 and **is not a dependency**. `CommitId` exists as a type but is never populated from a real repository — every production call site passes `None`; the only literal is a test in `edge.rs:207` |
| Canonical path representation | **Partial** | `root.canonicalize()` appears **once**, in `pipeline::repository_name`, as `.ok()` with a fallback, and only to derive a display *name*. `discovery::discover` returns the root **as given, uncanonicalised** |
| Repository fingerprint | **No** | `incremental::content_hash` is per-*file*, uses `DefaultHasher`, and its own documentation says it is *"not a security boundary"* and *"not stable across Rust releases"* |
| Workspace identity | **No** | nothing of the kind exists |
| `NodeIdentity`-style canonicalisation | **Not applicable** | `cartograph_core::identity` is *node* identity `(kind, name, file)` **within one graph**; it says nothing about which repository a graph came from |
| `Repository { name, requested }` | **Not an identity** | `name` is a display string, *"deliberately not the path"*, and is not unique — two checkouts of one repository share it, and two unrelated repositories can too |

**There is no existing repository-identity mechanism in this repository.**

This ADR therefore accepts the **property**, not an algorithm:

> Session authorization binds **one canonical identity** to **one repository**,
> where that identity is stable across the different ways a single repository
> can be named, and distinct between different repositories.

**How that identity is derived is deferred.** Inventing a fingerprint here,
with no evidence to select one, is the failure this ADR exists to prevent.

#### The one-repository rule, and what it does to `diff`

One authorized repository per session is deliberate: it is the smallest
surface, and it matches the singular wording of every statement of the
invariant — *"the repository"*.

`cartograph_graph::diff::diff(before, after)` takes **two graphs**, and
`diff_cmd` builds them from **two paths**. The rule therefore carries a
consequence that must be enforced, not merely noted:

> **Both trees of a diff must resolve to the same authorized repository
> identity, and authorization validates both before the diff runs.**

`before` = repository A with `after` = repository B is refused **even when both
paths are individually valid and readable**. A diff across two repositories is
not a diff; it is a way to read a second repository through a query that was
authorized for the first.

What the second tree legitimately *is* — a revision, a worktree, a sibling
checkout of the same repository — depends on how identity is derived, and is
therefore part of that deferred decision.

#### Refusal semantics

An unauthorized request is refused **before analysis**, as an internal outcome
of the authorization layer. **No wire-level error code is fixed here.** Mapping
the internal refusal onto an MCP protocol error belongs to the transport slice;
fixing a code before the transport exists would pin a contract nothing has
exercised.

The refusal must obey the existing privacy gate: it may not echo a rooted path
or its parent directories (RULE 015, and the tests named under *Security
implications*).

### C. Transport is stdio — C1

Accepted. The owner's choice resolves what this ADR previously recorded as an
inference. The reasoning it rests on:

- **RULE 013** — *"fully useful with zero API keys and no network"*;
  [ADR-0005](ADR-0005-local-first-privacy.md) — no source leaves the machine,
  default and unconditional.
- The CLI already contracts **stdout = result, stderr = diagnostics and
  progress** (`main.rs:23–24`) — the partition a JSON-RPC-over-stdio server
  needs.
- The npm launcher already *"forwards argv, **stdio** and the exit code"*
  (`CHECKPOINTS.md:1271`), so a launch mechanism that inherits stdio exists.
- **No listener exists to build on**: no socket, port, HTTP client or server
  anywhere in `crates/`.
- The frozen stack rejects Axum for v1, and non-goal 7 is *"any cloud component
  before a user asks for one and offers to pay"*.

Recorded honestly, because the distinction matters for how this ADR should be
read: before the owner chose, stdio was the **strongest inference and not a
requirement** — no source names a transport, and RULE 013 constrains *requiring*
a network rather than forbidding a loopback listener. It is now a decision,
which is a different thing from a reading.

### D. Dependency boundary

```
cartograph-core
      ↑
cartograph-graph        trace · blast · diff   (+ A4, if it lands here)
      ↑
cartograph-pipeline     run · run_with_cache · Analysis   (+ authorization)
      ↑
cartograph-mcp          rmcp · tokio · stdio · tool registration
      ↑
Claude Code / Cursor
```

- `cartograph-mcp` **must not depend on `cartograph-cli`** — a binary crate
  (`[[bin]] name = "cartograph"`, no `[lib]`; `cargo test -p cartograph-cli
  --lib` answers *"no library targets found"*). That fact is what forced
  [ADR-0019](ADR-0019-trace-belongs-to-the-graph.md).
- `cartograph-core`, `-graph` and `-pipeline` **stay free of `rmcp`, `tokio`
  and any transport dependency**. Nothing about a traversal or a pipeline run
  is asynchronous.
- **A4 does not live in `cartograph-mcp`** merely because MCP consumes it.
- `rmcp` (frozen stack, M15) and `tokio` (frozen stack, M04) are **not in the
  workspace's `Cargo.toml` or `Cargo.lock`** — zero entries for either — and
  arrive with the transport slice, not before. (`tokio` does appear in
  `desktop/src-tauri/Cargo.lock` as a Tauri transitive dependency; that crate is
  `exclude`d from the workspace by
  [ADR-0016](ADR-0016-desktop-boundary.md) decision 3 and is not a workspace
  dependency.)

### E. Where the authorization type lives

Derived from the invariant rather than from convenience. The requirement is
that authorization is enforced **below the transport layer**, so that no client
can bypass it.

| Candidate | Assessment |
|---|---|
| `cartograph-mcp` | **Rejected.** It *is* the transport layer. A guard living there is bypassed by every other client, which is what "below the transport layer" forbids |
| `cartograph-core` | **Rejected.** Core is the domain model — nodes, edges, evidence, confidence. Which repository a caller may analyse is not a property of an edge |
| `cartograph-graph` | **Rejected.** Graph queries operate on an `&ArchitectureGraph` that **already exists**; by then the analysis has run, so a guard here cannot refuse *before analysis* |
| `cartograph-pipeline` | **Selected.** It owns `run(path, Options) -> Analysis` — the exact boundary where a path becomes an analysis, and therefore the only place a refusal can precede one. It is a library every client can use, and `cargo test --workspace` already covers it |
| a new library crate | Admissible if the type outgrows what `cartograph-pipeline` should hold. Not justified for one type today, and a crate added speculatively is the dependency policy's stated failure mode |

## Deferred, deliberately

Not settled by this ADR. An implementation may not treat them as settled:

| Deferred | Why it is still open |
|---|---|
| **How the canonical identity is derived** | The repository provides no mechanism — no `gix`, no populated `CommitId`, no durable fingerprint, and `canonicalize()` used only opportunistically for a display name. The property is accepted; the algorithm needs its own evidence and its own decision |
| **How a session receives its initial grant** | Choosing *canonical identity, one per session* does **not** imply a launch argument, a handshake tool, an environment variable or a config file. The owner selected the authorization property, not the grant channel |
| **A4's concrete data model** | Accepted as a decision and a placement constraint; its shape is its own slice |
| **The wire-level refusal code** | Belongs to the transport slice |

The grant channel must not be allowed to mutate the property: whatever delivers
the grant, *one canonical identity, bound to every query, refused before
analysis* is fixed.

## Decision rule

No implementation may treat a deferred item as decided on the strength of
*"most likely"*, *"obvious"*, *"industry standard"*, *"usual MCP behaviour"* or
*"strongly implied"*. Only an explicit owner selection recorded here, or an
authoritative repository source, settles one. RULE 009 — unknown values remain
unknown — governs the project's own design for the same reason it governs the
graph: a plausible guess recorded as a fact is worse than a gap, because nobody
returns to check it.

## Slice map

| Slice | Owns | State |
|---|---|---|
| **1** | trace traversal extracted into `cartograph-graph` | **Complete** — PR #38, merged at `5d88405` |
| **2** | **the authorization boundary**: canonical repository identity, single-repository session context, construction and validation of an authorized session, the query guard, refusal semantics, privacy tests | next |
| **3** | `cartograph-mcp`: `rmcp`, `tokio`, stdio transport, tool registration, session lifecycle integration, real Claude Code / Cursor validation | after 2 |
| **A4** | the aggregated map capability, in a reusable library layer | **may need its own slice**, depending on its concrete API; it is not part of the authorization boundary |

Slice 2 explicitly does **not** own `rmcp`, `tokio`, stdio, tool registration,
the wire protocol, or the MAP implementation.

## Query readiness

| Query | Status |
|---|---|
| **map** | **Not implemented** — A4 is a new capability in a library layer |
| **trace** | Ready at the graph layer (`cartograph_graph::trace`, Slice 1) |
| **blast** | Ready at the graph layer (`cartograph_graph::blast`, M12) |
| **diff** | Ready at the graph layer (`cartograph_graph::diff`, M13), **subject to** both trees resolving to the one authorized identity |

One inherited consequence, recorded rather than discovered later:
[ADR-0019](ADR-0019-trace-belongs-to-the-graph.md) kept `refusals_near` in the
CLI, because refusals live in `Analysis::results` and reaching them from the
graph would invert the dependency direction. **An MCP `trace` therefore answers
without refusals** unless a later slice lifts them into `cartograph-pipeline`,
which already depends on the resolver.

## Security implications

The invariant is unchanged and non-negotiable: **MCP exposes only the
session-authorized repository.** Placing the guard in `cartograph-pipeline`
rather than in the transport is what makes that true for *every* client, rather
than for the one that happens to be well-behaved.

The existing privacy gate is the floor that "privacy gate extended to MCP" must
clear:

- `no_json_document_contains_an_absolute_machine_path` (`cartograph-cli/tests/cli.rs`)
- `a_rooted_drive_less_path_never_reaches_serialised_output` (same file)
- `no_error_message_from_a_rooted_path_contains_its_parents` (`cartograph-desktop/tests/lifecycle.rs`)

`SourceLocation` already refuses absolute paths at construction and
`discovery::walk` already skips symlinks; neither scopes a *root*, which is
what Slice 2 adds.

## Validation requirements

M15's acceptance — *"MCP client can query the graph of an authorized repo **and
nothing else**"* — is a claim about a live client, so a real Claude Code or
Cursor session is the evidence, at the standard M14's dogfood set. A measured
constraint to design against: on the development machine an Airflow analysis
takes **3m42s** and Zulip **51s**, and every query re-analyses from scratch. A
session answering repeated queries will need `pipeline::run_with_cache` and
`IncrementalState`, or a client will time out.

## Alternatives

Recorded because they were weighed, not to pad the decision.

**For A.** *The analysis summary* — cheapest, needs no extraction, and answers
a different question; relabelling it MAP would make the tool quietly claim
something it does not do. *The raw graph* — literally "the graph", and two
megabytes on a real repository. *The render scene* — answers MAP's question and
is owned by a peer client, so consuming it inverts the client boundary and
ships coordinates to something that cannot draw.

**For B.** *Path-prefix authorization* — simplest, and defeated by `..`, a
symlinked root, and Windows case and short-name forms. *Launch-time grant* —
the only option whose scoping is structural rather than enforced by code, but
it fixes the grant channel, which the owner deliberately left open. *Handshake
tool* — most flexible, largest trusted surface, and it needs a state machine
before there is a transport to run it on. *Several repositories per session* —
serves `diff` naturally, and widens what a confused or compromised client can
read.

**For C.** *Loopback TCP or HTTP* — supported by rmcp and by both named
clients, with no listener code, no port convention and non-goal 7 against it.
*Unix socket* — local by construction, and Windows is a first-class target:
M09's privacy defect was a Windows finding, and CI builds Windows.

**For E.** Putting the guard in `cartograph-mcp` — rejected above; it is the
one placement that makes the invariant depend on the client behaving.

## Consequences

M15 can proceed, and the order is fixed: Slice 2 builds the authorization
boundary in `cartograph-pipeline`; Slice 3 adds `cartograph-mcp` with `rmcp`,
`tokio` and stdio; A4 lands in a library layer once its shape is decided. No
dependency is added before Slice 3.

Two decisions remain open and are named above rather than absorbed into code.
Until the identity derivation is chosen, Slice 2 can define the *type* and the
*enforcement point* — the property is fixed — but cannot claim that any
particular identity algorithm is correct.
