# ADR-0019 — The trace traversal belongs to the graph, not to a client

**Status:** Accepted · 2026-09-03 · Implements [ADR-0001](ADR-0001-rust-core-is-the-product.md) ·
Follows [ADR-0016](ADR-0016-desktop-boundary.md) · Proposed with M15 Slice 1

## Decisions recorded on acceptance

1. **The trace traversal and the symbol lookup move to
   `cartograph-graph::trace`.** They are analysis, and analysis does not live
   in a client.
2. **`cartograph-cli`'s `trace_cmd` becomes presentation only** — the JSON
   document, the indented text, the labels, and the wording of a failure.
3. **The graph API returns identifiers**, not rendered structures: `NodeId`,
   `EdgeId`, a stop reason, and a depth. A caller reads names, confidence,
   provenance and evidence from the graph it already holds.
4. **`refusals_near` stays in the CLI**, because a refusal is a *resolver*
   fact, not a graph fact.
5. **This slice adds no dependency and no MCP surface.** `rmcp`, `tokio`,
   transport, repository authorization and the meaning of "map" are all
   deferred, and this ADR does not decide any of them.

## Context

### The fact that forced this

M15's scope is an MCP server "exposing graph queries (map/trace/blast/diff)".
Two of those four were already reachable by any client:

| Query | Where the analysis lives |
|---|---|
| `blast` | `cartograph-graph::blast` (M12) |
| `diff` | `cartograph-graph::diff` (M13) |
| `trace` | **`cartograph-cli::trace_cmd`** |
| "map" | **`cartograph-cli::summary_cmd`** (and see *Deferred*) |

`cartograph-cli` is a **binary** crate. It has no library target — `cargo test
-p cartograph-cli --lib` answers *"no library targets found in package
`cartograph-cli`"* — so nothing can depend on it. `edges_from` had exactly two
callers in the workspace: its own definition in `cartograph-graph`, and
`trace_cmd.rs`.

This is the same discovery ADR-0016 made at M11, in the same words:

> A binary cannot be depended on. So the second client had two options, and
> both were wrong: duplicate the sequence, or reach into the first client.

There it was the analysis *sequence*, and `cartograph-pipeline` was the answer.
Here it is the traversal, and this is the answer.

### Why the traversal is analysis and not presentation

It decides which relationships exist on a path, in what order they are
reported, when a chain has genuinely ended, when it has been cut short, and
when a cycle has closed. Every one of those is a claim about the architecture.
RULE 002 says a client holds none.

The alternative — letting the MCP server walk the graph itself — produces two
traversals that can disagree, and the disagreement would be invisible: both
would look correct in isolation, and a user comparing `cartograph trace` with
an MCP answer would have no way to tell which was lying.

## Decision

### 1. `cartograph-graph::trace`

```rust
pub const DEFAULT_MAX_DEPTH: usize = 10;

pub enum Stop { ChainEnd, MaxDepth, Cycle }

pub struct TraceStep { pub edge: EdgeId, pub to: NodeId,
                       pub next: Vec<TraceStep>, pub stop: Option<Stop> }

pub struct Trace { pub from: NodeId, pub steps: Vec<TraceStep>,
                   pub stop: Option<Stop>, pub max_depth: usize }

pub enum SymbolError { NotFound { near: Vec<NodeId> },
                       Ambiguous { matches: Vec<NodeId> } }

pub fn trace(graph: &ArchitectureGraph, from: NodeId, max_depth: usize) -> Trace;
pub fn resolve_symbol(graph: &ArchitectureGraph, query: &str) -> Result<NodeId, SymbolError>;
pub fn near_misses(graph: &ArchitectureGraph, name: &str) -> Vec<NodeId>;
pub fn split_query(query: &str) -> (Option<&str>, &str);
pub fn path_matches(file: &str, hint: &str) -> bool;
```

Behaviour is **carried over unchanged**, including the parts that are easy to
lose in a move:

