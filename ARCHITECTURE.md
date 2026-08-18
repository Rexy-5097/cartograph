# Cartograph — Architecture

> Frozen Engineering Specification V3. Changes to anything on this page require
> an [ADR](docs/adr/).

## One core, three clients

```
        ┌──────────┐   ┌──────────┐   ┌──────────────┐
        │   CLI    │   │ Desktop  │   │  MCP server  │
        │  (M09)   │   │  (M11)   │   │    (M15)     │
        └────┬─────┘   └────┬─────┘   └──────┬───────┘
             └──────────────┼────────────────┘
                            ▼
        ┌───────────────────────────────────────────┐
        │                RUST CORE                  │
        │                                           │
        │   tree-sitter    LSP client    gix        │
        │        └──────────────┬──────────┘        │
        │              Semantic model               │
        │              Graph engine                 │
        │        query · diff · analysis            │
        └───────────────────┬───────────────────────┘
                            ▼
                  redb — local graph store
        ───────────────────────────────────────────
             NO SOURCE LEAVES THE MACHINE
```

The Rust core is the product (RULE 001). The CLI, the desktop application and
the MCP server are clients of one graph API and hold no analysis logic of their
own (RULE 002). That single decision governs every other choice here: it is why
layout is computed in Rust, why the domain model lives in `cartograph-core`
rather than in the CLI, and why adding a feature to one client never means
adding it three times.

## Crates

| Crate | Owns | State |
|---|---|---|
| `cartograph-core` | Domain model: nodes, edges, evidence, provenance, confidence | Implemented |
| `cartograph-graph` | The architecture graph, traversal | Implemented |
| `cartograph-parser` | tree-sitter extraction, per language | TypeScript/TSX (M01) and Python (M02) implemented |
| `cartograph-resolver` | Canonical routes, cross-language matching, edge construction | M03–M04 implemented; ORM at M06 |
| `cartograph-cli` | The `cartograph` binary | `version`, `parse` |
| `cartograph-testkit` | Fixtures and builders. Never a runtime dependency | Implemented |

### Dependency direction

```
cli ──► core, parser, resolver
graph ──► core ──► (nothing Cartograph-owned)
parser ──► core
resolver ──► parser, core, graph
testkit ──► core
```

`cartograph-core` depends on no other Cartograph crate. Nothing depends on
`cartograph-cli`. This is checked by
[QG-006](agentos/gates/QG-006-architecture-compliance.md).

## The domain graph

Cartograph owns its graph model. `petgraph` provides algorithms and sits
underneath; it never appears in a public API (RULE 003,
[ADR-0003](docs/adr/ADR-0003-cartograph-owns-the-graph.md)). `StableDiGraph`
specifically, because the incremental engine (M10) removes and re-adds
subgraphs and invalidated indices would corrupt every stored reference.

### Node kinds

`Repository` · `Package` · `Directory` · `File` · `Module` · `Class` ·
`Function` · `Method` · `Variable` · `Route` · `Table` · `Column` ·
`ExternalService` · `EnvVar`

Three domains deliberately: code, the HTTP boundary, and data. The
differentiating output is a path that crosses all three.

### Edge kinds

`Import` · `Call` · `Inherits` · `Implements` · `References` · `HttpCall` ·
`OrmAccess` · `Queries`

The last three are the cross-stack kinds — the reason the resolver exists.

### The edge record

Every edge carries source, target, kind, confidence, provenance, evidence,
source location, commit and creation time. This is enforced by the type system:
`Edge` has no `Default`, no public fields, and one constructor that requires all
of them. An edge that cannot answer *"show me exactly why you think these two
things are connected"* does not compile.

See [docs/graph/](docs/graph/).

## Two-tier analysis

```
tree-sitter  →  syntactic facts   ("what does this file say?")
     ↓
LSP client   →  semantic facts    ("what does this name refer to?")
```

tree-sitter is incremental and error-tolerant, so a file mid-edit still yields
useful facts. It cannot resolve types. tsserver and Pyright already do that work
correctly, and reimplementing either is not a good use of this project's time.
See [ADR-0002](docs/adr/ADR-0002-two-tier-analysis.md).

## The resolver

The single subsystem that justifies the project.

```
   FRONTEND SIDE              BACKEND SIDE
   AST extraction             Route extraction
   HTTP call sites            Normalization
   String analysis                  │
        └────────────► JOIN ◄───────┘
                  Route matching
                Confidence scoring
                 ORM resolution
                        ▼
                   Graph edge
```

Frontend and backend are analysed independently, then joined on a canonical
route form. Where an OpenAPI document exists it is ground truth and inference is
skipped.

Normalisation (M03) canonicalises each observation on its own; matching (M04)
is the first stage permitted to relate two of them, and the only one permitted
to create an `HttpCall` edge. See
[docs/resolver/canonical-routes.md](docs/resolver/canonical-routes.md) and
[docs/resolver/matching.md](docs/resolver/matching.md).

See [docs/resolver/](docs/resolver/).

## Confidence

Confidence is a first-class field, not a decoration. The current values are
**uncalibrated starting priors**:

| Analysis | Prior | Kind |
|---|---|---|
| LSP exact symbol resolution | 1.00 | derived |
| Route + method exact match | 0.98 | derived |
| OpenAPI confirmed | 0.98 | derived |
| Static import resolution | 0.90 | derived |
| Constant propagation | 0.80 | inferred |
| Partial template match | 0.65 | inferred |
| Model inference | 0.45 | inferred |

An edge marked 0.8 is *not yet* known to be correct 80% of the time.
Establishing that is M08, which fits these against a labelled benchmark and
publishes a reliability diagram and expected calibration error. Until then no
accuracy claim may rest on these numbers.

Whether an edge is *derived* or *inferred* is decided by its provenance — the
method — not by a threshold on the number.

## Storage

`redb`: pure Rust, embedded, no C dependency, no server. The graph is local
(RULE 011). Storage lands at M09/M10; M00 holds the graph in memory only.

## Privacy architecture

These are invariants from v1, not features to add later. They are the property
that lets someone run Cartograph against their employer's monorepo.

- No source leaves the machine. Default and unconditional.
- AI is opt-in, per repository, off by default.
- Credentials in the OS keychain, never in configuration.
- Redaction at the tracing layer: no source contents, tokens or environment
  variable values in logs.
- MCP exposes only the repository authorised in the current session.
- Temporary files cleaned on exit.

See [ADR-0005](docs/adr/ADR-0005-local-first-privacy.md) and
[docs/security/](docs/security/).

## Deliberately deferred

| Thing | Until |
|---|---|
| `salsa` | M10 — build invalidation by hand first, so the model is understood before a framework hides it |
| `wgpu` / custom renderer | Sigma.js is measurably the bottleneck (no rendering problem below 50k visible nodes) |
| Leiden clustering | Directory structure and package boundaries are the v1 clustering prior, and are usually correct |
| Content-addressed node identity | M10 |
| Axum, PostgreSQL, Redis, object storage | A user asks for a cloud component and offers to pay |
