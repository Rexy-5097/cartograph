# ADR-0021 — The ASK boundary: surfaces, citation contract, and the first network edge

**Status:** Proposed · 2026-09-05 · Governs M16 · Proposed with M16 Slice 1 · Enforces
[ADR-0007](ADR-0007-no-llm-graph-construction.md) and
[ADR-0005](ADR-0005-local-first-privacy.md) · Follows
[ADR-0020](ADR-0020-mcp-boundary-and-authorization.md)

> **Not accepted.** Two decisions in it are deliberately left open — **C**
> (AI provider and HTTP client) and **D** (keychain crate) — because the
> document that authorises dependencies is not in this repository. See
> *Deferred, deliberately*. An implementation may not treat C or D as settled.
>
> **Amended 2026-09-05 — two amendments.** ***Amendment 1***: the
> specification is now available and has been read; it names no crate for C
> or D. ***Amendment 2*** proposes the crate for **D**, as an implementation
> choice this repository makes rather than frozen-stack content. **C remains
> deferred.**

## Context

M16 is *ASK*: opt-in, per-repository AI explanation over already-derived
subgraph evidence. Its acceptance sentence is

> "ASK answers cite edge evidence verbatim; with AI disabled the feature
> degrades to showing the raw evidence; gates pass."

Three properties of the repository make this a larger step than its one-line
scope suggests, and each is established rather than assumed:

- **It is the first network code in the Rust workspace.** No HTTP client is a
  dependency of any crate. Every network call in the project today lives in
  CI's Node helper (M14), not in the binary.
- **It is the first credential.** `docs/security/README.md` names M16 as the
  arrival of OS keychain storage, "first credential: opt-in AI key". No
  keychain dependency exists.
- **There is nowhere to persist per-repository state.** `redb` is not a
  dependency; `ARCHITECTURE.md` says storage "lands at M09/M10" and it did not;
  `cartograph-pipeline/src/incremental.rs` states the cache "writes nothing".

Against that, the raw material ASK explains already exists and is well shaped.
`Edge` carries `evidence`, `provenance`, `confidence` and `location` as
mandatory constructor arguments, and its own documentation anticipates this
milestone: *"A model may later explain an edge that already exists."*
`cartograph-graph` produces subgraphs through `trace` and `blast_radius`, and
`cartograph-desktop/src/evidence.rs` already fetches and displays the evidence
behind one selected edge.

## Decision

### A. ASK is a desktop surface and an MCP surface. Not a CLI command.

`vision.md` lists the product surfaces as MAP (M11) → BLAST (M12) → DIFF (M13)
→ ASK (M16), and the desktop is where a person selects an edge and asks what it
means. The degraded mode the acceptance sentence requires — "showing the raw
evidence" — is substantially what `cartograph-desktop/src/evidence.rs` already
does, so the desktop gives that clause a home rather than a reimplementation.

MCP is the second surface because M15 established the agent-facing surface and
an explanation belongs beside `map`, `trace`, `blast` and `diff`. The
authorization guard sits in `cartograph-pipeline`, below the transport
(ADR-0020), so an ASK tool inherits it without restating it.

**The CLI does not get an `ask` command in M16.** This is a scope decision with
a mechanical cost behind it: every client command that emits JSON has bumped the
output contract — `1.0` at M09, `1.1` with blast at M12, `1.2` with diff at M13
— so a CLI `ask` implies schema `1.3`, a new schema file, and golden-file
compatibility work. The M16 specification does not ask for a CLI surface, and
scope is not widened without evidence that requires it.

### B. Per-repository opt-in lives in a small OS-level configuration file.

The state is a per-repository boolean and the minimum needed to attach it to a
repository. It is **not** a settings system: PROJECT_RULES non-goal 5 names "a
settings panel" as a way to feel productive while avoiding the resolver, and
this decision buys the smallest thing that satisfies "opt-in, per repository,
off by default" and nothing more.

What the file may hold:

- which repositories have opted in, and nothing about their contents
- an explicit off-by-default: absence of an entry means AI is off

What the file may **never** hold, as a hard constraint from SECURITY.md
("Credentials live in the OS keychain — never in configuration files"):

- an API key, in plaintext or any encoding
- source text, evidence text, or any repository content
- environment variable values

