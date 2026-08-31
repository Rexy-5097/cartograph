# ADR-0013 — M10 may change the resolver, but not M09's semantics

**Status:** Accepted · 2026-08-31 · Supersedes a working constraint of M10, not any accepted checkpoint

## Context

M10's working rule was that the analyser stay **byte-identical** to
`cartograph-m09`: `git diff cartograph-m09 -- crates/cartograph-{core,graph,parser,resolver,testkit}`
had to be empty. That rule did its job through the parse-cache slice, which
lives entirely in the CLI and needed no resolver change.

It then blocked the milestone. Tracing `discover_accesses` proved that a Stage C
result depends on more than the asking file's content and the merged model set:

- it follows imports into other modules, and through re-export chains up to
  `MAX_REEXPORT_HOPS`, reading each hop's symbols;
- `resolve_python_module` matches a dotted specifier against the repository's
  **file paths** and answers only when exactly one candidate matches, so adding
  a file whose path collides — even one containing only `VERSION = 1` — turns a
  resolved import into an unresolved one and withdraws an edge.

Neither dependency can be predicted from a content hash before the resolution
runs. The read set has to be **recorded during** resolution, and the code that
performs the reads lives in `cartograph-resolver`. Keeping the crate
byte-identical would have left only unsound options: a content-hash-only key
that silently misses the path-set dependency, or no incremental resolution at
all.

A byte-identical *implementation* was never the property worth protecting. The
property worth protecting is that M10 does not change what Cartograph
**claims** about a repository.

## Decision

M10 may modify `crates/cartograph-resolver` (and the other analyser crates
where the same argument applies).

The constraint becomes semantic rather than textual:

> M10 may change the resolver implementation, but the semantics accepted at
> `cartograph-m09` must be preserved for every supported input, unless an ADR
> reviewed and accepted by the project owner documents an intentional change.

"Semantics" is the canonical graph: node and edge semantic identity, including
kind, name, source location, confidence, provenance and evidence. Excluded are
only the values already documented as unstable — `NodeId`, `EdgeId`, insertion
order and `created_at`.

**The clean full rebuild remains the reference oracle.** It is not deleted,
replaced or reimplemented. Every M10 slice compares against it with canonical
equality, and a difference is a defect in the slice, never an update to the
oracle.

Where an operation gains a tracked variant, the untracked entry point
**delegates to it** rather than being duplicated, so a single implementation
serves both and tracking cannot drift from the behaviour it describes.

## Consequences

- The M10 verification step changes from "diff is empty" to "canonical graph
  equality against the oracle, plus the full M07/M08/M09 regression suite".
  The weaker mechanical check is replaced by a stronger semantic one, and the
  regression suite is what enforces it.
- A reviewer loses the ability to confirm "the analyser did not change" by
  reading a diffstat. In exchange the differential suite states the property
  that actually matters, on every supported mutation class.
- `cartograph-m09` is untouched. This ADR changes how M10 is developed; it
  makes no claim about what M09 contained, and the accepted checkpoint and its
  bookkeeping stand exactly as recorded.
- An intentional semantic change now requires its own ADR. Nothing here
  pre-authorises one.

## Alternatives

- **Keep byte-identical, cache on content hashes only** — rejected: it cannot
  see the path-set dependency, so a file creation would leave a stale edge.
  That is the precise failure M10 exists to prevent.
- **Keep byte-identical, abandon incremental resolution** — rejected: it
  concedes the milestone to preserve a check that was a proxy for the real
  property.
- **Fork the resolver into an incremental copy** — rejected: two
  implementations of resolution is two answers to the same question, the defect
  ADR-0011 and the M09 pipeline consolidation were written to avoid.
