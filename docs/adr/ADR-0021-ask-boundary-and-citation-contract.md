# ADR-0021 — The ASK boundary: surfaces, citation contract, and the first network edge

**Status:** Accepted · 2026-09-05 · Governs M16 · Proposed with M16 Slice 1 · Enforces
[ADR-0007](ADR-0007-no-llm-graph-construction.md) and
[ADR-0005](ADR-0005-local-first-privacy.md) · Follows
[ADR-0020](ADR-0020-mcp-boundary-and-authorization.md)

> **Accepted 2026-09-05, after both open decisions were closed.** This ADR
> shipped as *Proposed* because two decisions in it were deliberately left
> open — **C** (AI provider and HTTP client) and **D** (keychain crate) —
> since the document that authorises dependencies was not in this repository.
> Both are now closed, so the ADR is Accepted.
>
> **Amended 2026-09-05 — three amendments.** ***Amendment 1***: the
> specification is available and has been read; it names no crate for C or D.
> ***Amendment 2*** closes **D** — `keyring-core` with one native store per
> target. ***Amendment 3*** closes **C** — Groq over its OpenAI-compatible
> HTTP endpoint, called through `ureq` with `rustls`, from a new
> `cartograph-ask` crate.
>
> **Amendments 2 and 3 record implementation choices this repository makes.
> Neither is claimed to be frozen-stack content, because neither is.**

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

Not settled when this ADR was written. **C and D are now closed** — by
Amendments 3 and 2 respectively, both accepted 2026-09-05. The rest of the
table stands, and an implementation may not treat those rows as settled.

| Deferred | Status |
|---|---|
| **C — AI provider and HTTP client** | **Closed by Amendment 3**, accepted 2026-09-05. §10 names no HTTP client and no AI provider SDK — its only network-adjacent entries, `async-lsp` and `tokio`, are for LSP transport — so the choice was the repository's own |
| **D — Keychain crate** | **Closed by Amendment 2**, accepted 2026-09-05. §13 requires the OS keychain; §10 names no crate, so the choice was the repository's own. Slice 4's boundary ships unchanged; a backend plugs in behind it |
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

**Status: Accepted, 2026-09-05.** Amendment 1 established that §13 requires the
OS keychain and §10 names no crate for it. This chooses the crate. It is an
**implementation choice this repository makes**, and it is **not** claimed to be
part of Frozen Engineering Specification V3.

Accepted by owner decision, with every crate claim below re-verified against
crates.io on the day of acceptance rather than carried forward from the draft.
What that check changed is recorded in *Verified at acceptance*; the decision
itself is unchanged.

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

### Verified at acceptance

Every crate named above was re-checked against crates.io on 2026-09-05. The
decision stands; three things it left implicit are now written down.

| Crate | Version | Features | Licence | MSRV | Platform |
|---|---|---|---|---|---|
| `keyring-core` | **1.0.0** | none — `sample` is off by default and stays off | MIT OR Apache-2.0 | **1.85** | all |
| `windows-native-keyring-store` | **1.1.0** | default | MIT OR Apache-2.0 | **1.88** | Windows Credential Manager |
| `apple-native-keyring-store` | **1.0.2** | default | MIT OR Apache-2.0 | **1.85** | macOS Keychain |
| `zbus-secret-service-keyring-store` | **1.0.1** | default | MIT OR Apache-2.0 | **1.88** | Linux Secret Service |

**The Linux crate is now named.** The draft said only *"a Secret Service store,
pure-Rust transport preferred"*. The store that satisfies that is
**`zbus-secret-service-keyring-store`**, and naming it is part of accepting the
decision rather than leaving the implementation to re-derive it.

The two alternatives are recorded so they are visibly rejected rather than
unconsidered:

| Linux alternative | Version | Why not |
|---|---|---|
| `dbus-secret-service-keyring-store` | 1.0.0 | Transports over `libdbus`, a C library. §10 chose pure Rust twice and said why both times — *"redb — Pure Rust, embedded, no C dependency"*, *"gix — Pure Rust; libgit2 breaks five-platform cross-compilation"*. Cartograph ships prebuilt binaries for three desktop targets (§14) |
| `linux-keyutils-keyring-store` | 1.0.0 | Needs no session daemon, but its credentials are session-scoped rather than durable. Wrong shape for a key a user sets once |