**The exact filesystem location is not decided here.** The repository has no
config-directory abstraction, and choosing one on Windows and Unix is a platform
question that needs platform evidence rather than a preference. The decision is
*"a small OS-level configuration file, key-free"*; the path is an implementation
detail that its slice must establish and justify.

### C. AI provider and HTTP client — **not decided.** See *Deferred*.

### D. Keychain crate — **not decided.** See *Deferred*.

### E. The citation contract

"Cite edge evidence verbatim" is given a mechanical meaning, because an
acceptance criterion that only a human can adjudicate cannot be a gate.

**An ASK answer is a list of explanation items. Each item carries one or more
structured citations. An item with zero citations is refused, and an answer
containing such an item is refused as a whole.**

One citation per item is too strict — a claim about a TypeScript call reaching a
database table spans three edges and honestly cites all three — and zero is
what RULE 007 exists to prevent: an uncited sentence is interpretation wearing
the appearance of a derived fact.

A citation is structured, not prose. It identifies the edge it cites and
carries the text it quotes. Four properties are machine-checkable and are the
acceptance test for clause 1:

1. `citation.text` is an **exact substring**, byte for byte, of the cited
   edge's `Evidence::as_str()`.
2. `citation.edge` resolves to an edge **present in the bundle that was sent**,
   not merely to an edge that exists in the graph.
3. The citation contains no text absent from that `Evidence`.
4. Every explanation item has at least one citation.

**Citations are scoped to one analysis.** `EdgeId` is graph-local and restarts
at zero for every analysis (ADR-0011), which `cartograph-desktop/src/evidence.rs`
already treats as a correctness hazard rather than untidiness: a stale edge id
"almost certainly *exists* in the new graph — as a completely different
relationship", so the lookup would succeed and show confident, wrong evidence.
ASK inherits that hazard the moment a citation outlives its analysis, and
therefore inherits the same defence: a citation carries the analysis it belongs
to, and a mismatch is refused rather than answered.

**Evidence text is not source code, and the distinction is load-bearing.**
`cartograph-core/src/evidence.rs` states that evidence "must never contain
source-file text, environment variable values or credentials (RULE 015). It
describes a claim; it does not quote the file." ASK sends evidence; ASK never
sends source files. Nothing in M16 redefines `Evidence` to mean source.

### F. Tracing-layer redaction is a prerequisite, not a follow-up.

`docs/security/README.md` schedules mechanical redaction — "not by convention" —
for "the first LSP/network code". That code is M16. SECURITY.md already states
as an invariant that "Redaction happens at the tracing layer", which is today
true by discipline rather than by construction: there is no `Layer`
implementation in the workspace, and the CLI installs a plain
`tracing_subscriber::fmt`.

**No AI or network request may be issued before redaction is active.** The
responsibility is architectural: redaction belongs at the tracing layer so that
it holds for code that has not been written yet, rather than at each call site,
where it holds only for the call sites someone remembered. The request itself,
its headers and any error derived from it must never reach a log carrying a
credential, source text, an environment value or an absolute path.

The implementation is not designed here. What is fixed is the ordering, the
placement, and that its tests assert absence mechanically rather than by review.

## The AI boundary, stated plainly

ADR-0007 is **immutable** and names this milestone: a language model "may
explain already-derived subgraph evidence (ASK, M16), where it interprets and
cites but cannot add", and ASK's design "must pass evidence *to* the model,
never accept structure *from* it."

The model may produce **explanation text and structured citations. Nothing
else.** It may not create a node, create an edge, modify graph structure, alter
`Evidence`, invent a citation, or have any output re-enter graph construction.
Model output is a separate artefact from architecture data at every layer that
carries it, and no code path admits it into the graph.

RAG, vector databases and LangChain are rejected for v1 (PROJECT_RULES,
mechanically scanned by QG-006). The evidence bundle is assembled by traversal,
not retrieved by similarity.

## Deferred, deliberately

Not settled by this ADR. An implementation may not treat them as settled.