- the two checks in the walk are in their original order, so a leaf reached
  exactly at the depth limit reports `ChainEnd` and not `MaxDepth`;
- branch order is `(kind, target id, edge id)`, independent of insertion order;
- the visited set is scoped to the *current path*, so a cycle terminates while
  a node reachable by two branches is still reported on both — two routes to
  one artefact are two facts;
- ambiguity is refused with every candidate rather than resolved by choosing
  (RULE 009), and near-misses are case-insensitive equality or substring, never
  edit distance, because suggesting `Order` for `Odrer` is guessing (RULE 010).

### 2. Identifiers, not structures

`TraceStep` carries an `EdgeId` and a `NodeId`. It does not carry the edge's
confidence, provenance, evidence or location, because the caller has the graph
those came from and a second copy is a second thing that can be stale.

`SymbolError` carries `NodeId`s for the same reason. How a candidate is
*described* to a person differs by client — the CLI prints
`name  (kind)  file:line` — so the graph does not describe anything.

### 3. `Stop` is serialisable; its sentences are not

`Stop` derives `Serialize` with kebab-case slugs, exactly as
`cartograph_core::NodeKind` does, because those slugs are already in the CLI's
JSON schema and must not change. The English sentence for each stop
(*"stopped at --max-depth — the chain may continue"*) stays in the CLI: it
names a flag that only the CLI has.

### 4. Refusals stay in the CLI

`refusals_near` reads `Analysis::results` and `cartograph_resolver::MatchStatus`
to report observations the resolver declined near a break. Moving it would make
`cartograph-graph` depend on `cartograph-resolver`, inverting the direction
QG-006 checks and the layering ADR-0016 established. It stays where the data
is.

The consequence is stated rather than hidden: **a non-CLI client calling
`trace::trace` gets the chain without the refusals**. A future slice that wants
them for MCP should lift them into `cartograph-pipeline`, which already depends
on the resolver — not into the graph.

## Consequences

Existing CLI output is unchanged, and that is the acceptance evidence for this
slice rather than an aspiration: the goldens were not edited, and traces of two
real repositories are byte-identical before and after once the wall-clock
duration — which varies between two runs of the *same* binary — is normalised.

`cartograph-graph` gains 19 tests it did not have, covering behaviour that
previously had only the CLI's end-to-end coverage.

## Deferred, deliberately

This ADR records an extraction. It decides none of the following, and a later
slice must not read agreement into its silence:

| Deferred | Why it is not decided here |
|---|---|
| `rmcp`, `tokio`, any async | Slice 1 adds no dependency. The frozen stack names `rmcp` at M15 and `tokio` at M04; neither is in the workspace, and neither is needed to move a traversal. |
| MCP transport (stdio or otherwise) | The M15 spec names Claude Code and Cursor but no transport. RULE 013 points at stdio; that is an inference, not a decision. |
| Session-scoped repository authorization | ADR-0005 and SECURITY.md state the *invariant* — "MCP exposes only the repository authorised in the current session" — and no repository evidence states the mechanism. |
| What "map" denotes as a query | There is no `cartograph map`. The candidates are the CLI's summary and the desktop's `Scene`, and choosing between them is a decision, not a reading. `summary_cmd` therefore stays in the CLI for now. |
| A QG for MCP correctness | `agentos/gates/README.md` names one among gates "later milestones add"; no such gate file exists. |

## Alternatives

- **Let the MCP server walk the graph itself** — rejected: two traversals that
  can disagree, and RULE 002 forbids a client holding analysis.
- **Depend on `cartograph-cli`** — impossible; it is a binary.
- **Move `refusals_near` too** — rejected: it would invert the dependency
  direction to reach resolver types.
- **Move the whole of `trace_cmd`** — rejected: the JSON envelope, the indented
  text and the flag names are the CLI's contract, not the graph's.
- **Return rendered structures from the graph** — rejected: it duplicates facts
  the caller can already read, and bakes one client's vocabulary into the core.