**An MSRV discrepancy, recorded rather than discovered later.** Two of these
stores declare `rust-version = 1.88`, and this workspace declares
`rust-version = "1.85"`. That gap is **not introduced here**: `rmcp` has
declared 1.88 since M15 and the workspace has built and passed its gates
throughout. The pinned toolchain is 1.97.1, so nothing fails to compile. What
is true is narrower and worth saying plainly: **the workspace's declared
`rust-version` understates its real floor, and has since M15.** Correcting it
is a one-line change that belongs to whoever next touches the manifest with a
reason to — not to this amendment, which adds no dependency and changes no
manifest.

### What this amendment does not do

- It **adds no dependency**. No manifest or lockfile changes with it; the crates
  named here arrive in the implementation PR that cites this amendment, which
  may now proceed.
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

Two decisions remained open when this was written, and M16 could not be
completed without closing them. Amendments 2 and 3 close them.

## Amendment 3 — the AI provider and the HTTP client (C)

**Status: Accepted, 2026-09-05.** Amendment 1 established that Frozen
Engineering Specification V3 §10 names **no HTTP client and no AI provider
SDK**, and left **C** deferred. This closes it.

Accepted by owner decision. Every external fact below was verified against the
provider's and the crates' current documentation on the day of acceptance, and
re-verified before it; what that check found is in *Verified at acceptance*.

Everything below is an **implementation choice this repository makes**. The
distinction Amendment 2 drew for the keychain is drawn again here, because it
is the part most easily lost:

| | |
|---|---|
| **Frozen requirement (§13)** | *AI is opt-in, per repository, off by default.* *The product is fully useful with no API key and no network.* *Credentials live in the OS keychain, never in configuration files.* *Never log source contents, tokens, or environment variables.* |
| **Frozen requirement (ADR-0007, immutable)** | The model interprets already-derived evidence and cites it. It never constructs, and its output never re-enters the graph. |
| **Frozen requirement (ADR-0005 / SECURITY.md)** | No source leaves the machine. |
| **Owner decision (this amendment)** | **Groq**, called over its OpenAI-compatible HTTP API, with **`openai/gpt-oss-120b`**. |
| **Owner decision (this amendment)** | **`ureq`**, blocking, with **`rustls`**. |
| **This repository's own choice (this amendment)** | A new `cartograph-ask` crate; a `Transport` seam; the wire schema; the validation shape. |

**Groq is not claimed to be named by Frozen Engineering Specification V3. It is
not.** §10 names no AI provider at all. **`ureq` is not claimed to be in the
frozen stack. It is not.** Neither is `rustls`. The route is the one §10 and
QG-006 already provide: §10 reads *"Frozen. Not reopened until M9 ships"* and
M09 shipped, and QG-006 says *"new dependencies not named in the frozen stack
need an ADR reference in the PR."* This amendment is that reference.

### Decision C1 — the provider is Groq, over direct HTTPS

| | |
|---|---|
| Provider | Groq |
| Base URL | `https://api.groq.com/openai/v1` |
| Endpoint | `POST /openai/v1/chat/completions` |
| Authentication | `Authorization: Bearer <key>` |
| Model | `openai/gpt-oss-120b` |
| Structured output | `response_format: {"type":"json_schema", ..., "strict": true}` |
| SDK | **None.** The request is a JSON document this repository builds. |

**No SDK, including Groq's own.** The request is four fields and the response
is one string to parse; an SDK would add a dependency tree, a second
serialisation path, and — decisively — a body this repository did not construct
byte for byte. The privacy test in **C6** checks *the exact bytes sent*, and it
can only do that if this repository produced them.