| Deferred | Why it is still open |
|---|---|
| **C — AI provider and HTTP client** | **Superseded by Amendment 1**, and still open. The specification is available; §10 names no HTTP client and no AI provider SDK. Its only network-adjacent entries, `async-lsp` and `tokio`, are for LSP transport |
| **D — Keychain crate** | **Proposed by Amendment 2**, awaiting acceptance. §13 requires the OS keychain; §10 names no crate, so the choice is the repository's own. Slice 4 ships the boundary with no backend until it is accepted |
| **The configuration file's path** | Decided as a *kind* of thing (B); the platform-specific location needs platform evidence |
| **The redaction layer's implementation** | Placement and ordering are decided (F); the mechanism is its slice's work |
| **Whether MCP ASK ships in M16 or after the desktop surface** | A sequencing question the slice plan may answer with evidence |

## Amendment 1 — the frozen specification is available, and it names no crate for C or D

**Status: Accepted, 2026-09-05.** When this ADR was written, C and D were
deferred because *"the frozen engineering specification, which authorises
dependencies, is not in this repository."* The specification —
**Cartograph, Frozen Engineering Specification V3, August 2026** — is now
available and has been read. That sentence is superseded, and what replaces it
is narrower rather than broader.

### What the specification establishes

**§13 Security** states the requirement plainly:

> *Credentials live in the OS keychain, never in configuration files.*
> *AI is opt-in, per repository, off by default. The product is fully useful
> with no API key and no network.*
> *Never log source contents, tokens, or environment variables. Redact at the
> tracing layer.*

**§10 Tech stack** is the authorising list, and it is complete: thirteen crates
for the Rust core — `tree-sitter`, `async-lsp` + `lsp-types`, `petgraph`,
`redb`, `gix`, `notify`, `rayon`, `tokio`, `clap`, `thiserror`/`anyhow`,
`tracing`, `criterion`, `rmcp` — and eight choices for the desktop.

### The distinction that matters

§13 states a **requirement**. §10 is the list of **authorised crates**. They are
not the same thing, and for M16 they do not meet:

| | Status |
|---|---|
| **C — AI provider / HTTP client** | §10 names **no HTTP client** and **no AI provider SDK**. The only network-adjacent entries are `async-lsp` and `tokio`, both for LSP transport. **Still deferred.** |
| **D — keychain crate** | §13 requires the OS keychain; §10 names **no credential-storage crate**. **Still deferred.** |

So the blocker changed shape rather than lifting: it is no longer *"the document
is missing"* but *"the document is present and does not name one."* That is a
better place to be — it is now a question with a known answer procedure rather
than an unknown — but it is not authorisation, and this ADR does not treat it
as one.

The route is the repository's own: §10's freeze reads *"Frozen. Not reopened
until M9 ships"*, and M09 shipped long ago, so the list is reopenable; and
QG-006 already says *"new dependencies not named in the frozen stack need an ADR
reference in the PR."* Choosing a keychain crate is therefore a decision with
its own evidence — platform behaviour, maintenance, dependency footprint,
testability without a real user's credentials — and belongs in its own ADR, not
inside an implementation slice.

### What Slice 4 does instead

It ships the **credential boundary and no backend**:
`cartograph_desktop::credential` defines `CredentialStore` (`get` / `set` /
`delete`, with `PermissionDenied`, `Unavailable` and `Backend` distinguished
from absence) and a `NoCredentials` implementation that reports every
credential as absent.

That is not a placeholder for its own sake. With no backend, ASK degrades to raw
evidence — which is precisely the property §13 asks for, *"fully useful with no
API key and no network"*, and it is the behaviour that ships and is tested. A
keychain backend can be added behind the trait without any caller changing.

### Storage: why the opt-in table is not `redb`

