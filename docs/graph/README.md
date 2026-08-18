# The domain graph

Implemented in [`cartograph-core`](../../crates/cartograph-core/) and
[`cartograph-graph`](../../crates/cartograph-graph/). Summary in
[ARCHITECTURE.md](../../ARCHITECTURE.md#the-domain-graph).

## The edge record

Never `A → B`; always the claim and how it was known:

| Field | Type | Notes |
|---|---|---|
| `source`, `target` | `NodeId` | Graph-local handles until M10 |
| `kind` | `EdgeKind` | 8 kinds; `HttpCall`/`OrmAccess`/`Queries` are cross-stack |
| `confidence` | `Confidence` | Validated to `[0,1]`; priors uncalibrated until M08 |
| `provenance` | `Provenance` | Which analysis fired; decides derived-vs-inferred |
| `evidence` | `Evidence` | Non-empty; never source text/env values (RULE 015) |
| `location` | `SourceLocation` | Repository-relative, 1-based; absolute paths rejected at construction |
| `commit` | `Option<CommitId>` | `None` = unknown until gix lands (M07) |
| `created_at` | RFC 3339 | Fixed clock available for byte-stable fixtures |

All fields except `commit` are required **by the constructor** — an
unevidenced edge does not compile (ADR-0004).

## Node kinds

Repository · Package · Directory · File · Module · Class · Function · Method ·
Variable · Route · Table · Column · ExternalService · EnvVar (name only, never
a value).

## Identity caveat

`NodeId`/`EdgeId` are allocated per graph from zero. Handles must not cross
graphs; content-addressed identity is an M10 deliverable, designed alongside
the invalidation model rather than inherited by accident.
