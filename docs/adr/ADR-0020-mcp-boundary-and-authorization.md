# ADR-0020 — MCP boundary and authorization model

**Status:** Accepted · 2026-09-03 · Governs M15 · Follows
[ADR-0019](ADR-0019-trace-belongs-to-the-graph.md) · Implements
[ADR-0001](ADR-0001-rust-core-is-the-product.md) · Enforces
[ADR-0005](ADR-0005-local-first-privacy.md)

> **Amended 2026-09-03 — see *Amendment 1*, below, which is accepted.**
> The four decisions below stand unchanged. The deferred item *"how the
> canonical identity is derived"* turned out not to be a free choice:
> investigation found that **no derivation from the analysed tree can satisfy
> the diff rule this ADR also states**. Amendment 1 records that contradiction
> and the owner's resolution — **R2, grant-supplied repository identity** —
> together with the trust boundary it creates. This ADR is not rewritten.

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
| **How the canonical identity value is derived** | Still open. Amendment 1 decided *where identity comes from* — the grant — and deliberately did **not** decide what the value is. No `gix`, no populated `CommitId`, no durable fingerprint exists to derive one from |
| **How a session receives its initial grant** | Choosing *canonical identity, one per session* does **not** imply a launch argument, a handshake tool, an environment variable or a config file. The owner selected the authorization property, not the grant channel |
| **A4's concrete data model** | Accepted as a decision and a placement constraint; its shape is its own slice |
| **The wire-level refusal code** | Belongs to the transport slice |

The grant channel must not be allowed to mutate the property: whatever delivers
the grant, *one canonical identity, bound to every query, refused before
analysis* is fixed.

## Amendment 1 — the identity source, and the two-tree contradiction

**Status: Accepted, 2026-09-03.** The four decisions above are unaffected;
this resolves the *source* of the identity they assume. The owner selected
**R2 — grant-supplied repository identity**.

### The contradiction

Two things this ADR states are, together, unsatisfiable by any derivation from
the analysed tree:

1. **"One MCP session has exactly one authorized repository identity"**, derived
   as a *canonical repository identity*; and
2. **"Both trees of a diff must resolve to the same authorized repository
   identity"**, over a `diff` that takes **two filesystem paths**
   (`diff_cmd` calls `pipeline::run` twice; `graph::diff` takes two graphs).

The production path that breaks it is Cartograph's own, running today on `main`:

```
architecture-review.yml
  trees/base  ← actions/checkout of  github.repository      (Rexy-5097/cartograph)
  trees/head  ← gh api …/tarball/$SHA | tar -xz             (ronitsaha11/cartograph)
  ./target/release/cartograph diff trees/base trees/head --markdown
```

Two facts from that workflow, both verified rather than assumed:

- **The trees are asymmetric.** `trees/base` is a checkout and *has* `.git`;
  `trees/head` is a tarball extraction and has none — the runner logged
  `head tree: 663 files, no .git: yes` (run 33733818164).
- **For a fork pull request they come from different repositories.** Base is
  `github.repository`; head is
  `github.event.pull_request.head.repo.full_name`, observed as
  `HEAD_REPO: ronitsaha11/cartograph`. A Git-remote identity would call
  Cartograph's own before/after pair **two different repositories** and refuse
  the review that M14 was accepted on.

M13's evidence has the same shape: *"Airflow `ed68491d8b` → `9b43d6abc0`, two
revisions extracted read-only"* — two directories, compared by path.

### What any option must establish

| | Proposition |
|---|---|
| P1 | `before` belongs to the authorized repository |
| P2 | `after` belongs to the authorized repository |
| P3 | `before` and `after` are the **same** repository |
| P4 | **`before` = repository A with `after` = repository B is refused** |

P4 is the one that makes the others worth having. An option that satisfies
P1–P3 by weakening P4 has not solved the problem; it has renamed it.

### Option R1 — containing-root authorization

The session authorizes a canonical root **C**; a query tree **T** is admitted
when `canonicalize(T)` is `C` or beneath it.