§10 does name a storage crate — `redb`, *"Pure Rust, embedded, no C
dependency"* — so the choice was made deliberately rather than overlooked.
`redb` is the local **graph** store (§5's figure labels it *"redb — local
graph"*), and it is not a dependency of this workspace yet. Introducing an
embedded database to hold one boolean per repository would fail §10's own
dependency test: *"what concrete Cartograph problem does this solve right
now?"*

The opt-in table is instead a small JSON document written with `serde_json`,
already a workspace dependency, in an OS configuration directory resolved from
the platform's own environment variables. No configuration-format crate and no
directory crate is added; §10 names neither, and neither is needed.

### What is unchanged

Decisions A, B, E and F stand. C and D remain in *Deferred, deliberately*, with
their reason restated: the specification is available and does not name a crate
for either.
## Amendment 2 — the keychain implementation choice (D)

**Status: Proposed, 2026-09-05.** Amendment 1 established that §13 requires the
OS keychain and §10 names no crate for it. This proposes the crate. It is an
**implementation choice this repository makes**, and it is **not** claimed to be
part of Frozen Engineering Specification V3.

The distinction is the point of this amendment, so it is stated before anything
else:

| | |
|---|---|
| **Frozen requirement (§13)** | *"Credentials live in the OS keychain, never in configuration files."* Not negotiable, not chosen here. |
| **Implementation choice (this amendment)** | `keyring-core` plus a native platform store per target. Chosen here, on the evidence below, and reopenable like any other dependency. |

### Decision

**Depend on `keyring-core` for the credential interface, and on one native
platform store per target. Take no umbrella crate and no default features.**

| | |
|---|---|
| Interface | `keyring-core` `^1` — default features (none enabled) |
| Windows | `windows-native-keyring-store` `^1` — Windows Credential Manager |
| macOS | `apple-native-keyring-store` `^1` — macOS Keychain |
| Linux | a Secret Service store, **pure-Rust transport preferred** — see below |
| Tests | `keyring-core`'s `mock` store; **no test touches a real keychain** |
| Licence | MIT OR Apache-2.0, dual, matching the project's Apache-2.0 distribution |

### Why this satisfies the requirement

§13 asks for the *OS* keychain, and these are the OS keychains: the Windows
Credential Manager, the macOS Keychain, and on Linux the Secret Service that
GNOME Keyring and KWallet implement. Nothing is stored by Cartograph itself, so
the credential never reaches a file the product owns — which is the second half
of §13, *"never in configuration files"*, and is already enforced structurally
by `optin.rs` holding only a locator, an opaque identity and a flag.

### Why the stores are separate crates, and why that matters

The upstream README is explicit that credential stores live "all in separate
crates of their own", and that consumers "should take dependencies on the
keyring-core crate and the specific credential stores they want to use". It
cautions specifically against the `cli` feature, which "will bring with it a
host of credential stores (and other libraries) they don't need".

So the smallest correct shape is not the umbrella `keyring` crate with features
switched off — it is `keyring-core` plus exactly the store each target needs.
That is what §10's own dependency test asks for: *"what concrete Cartograph
problem does this solve right now?"* One store per platform solves it; a
bundle does not.

`keyring-core` `1.0.0` has a single feature, `sample`, **off by default** — and
`sample` is the one that pulls `chrono`, `dashmap`, `regex`, `ron`, `serde` and
`uuid`. Leaving it off keeps the interface dependency close to nothing. The
`mock` store is **not** behind that feature and is available regardless, which
is what makes the test story work without switching anything on.

### The Linux backend, decided on the dossier's own reasoning

Two Secret Service transports exist upstream: one over `libdbus` (a C library)
and one over `zbus` (pure Rust). §10 chooses pure Rust twice, and says why both
times — *"redb — Pure Rust, embedded, no C dependency"*, and *"gix — Pure Rust;
libgit2 breaks five-platform cross-compilation"*. Cartograph ships prebuilt
binaries for three desktop targets (§14), so the same reasoning applies
unchanged: **prefer the pure-Rust transport**.

A kernel `keyutils` store also exists and needs no session daemon, but its
credentials are session-scoped rather than durable, which is the wrong shape for
a key a user sets once. Recorded so the alternative is visible rather than
unconsidered.

### Why file-backed alternatives are rejected

`cryptex` supports "file-backed keyrings instead of the OS keyring". That is
precisely what §13 forbids, so it is rejected on the specification rather than
on preference. `keyring-search` searches credential stores and does not store,
so it does not answer the requirement. `uv-keyring` is a downstream fork with a
narrower platform set and no advantage here.

### Why a mock backend in tests, and why CI needs no keychain

A test that reached a real keychain would prompt a developer, fail in CI where
no session keyring exists, and — worst — could read or overwrite a real
credential belonging to the person running it. `keyring-core`'s `mock` store
removes all three. The existing `CredentialStore` trait in
`cartograph_desktop::credential` already lets a test substitute a store, and
Slice 4's suite does exactly that today with `NoCredentials` and two hand-written
mocks; nothing about that changes.

### Why the dependency is introduced now, and not earlier or later

Not earlier: §10's dependency test rejects "we might need it later", and until
M16 there was no credential to store. Not later: Slice 5 sends a request that
needs a key, and a credential path added alongside the code that first uses it
is a credential path that has already shipped once without review.

§10 reads *"Frozen. Not reopened until M9 ships."* M09 shipped, so the list is
reopenable by its own terms; and QG-006 already provides the mechanism —
*"new dependencies not named in the frozen stack need an ADR reference in the
PR."* This amendment is that reference.

### What this amendment does not do

- It **adds no dependency**. No manifest or lockfile changes with it; the crates
  named here arrive in the implementation PR that cites this amendment, and only
  if it is accepted.
- It does **not** settle **C** — the AI provider and HTTP client. §10 names
  neither, and that decision has its own evidence and its own amendment.
- It does **not** claim `keyring-core` is frozen-stack content. It is not.

### Consequences

- The desktop gains its first credential-storage dependency, and on Linux a
  Secret Service transport. The CI desktop job runs on Ubuntu and macOS, so the
  implementation PR must confirm that the Linux job builds without a session
  keyring present — tests use `mock`, so none should be needed, and that must be
  demonstrated rather than assumed.
- `NoCredentials` stays. A machine with no store, a locked store, or a build
  without a backend all continue to read as "no credential", which is what keeps
  §13's *"fully useful with no API key and no network"* true.
- Slice 4's boundary is unchanged: `get` / `set` / `delete`, with
  `PermissionDenied`, `Unavailable` and `Backend` distinguished from absence.
  A backend plugs in behind it without a caller changing.

## Security implications

Two invariants are widened by M16 and neither may be widened silently.

**The binary gains network egress.** Until M16 the Rust workspace makes no
outbound connection at all. RULE 013 — "fully useful with zero API keys and no
network" — survives only because the degraded mode is itself an acceptance
clause, and Slice 2 delivers it before any network code exists.

**The MCP server gains egress and a credential.** ADR-0020 placed the
authorization guard below the transport so that "MCP exposes only the
session-authorized repository", and that invariant is untouched by ASK. But the
server today makes zero outbound calls; an ASK tool gives a process that already
holds a repository grant the ability to talk to a third party. The MCP client is
itself usually an AI agent, so the surface is an AI calling an AI. This is
recorded as a consequence the owner should weigh, not as a hidden cost — and it
is the strongest argument for sequencing the desktop surface first.

The existing privacy floor that ASK must clear is the one ADR-0020 names:
`no_json_document_contains_an_absolute_machine_path`,
`a_rooted_drive_less_path_never_reaches_serialised_output`, and
`no_error_message_from_a_rooted_path_contains_its_parents`.

## Validation requirements

- Clause 1 is machine-checked by the four citation properties in **E**, and
  additionally inspected by hand on a real answer, per RULE 026.
- Clause 2 is machine-checked with **no key present and no network available**.
- No test may require a key, a network connection, or a live provider. A test
  suite that fails when a laptop is offline would make QG-004 and QG-009 report
  the weather.

## Alternatives

- **A CLI `ask` command in M16** — rejected: the specification does not ask for
  it and it drags schema `1.3` and golden-file compatibility into a milestone
  whose acceptance says nothing about the CLI.
- **`redb` for the opt-in flag** — rejected for M16: storage was promised at
  M09/M10 and did not arrive, and introducing an embedded database to store one
  boolean per repository is a large unplanned step. If durable graph storage is
  wanted, it deserves its own decision rather than arriving as ASK's side effect.
- **Free-text answers with citations checked by review** — rejected: an
  acceptance criterion that cannot be tested is not a gate (RULES 016–017).
- **Sending source files to the model for richer explanations** — rejected: it
  breaks "no source leaves the machine", which is the property that lets
  Cartograph near an employer's monorepo (ADR-0005).
- **Letting the model propose a missing edge when evidence is thin** — rejected
  by ADR-0007, immutably. Unknown stays unknown (RULE 009).

## Consequences

The degraded mode is built first and is useful on its own, which means the
milestone delivers value before a single dependency is added — and defers C and
D until the slice that genuinely needs them.

Explanations will sometimes be thinner than a model could produce unconstrained,
because every claim must be anchored to evidence that already exists. That is
the intended trade: an explanation that cannot cite is one a reader cannot
check, and Cartograph's claim is that its output is checkable.

Redaction arrives as infrastructure that later milestones inherit rather than
re-argue.

Two decisions remain open, and M16 cannot be completed without closing them.