**Why the OpenAI-compatible endpoint rather than a Groq-native one.** It is the
documented surface (*"Groq API to be mostly compatible with OpenAI's client
libraries"*), and it keeps the `Provider` trait's shape provider-shaped rather
than Groq-shaped. Substituting a different provider later is then a new
implementation of one trait, not a rewrite of `cartograph-ask`.

**Verified against current documentation, 2026-09-05**, not from memory:

| Property | Verified value | Source |
|---|---|---|
| `openai/gpt-oss-120b` availability | Listed under **Production Models**, not preview | Groq model list |
| Context window | **131,072** tokens | Groq model list |
| Max completion tokens | **65,536** | Groq model list |
| Strict structured output | **Supported** (`strict: true` list) | Groq structured-outputs docs |
| Best-effort structured output | **Supported** | Groq structured-outputs docs |
| Streaming with structured output | **Not supported** — *"Streaming and tool use are not currently supported with Structured Outputs"* | Groq structured-outputs docs |
| Strict-mode schema constraints | Every field `required`; every object `additionalProperties: false` | Groq structured-outputs docs |
| Error body | `{"error": {"message": "...", "type": "invalid_request_error"}}` | Groq error docs |
| Rate limiting | HTTP **429**, with `retry-after` and `x-ratelimit-*` headers | Groq rate-limit docs |
| Transport | HTTPS only | base URL scheme |
| `temperature: 0` | Converted server-side to `1e-8` | Groq OpenAI-compatibility docs |

The last row is recorded because it is a claim this project must **not** make:
Groq does not accept a temperature of exactly zero, so **ASK answers are not
deterministic and this repository will not describe them as such.** Determinism
is a property of the analysis, not of the explanation over it. The bundle, the
scope and the citation check are deterministic; the sentences are not.

`gpt-oss-120b` is available, supported, has sufficient context for any bundle
this design permits (**C5** caps the request far below 131,072 tokens), and
supports the required structured response format. No substitution is proposed
and none is needed.

### Decision C2 — the HTTP client is `ureq` 3.4, blocking, over `rustls`

| | |
|---|---|
| Crate | `ureq` **3.4.0** |
| Features | **Defaults**, plus nothing. `json` is deliberately **off** — see below |
| What the defaults are | `gzip`, `rustls` (which enables `_ring`, `rustls-no-provider`, `rustls-webpki-roots`) |
| Licence | MIT OR Apache-2.0 |
| MSRV | **1.85** — exactly this workspace's `rust-version` |
| I/O model | **Blocking.** *"It uses blocking I/O instead of async I/O, because that keeps the API simple and keeps dependencies to a minimum."* |
| TLS | `rustls` **0.23.43**, Apache-2.0 OR ISC OR MIT |
| Crypto provider | `ring` **0.17**, Apache-2.0 AND ISC |
| Root store | `webpki-roots` — bundled Mozilla trust anchors |

**Why blocking, stated as architecture rather than taste.** The desktop and the
whole synchronous core path — `cartograph-core`, `-graph`, `-pipeline`,
`-desktop` — use no async Rust. Tokio exists in exactly one crate,
`cartograph-mcp`, because `rmcp` requires it. An async HTTP client would push a
runtime into the synchronous path to serve one request per user gesture. ASK is
a person pressing a button and waiting for a sentence; there is no concurrency
to win.

**What is claimed about `rustls`, precisely.** `rustls` is **not** described
here as literally pure Rust, because with its default provider it is not:
`ring` contains C and assembly. The accurate claim, and the only one this
amendment makes, is that **`rustls` avoids reliance on the system TLS library**
— no `schannel` on Windows, no Secure Transport on macOS, no OpenSSL on Linux.
That matters for the same reason §10 gave twice, for `redb` (*"no C
dependency"*) and `gix` (*"libgit2 breaks five-platform cross-compilation"*):
Cartograph ships prebuilt binaries for three desktop targets (§14), and one TLS
stack that behaves identically on all three is worth more than one that
inherits three different platform policies.

The cost is recorded rather than hidden: `rustls-webpki-roots` means the trust
anchors **ship with the binary** and are updated by a release rather than by
the operating system. `ureq`'s `platform-verifier` feature is the alternative,
and it is the thing being declined — deliberately, for the cross-platform
consistency above. If a release cadence ever makes bundled roots the wrong
trade, that is a decision to revisit here, with evidence.

**Why `json` is off.** `ureq`'s `json` feature would serialise the request
body inside the client. **C6** requires a test over the exact bytes sent, and a
test cannot check bytes it did not see. So `cartograph-ask` serialises with
`serde_json` — already a workspace dependency — inspects that `String`, and
sends *it*. One serialisation, one artefact, one thing to test.

**Configuration set explicitly, not by default:** `https_only(true)`, a global
timeout, and `max_redirects(0)` — a redirect on an authenticated request is a
credential-forwarding hazard, and there is no legitimate redirect on this
endpoint.

**Rejected alternatives**

| Alternative | Why not |
|---|---|
| `reqwest` | Async-first; brings Tokio into the desktop path, which is the thing being avoided. Its blocking mode is a runtime in a thread — the same cost, less visibly |
| `hyper` | A protocol implementation, not a client. TLS, redirects, timeouts and pooling would all be written here |
| Groq SDK / OpenAI SDK | See C1. A body this repository did not construct cannot be byte-checked |
| `native-tls` / system TLS | Three platform behaviours to support and cross-compile against, for one endpoint |
| `curl` via `Command` | Shelling out. Rejected outright: it puts a credential on a command line |

### Decision C3 — the boundary: `cartograph-ask`, and what crosses it

A **new crate, `cartograph-ask`**, holds the `Provider` trait, the Groq
implementation, the wire types, citation validation and the provider errors.

**The HTTP client is a dependency of this crate and of nothing else.** Not
`cartograph-pipeline`, not `-core`, not `-graph`, not `-cli`. This is the same
containment argument QG-006 already enforces for `petgraph`: capability that
lives in a shared crate is capability every client acquires. The CLI and the
MCP server must remain incapable of network egress **by their dependency
graph**, not by their authors' restraint.

```
core ──► graph ──► ask ─────────┐
                                ├──► desktop
core ──► graph ──► pipeline ────┴──► cli, mcp
```

`cartograph-ask` depends on `cartograph-core` and `cartograph-graph`. It does
**not** depend on `cartograph-pipeline`, and therefore cannot reach
`RepositoryIdentity`, `AuthorizedTree` or the analyser. It cannot name a
repository even if it wanted to.

**The trait is blocking and takes three things:**

```rust
pub trait Provider {
    fn explain(
        &self,
        bundle: &EvidenceBundle,
        question: &Question,
        credential: &Secret,
    ) -> Result<Answer, ProviderError>;
}
```

**`Secret` moves from `cartograph_desktop::credential` to `cartograph-core`,
re-exported from its old path so no caller changes.** It is needed below the
desktop now, and its redacting `Debug` is exactly the property that must travel
with it. This is the only refactor Slice 5 requires of merged code.

**It needs no ADR of its own**, and the reason is measurable rather than
asserted. `Secret` is a `String` newtype with **no dependencies at all** — its
only import is the module's, not its own — so nothing travels with it. Outside
its defining file it has exactly **one** caller in the workspace,
`crates/cartograph-desktop/tests/optin.rs`, and a `pub use` at the old path
keeps even that unchanged. No behaviour changes, no dependency is added, and no
boundary moves: the move *serves* the boundary this amendment already decided,
rather than proposing a new one. RULE 019 asks for an ADR for a significant
architecture change; relocating a dependency-free type one layer down, behind a
re-export, is an ownership correction.

**`CredentialStore`, `CredentialError` and `NoCredentials` stay in the
desktop.** They cannot follow `Secret` down: `CredentialStore::get` takes
`&RepositoryIdentity`, which lives in `cartograph-pipeline`, so moving the
trait into `cartograph-core` would make core depend on another Cartograph crate
— the one thing QG-006 checks by name. The split is therefore not a compromise
but the only shape that holds: the *value* goes down to where both the desktop
and `cartograph-ask` can see it; the *store*, which needs a repository
identity, stays up with the client that has one.

**The secret type is not weakened by the move.** No `Display`, no `Serialize`,
no owned accessor, and a `Debug` that prints `Secret(<redacted>)`. `expose()`
remains the only way out and still yields a borrow.

| The provider receives | The provider is structurally denied |
|---|---|
| the user's question | source files — nothing in a bundle is source (`Evidence` may not quote a file) |
| the `EvidenceBundle` — already-derived claims | the `ArchitectureGraph` — the trait takes a bundle, and a bundle is a copy |
| the credential, borrowed for the call | `RepositoryIdentity` — not in the crate's dependency graph |
| | absolute paths — `SourceLocation` refuses them at construction |
| | environment variable values — `NodeKind::EnvVar` has no field for one |
| | any other secret |

**What the model may produce: explanation text and structured citations.
Nothing else.** It cannot construct or modify graph structure, alter
`Evidence`, authorize or select a repository, or invent a citation, because
nothing in `cartograph-ask` can do those things — there is no graph to mutate
and no authorization to reach. ADR-0007 is enforced by the dependency graph,
not by a prompt.

### Decision C4 — one honest exception, stated rather than glossed

The instruction for M16 is that no filesystem path is sent. **Repository-relative
paths are sent, and cannot not be**, so it is written down here rather than
discovered later.

`NodeIdentity` is `kind|name|file` (`cartograph_core::identity_of`), and
`BundledEdge` carries `location`. Both take the file from `SourceLocation`,
which **refuses absolute paths and `..` traversal at construction**. Further,
evidence text itself frequently names a file — and evidence must be transmitted
byte for byte or a verbatim citation cannot be checked against it.

So the rule is stated at the precision the code can actually hold:

> **No absolute path, and nothing identifying this machine, ever leaves it.
> Repository-relative paths do, because they are part of the derived evidence
> the answer must cite.**

The frozen requirement — *no source leaves the machine* — is untouched: a path
is not source. `src/api/client.ts` is sent; its contents never are.

This is flagged for the owner explicitly. If repository-relative paths must
also be withheld, the mechanism is per-request opaque aliases (`artefact 3`)
mapped back locally — which costs explanation quality and cannot redact paths
that occur inside evidence text without breaking verbatim citation. That
trade-off is not taken unilaterally.

### Decision C5 — the request

One system message stating the contract, one user message carrying the question
and the evidence. Nothing else.

```json
{
  "model": "openai/gpt-oss-120b",
  "messages": [
    { "role": "system", "content": "<fixed contract text>" },
    { "role": "user",   "content": "<question>\n\n<rendered evidence>" }
  ],
  "temperature": 0,
  "max_completion_tokens": 2048,
  "stream": false,
  "response_format": {
    "type": "json_schema",
    "json_schema": {
      "name": "cartograph_answer",
      "strict": true,
      "schema": { "...": "the shape in C6" }
    }
  }
}
```

The system message is a **compile-time constant**, not built from user input.
It states: every claim must cite; a citation quotes an edge's evidence exactly;
do not invent an edge; do not describe anything not in the evidence; unknown
stays unknown (RULE 009).

Each bundled edge renders as its `edge`, `from`, `to`, `kind`, `evidence`,
`provenance`, `confidence` and `location`. **Nothing else.** No repository
name, no analysis timestamp, no tool version, no counts, no identity.

**Size.** The bundle is capped by *serialised bytes* before the request is
built, truncating in the bundle's own deterministic order and reporting
`truncated` in the `Answer` so the panel can say so — the treatment
`cartograph_pipeline::map` already gives `Truncation`. Silent truncation is
forbidden: an answer that cites less than the user believes it saw is the same
failure as an uncited claim. The cap sits far below the model's 131,072-token
context, so context is never the binding constraint.

`stream: false` is required, not chosen: Groq does not support streaming with
structured outputs.

### Decision C6 — the response, and validation as the gate

The model must return exactly:

```json
{
  "items": [
    { "text": "...", "citations": [ { "edge": "...", "text": "..." } ] }
  ]
}
```

`strict: true` requires every field `required` and every object
`additionalProperties: false`. The schema is written that way.

**Schema compliance is not trusted.** It constrains shape, never truth: a model
can emit a perfectly-shaped citation quoting text no edge contains. So every
citation is re-checked locally, against the bundle that was actually sent,
by machinery that already exists and already ships —
`EvidenceBundle::cite` and `EvidenceBundle::verify` (Slice 1):

| Check | Enforced by | Failure |
|---|---|---|
| citation belongs to the bundle that was sent | `BundleScope` equality | `CitationError::ForeignBundle` |
| the cited edge is in that bundle | membership | `CitationError::UnknownEdge` |
| the quoted text is a byte-for-byte substring of that edge's `Evidence` | substring | `CitationError::NotVerbatim` |
| the quotation is non-empty | length | `CitationError::Empty` |
| every item has at least one citation | `cartograph-ask` | `ProviderError::UncitedItem` |
| every item has non-empty text | `cartograph-ask` | `ProviderError::EmptyItem` |

**Any failure rejects the entire answer.** Not the item — the answer. A partially
valid answer is the most dangerous output this feature can produce, because it
looks checked. `Answer` has private fields and no public constructor: **the only
way to obtain one is to pass validation**, so "render an unvalidated answer" is
not an expressible program.

On rejection ASK falls back to the degraded raw-evidence path of Slice 2 and
says the explanation was refused. The user is never worse off than with AI
switched off.

### Decision C7 — the credential

```
ASK request
  -> opt-in confirmed for this repository   (cartograph_desktop::optin)
  -> CredentialStore::get(&RepositoryIdentity) -> Option<Secret>
  -> Secret::expose() at the moment the header is written
  -> request sent
  -> the borrow ends
```

`RepositoryIdentity` is used **only** to look the credential up, in the
desktop. It is never passed to `cartograph-ask` and never appears in a request.

The key is **never** placed in: the `Provider` struct, the `Answer`, the
`EvidenceBundle`, any frontend or IPC payload, configuration, a log, an error,
or an MCP response. Structurally: `Secret` keeps its redacting `Debug`;
`ProviderError` variants carry status codes and fixed strings and have no field
that could hold one; the provider is constructed without a credential and
receives it per call as a borrow.

### Decision C8 — logging: provider bodies are never logged

Not redacted. **Never logged.** Redaction is defence in depth for values that
reach a log by accident; a request body is not an accident, and a rule that
depends on a redactor recognising evidence text would fail the first time
evidence looked ordinary.

Made structural, four ways:

1. `GroqRequest` and `GroqResponse` have hand-written `Debug` that print
   neither body nor header — the treatment `Secret` and `RepositoryIdentity`
   already receive.
2. `ProviderError` carries an HTTP status, a Groq error `type`, and fixed
   strings. It never carries a body, a header, a URL with a query, or a
   credential.
3. A test asserts that `format!("{err:?}")` and `format!("{err}")` over every
   error variant contain neither the fixture key nor the evidence text.
4. A source-level check over `crates/cartograph-ask/src`: no `tracing` macro
   call may name a body, header, payload or credential binding.

`cartograph_pipeline::redaction::Redacting` (Slice 3) sits underneath all of
this and is a prerequisite, per **F** — no provider request may be issued
before it is installed.

**MCP's `eprintln!` diagnostics are resolved by C9, not by redaction.** They
print a grant error, a tree count and transport states. They cannot print a
credential because — by C9 — the MCP server never receives one. The isolation is
the dependency graph: `cartograph-mcp` does not depend on `cartograph-ask`.

Every `println!` in `cartograph-pipeline::incremental` was confirmed to sit
inside `#[cfg(test)]`; no production path in the analyser prints.

### Decision C9 — MCP stays evidence-only in M16

`cartograph-mcp` gains **no provider, no credential and no egress.** Its four
tools, its authorization guard and its stdio protocol are untouched.

ADR-0021 already recorded the reason as a consequence to weigh: an MCP client
*is* a model, so an ASK tool there would be an AI calling an AI — a second
credential path and a second egress path, added to a process that already holds
a repository grant, to serve a client that can already explain what it is
given. Cartograph's job there is to hand it checked evidence.

The MCP `ask` surface described in decision **A** therefore lands, if at all,
after the desktop surface and with its own evidence. Decision **A** is not
revoked; its second half is deferred past M16.

### Decision C10 — degraded mode is a permanent regression test

| Condition | Behaviour | Network |
|---|---|---|
| No opt-in | raw evidence | **none** |
| Opt-in, no credential | raw evidence | **none** |
| Opt-in, credential store unavailable or denied | raw evidence | **none** |
| Opt-in alone, no explicit ASK-with-AI request | raw evidence | **none** |
| Opt-in **and** credential **and** an ASK request | provider call | **one** |
| Provider fails, times out, or returns an invalid answer | raw evidence, plus "the explanation was refused" | attempted |

Proved rather than asserted: every degraded-path test is wired with a
`Transport` implementation that **panics if it is called**. "No network when AI
is disabled" then fails as a test failure rather than as a review oversight.

### Testing (no live call in CI, ever)

The seam is a `Transport` trait —
`post_json(url, headers, body) -> Result<HttpResponse, TransportError>` —
with `UreqTransport` in production and `MockTransport` in tests. It exists so
the Groq provider's *request construction* is testable without a network, which
is the part that carries the privacy and credential guarantees.

Request: correct endpoint · `Authorization: Bearer <key>` header shape, asserted
against a **runtime-generated fixture key** so no literal secret is ever a
tracked file (QG-005) · `response_format` present and strict · system message is
the constant · body contains no source, no absolute path, no
`RepositoryIdentity`, no credential.

Response: success · malformed JSON · valid JSON of the wrong shape · missing
`items` · item with no citation · item with empty text · citation naming a
foreign bundle · citation naming an edge outside the bundle · non-verbatim
citation · empty citation · multiple citations on one item · valid multi-item
answer.

Transport: timeout · connection failure · 400 · 401 · 413 · 422 · 429 with
`retry-after` · 500 · 502 · 503 · a body that is not JSON at all.

Credential and mode: credential missing · store unavailable · store denies
permission · AI disabled · no opt-in · **no network when AI is disabled**
(panicking transport) · credential never appears in any log, error or `Debug`.

**No test requires a key, a network, or a live provider.** A suite that fails on
an aeroplane would make QG-004 and QG-009 report the weather.

### Real validation, done once, locally

Per RULE 026 the citation contract is also inspected by hand on a real answer.
That run reads the key **only from the OS keychain** — never pasted into
source, never written to disk, never logged, never committed — against an
authorized repository with opt-in enabled, and confirms a real Groq answer,
real citations that validate, and a controlled invalid citation that is
rejected. No source code is sent.

### Verified at acceptance

Re-checked on 2026-09-05, against the provider's own documentation and
crates.io, before acceptance rather than after.

**The model.** `openai/gpt-oss-120b` is current, production, and **not
deprecated**. It appears on Groq's deprecation page only as the **recommended
replacement** for models that were retired — among them `llama-3.3-70b-versatile`,
`qwen/qwen3-32b`, `meta-llama/llama-4-scout-17b-16e-instruct` and
`moonshotai/kimi-k2-instruct-0905`. Being the destination of other models'
deprecations is a better longevity signal than mere presence on a list, and it
is the reason no fallback model is specified here.

**The client.** `ureq` **3.4.0**, MIT OR Apache-2.0, blocking, MSRV **1.85**.
Defaults are `gzip` and `rustls`; `rustls` itself enables `_ring`,
`rustls-no-provider` and `rustls-webpki-roots`. `json` is **not** default and is
**not** enabled — see C2. `native-tls`, `platform-verifier`, `brotli` and
`socks-proxy` are not default and are not enabled. **No async runtime is
reachable through this dependency**, which is the property that keeps Tokio out
of the desktop path.

**The TLS stack.** `rustls` **0.23.43**, Apache-2.0 OR ISC OR MIT, MSRV 1.71.
Its default provider is `ring` **0.17**, Apache-2.0 AND ISC, which **contains C
and assembly**. That is recorded here, not glossed, and it is why this
amendment says `rustls` *avoids reliance on the system TLS library* rather than
that it is pure Rust.

**MSRV.** `ureq`'s 1.85 matches this workspace's declared `rust-version`
exactly, so **C adds no MSRV pressure**. The gap noted in Amendment 2 — two
keychain stores declaring 1.88 — is unrelated to this amendment and predates
both, since `rmcp` has declared 1.88 since M15.

### Dependency matrix (QG-006)

QG-006 requires *"new dependencies not named in the frozen stack need an ADR
reference in the PR."* This is that reference; the table is the review.

| Crate | Version | Features | Licence | Platforms | Why, concretely | Alternative rejected |
|---|---|---|---|---|---|---|
| `ureq` | 3.4.0 | defaults (`gzip`, `rustls`); **`json` off** | MIT OR Apache-2.0 | Windows, macOS, Linux — all three §14 targets | The one HTTPS POST ASK makes. Blocking, so no runtime enters the desktop path. MSRV 1.85 equals this workspace's | `reqwest` (async, so Tokio in desktop); `hyper` (not a client); shelling to `curl` (credential on a command line) |
| `rustls` | 0.23.43 | via `ureq`'s `rustls` | Apache-2.0 OR ISC OR MIT | same | One TLS behaviour across three prebuilt targets; no system TLS library. **Not claimed to be literally pure Rust** | `native-tls` (three platform behaviours, cross-compilation cost) |
| `ring` | 0.17 | via `rustls`'s default provider | Apache-2.0 AND ISC | same | `rustls`'s default crypto provider. **Contains C and assembly** — recorded, not hidden | `aws-lc-rs` (more C, larger build); a pure-Rust provider (less scrutinised, slower) |
| `webpki-roots` | via `ureq` default | — | MPL-2.0 (Mozilla trust anchors) | same | Bundled trust anchors, identical on every target | `platform-verifier` — declined for cross-platform consistency; cost recorded above |

Direct additions: **`ureq` only.** `rustls`, `ring` and `webpki-roots` arrive
through it and are listed because a review of "one dependency" that is really
four is not a review. The implementation PR must attach
`cargo tree -p cartograph-ask` so the full transitive set is on the record
rather than summarised here.

No prohibited-for-v1 dependency is added: QG-006's list — neo4j, sqlx, diesel,
postgres, redis, aws-sdk, rusoto, kafka, rdkafka, kube, **langchain**, qdrant,
milvus, pinecone — is untouched, and no RAG, vector store or agent framework
appears. QG-008's future-milestone list (`async-lsp`, `lsp-types`, `redb`,
`gix`, `notify`, `regex`, `url`, `salsa`) is untouched. `petgraph` containment
and the dependency direction are unaffected: `cartograph-ask` sits beside
`cartograph-pipeline`, and nothing depends on `cartograph-cli`.

### Consequences

- The Rust workspace makes its **first outbound network connection**, in exactly
  one crate, reachable from exactly one client, on exactly one condition.
- The desktop gains a dependency on `cartograph-ask`. The CLI and MCP server
  gain nothing and remain incapable of egress by construction.
- `Secret` moves to `cartograph-core` and is re-exported. One refactor, no
  caller changes.
- ASK answers are **not deterministic**, and this project will not claim they
  are. Determinism belongs to the analysis; the explanation is checked, not
  reproduced.
- Bundled trust anchors are updated by a Cartograph release rather than by the
  operating system.
- Groq is a third party outside this machine. What reaches it is a question and
  derived evidence, on explicit per-repository opt-in, with a key the user
  supplied. That is the trade the opt-in exists to let a user make knowingly.

### What this amendment does not do

- It **adds no dependency**. No manifest or lockfile changes with it. `ureq`
  arrives in the Slice 5 PR that cites this amendment, which may now proceed.
- It does **not** claim Groq, `ureq`, `rustls` or `ring` are frozen-stack
  content. None of them are.
- It does **not** re-open **D**. Amendment 2 closed the keychain choice and is
  accepted; nothing here changes it.
- It does **not** widen MCP. `cartograph-mcp` gains no provider, no credential
  and no egress, and its tools are untouched.
- It does **not** accept M16, move the ledger, advance the version, or create a
  tag. M16 remains `next_allowed_milestone`, and `cartograph-m16` does not
  exist.

### Status of ADR-0021 after this amendment

**C** is closed here; **D** was closed by Amendment 2. Both are accepted, so
**ADR-0021 itself moves from Proposed to Accepted** and its opening note now
records how they were closed rather than that they were open.

Nothing in decisions **A**, **B**, **E** or **F** changes, with one sequencing
exception stated in C9: **A**'s second surface, an MCP `ask` tool, is deferred
past M16. It is not revoked.

M16 Slice 5 may now begin, in the order its plan sets out. It may not begin
before it: a dependency added ahead of the decision that authorises it is a
dependency that shipped without review.