| | |
|---|---|
| Relation of the trees to the root | In the M14 workflow `trees/base` and `trees/head` are siblings inside the runner workspace, so a containing root **does** exist |
| Symlink / junction escape | **Resisted, measured.** On this Windows machine `canonicalize()` resolves a directory junction to its target: `…\link` → `…\real`, identical to the target's own canonical form. A junction pointing outside **C** therefore canonicalizes outside **C** and fails containment — provided both sides are canonicalized |
| Windows behaviour | **Measured.** `canonicalize()` normalises separators, `..`, drive case, component case and trailing slash — six aliasing forms of one path all compared equal — and returns a `\\?\`-prefixed absolute path that names the machine and must never be echoed (RULE 015, `is_rooted`) |
| Extracted trees inside one root | Yes — this is the case the M14 workflow already produces |
| Does Cartograph's own workflow satisfy it? | **Mechanically yes**, and that is the problem: the root it would authorize is the runner workspace — a scratch directory that also holds the base source and the built binary — not a repository |
| Security guarantee | **Containment, not identity.** Honestly named it is an *authorized subtree* |
| P4 | **Not satisfied.** Two unrelated repositories placed under one authorized root both pass. R1 cannot tell A-before/B-after from a legitimate pair, because it never learns what a repository is |
| Residual | `canonicalize()` **errors on a nonexistent path**, so identity exists only while the tree does; and a directory replaced by a junction between check and analysis is a TOCTOU window |

R1 is coherent and enforceable. It is not the property this ADR accepted, and
adopting it would mean saying so.

### Option R2 — grant-supplied identity

Identity becomes part of the **authorization state** rather than something
discovered in the directory. The grant binds one identity to the tree or trees
that represent it.

| | |
|---|---|
| How the trees prove correspondence | They do not. The grant asserts it, and the guard enforces *membership of the granted set* |
| Where the proof happens | **Outside the analyser**, at grant time — by whoever knows that `trees/base` and `trees/head` are two views of one repository. In the M14 workflow that knowledge exists in the workflow, which fetched both |
| Must the grant carry metadata? | Yes — at minimum an identity and the paths it covers. **What that metadata is remains the separately deferred grant-channel question, and this option does not decide it** |
| Diff | Natural: one identity, two granted trees, P3 satisfied by the grant |
| Non-Git repositories | **Unaffected** — nothing is derived from the tree, so a `.git`-less extraction is as authorizable as a clone |
| Security assumption | Trust moves to the grant channel. The tree proves nothing; the guard enforces exactly what it was told |
| Session lifecycle | Identity lives for the session, which is what "one session, one identity" already says |
| P4 | **Satisfied**, conditionally: A-before/B-after is refused because B was never granted. The condition is that the granter is correct — the assertion is only as good as its source |
| Honest characterisation | Identity becomes an **assertion**, not a derivation. Whether an asserted identity is a *canonical repository identity* in the sense accepted above is exactly what the owner must decide |

### Option R3 — `diff` is not on M15's MCP surface

One identity per session stands; `diff` waits for a mechanism that can connect
two extracted trees.

| | |
|---|---|
| Effect on the acceptance sentence | **None literally.** M15's acceptance is *"MCP client can query the graph of an authorized repo and nothing else; privacy gate extended to MCP; gates pass"* — it does **not** enumerate the four queries. The enumeration is in the *Scope* line |
| Does the scope line require all four? | It reads *"exposing graph queries (map/trace/blast/diff)"* — a parenthetical list. Narrowing a scope line has precedent: [ADR-0014](ADR-0014-m10-scope-reconciliation.md) is *"the governing record of M10's delivered scope"* after three scope items were deferred |
| Usefulness to Claude Code / Cursor | `map`, `trace` and `blast` answer three of the product's questions; `diff` is the one an agent reviewing a change would reach for |
| Product cost | `ROADMAP.md:68` names **DIFF the retention surface**. Excluding it from the agent surface is a product decision, not only an engineering one |
| Would the specification need rewriting? | Not rewritten — **reconciled**, in the ADR-0014 manner, recording what M15 delivered and why |
| P1–P4 | Vacuous for `diff`; unchanged for the single-tree queries |

### Decision — R2, grant-supplied repository identity

**The authorized repository identity is trusted session authorization state,
supplied by the component that establishes the session. It is not inferred from
the analysed tree.**

R1 was rejected because it authorizes a filesystem *location*: two unrelated
repositories beneath one authorized root both pass, so it cannot refuse
`before` = repository A with `after` = repository B — the case the diff rule
exists for. Adopting it would have quietly replaced *repository identity* with
*authorized location*.

R3 was rejected because it removes `diff` rather than answering the question,
and `map`, `trace` and `blast` would still need an identity source. It also
costs the retention surface (`ROADMAP.md:68`) on the agent surface.

R2 fits how Cartograph's own analysis is actually driven. The M14 workflow
already knows `github.repository`, the base SHA, the head repository and the
head SHA; it then hands the analyser **two filesystem paths and nothing else**.
The knowledge that those two trees are one repository exists in the component
that fetched them, and nowhere in the trees. R2 puts the identity where the
knowledge already is.

#### The trust boundary this creates, stated plainly

> **The component that establishes the session is part of the trust boundary.**
> It asserts the repository identity and which trees represent it. The analysed
> tree is **not** authoritative proof of anything, and the guard does not
> attempt to verify the assertion.

This is a conscious architectural choice, and it must not be described as
"validated repository", "secure path" or "canonical path" — those are different
properties, and two of them are the ones this amendment rejected.

What the boundary does and does not defend:

- **Defends:** an MCP client cannot widen its own scope. It presents a tree; the
  session decides whether that tree was granted, and refuses before any
  analysis.
- **Does not defend:** a compromised or mistaken grant producer. It can
  authorize anything — but it is the process that launched the server and could
  have read those files directly. The guard's job is to stop the *client*
  widening scope, not to defend against its own launcher.

#### What R2 does and does not settle

| | |
|---|---|
| **Settled** | identity is authorization state, asserted at grant time, never derived from a query path |
| **Settled** | one identity per session; every query resolved against it before analysis |
| **Settled** | for `diff`, both trees must be associated with the *same* authorized identity, checked before either analysis runs |
| **Not settled** | the identity **value** — no algorithm is chosen, and none may be inferred |
| **Not settled** | the **grant mechanism** — not a launch argument, environment variable, handshake or config file until decided |

The four terms this amendment keeps apart, because collapsing any two of them is
how the contradiction was reached in the first place:

| Term | Means |
|---|---|
| identity **property** | one canonical identity per session — **accepted, unchanged** |
| identity **source** | **decided: the grant, not the tree** |
| **authorization state** | what the session holds and enforces on every query |
| **initial grant** | how that state is first supplied — **still deferred** |

#### What this permits Slice 2 to build

An opaque identity type whose value arrives from the grant; a session holding
exactly one such identity together with the trees the grant associated with it;
a guard that admits a query only when its tree was granted for that identity,
and a paired guard for `diff` that requires both trees to carry it. Nothing in
that list requires the identity value or the grant channel to be chosen, which
is why Slice 2 can proceed while both remain deferred.

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
