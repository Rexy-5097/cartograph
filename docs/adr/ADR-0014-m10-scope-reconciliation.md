# ADR-0014 — M10 delivers incremental Stage C; three scope items are deferred

**Status:** Proposed · 2026-08-31 · Requires the project owner's acceptance

## Context

`agentos/milestones/M10-incremental-engine.md` names three scope items:

> content-hash node identity, hand-built dependency invalidation (salsa
> deliberately deferred), notify file watching. Targets: <250ms per normal file
> change (later <100ms).

`ROADMAP.md` additionally maps release 0.2 — "Local graph database" — to M10.

**One of those is built. Three are not.** This ADR records that rather than
letting the milestone close with the difference unstated.

The work took the order the milestone's own acceptance criterion implies —
*correctness vs full re-analysis*, then measurement — and measurement decided
the rest. Stage C (per-file ORM access resolution) was 894.8 ms of a 950.9 ms
cold resolve on zulip, 94.1%. It is now cached, and a warm resolve is 60.4 ms.
The remaining derived structures were then measured rather than assumed:
`ExportedConstants` 1.8 ms, `ModuleIndex` 0.29 ms, `RouteIndex` 0.032 ms,
`RouterIndex` 0.005 ms. A warm zulip run spends 318.7 ms of 379 ms in
filesystem discovery, reads and hashing — 84%, and the floor of any
content-addressed design.

## Decision

M10's delivered scope is **correct incremental analysis of the Stage C
per-file ORM access contribution**, with the clean rebuild retained as oracle
and `incremental == clean` as the acceptance invariant.

The following are **deferred, not delivered**:

### 1. Content-hash node identity — deferred

`NodeId` remains graph-local. `cartograph-core/src/id.rs` is byte-identical to
`cartograph-m09`.

This is the one deferral that is a **capability**, not a target, and it carries
forward a promise: ADR-0011 deferred stable identity *to M10*, and `id.rs` says
so in its own documentation. Deferring it again means:

- graphs still cannot be compared across runs by node handle;
- **M12 (blast radius) and M13 (structural diff) both need cross-run identity**
  and will have to deliver it, or receive it from a dedicated slice first.

It was not needed for Stage C caching, because the cache keys per-file results
by content and dependency identity and never by `NodeId`, and because canonical
graph equality compares semantic identity rather than handles. That is why the
milestone could be built without it — not a reason it stops mattering.

### 2. Notify file watching — deferred

No `notify` dependency exists. Watching is a process-lifecycle feature: it is
only useful to a long-lived process, and the CLI exits after each run. It
depends on the persistent cache below, and delivering it would have meant
building both under a correctness-first mandate before the invalidation model
had been proven.

### 3. Local graph database / persistent cache — deferred

No `redb` dependency exists. Both caches are process-local, so **the CLI cannot
currently benefit from either across invocations**: the measured speedups are
in-process. Persistence needs an on-disk format, a pinned hash (the current
`DefaultHasher` is explicitly unsuitable — it is not stable across Rust
releases), a schema version and a corruption story.

### 4. The `<250 ms` target — superseded as an acceptance criterion

It was never adopted. The governing priority for this milestone was stated as
**correctness > speed > memory > implementation velocity**, and timing was
recorded as observation throughout. The acceptance evidence is **work avoided**
— cache hits, misses and recomputation counts — not milliseconds.

For the record, warm in-process figures are far below 250 ms (zulip resolve
60.4 ms), but they are **not** claimed as meeting the target: the target
concerns a per-file change in a watching process, which does not exist.

## Consequences

- M10's three stated **acceptance** criteria are met: latency measured on the
  benchmark corpus, correctness against full re-analysis, gates pass.
- M10's **scope** is narrower than written. Anyone reading the milestone file
  alone would over-estimate what shipped, so that file and `ROADMAP.md` are
  updated to point here.
- `id.rs` currently tells the reader stable identity arrives at M10. That
  sentence becomes false on acceptance and is corrected to name this ADR.
- M12 and M13 inherit the node-identity requirement. Sequencing it is the
  project owner's decision, not this ADR's.
- Persistence and watching remain available as a later slice, and the
  dependency model built here is what they would need first.

## Alternatives

- **Build all four before accepting** — rejected: persistence and watching rest
  on an invalidation model that had to be proven first, and the correctness
  work is complete and independently valuable now.
- **Quietly drop the unbuilt items** — rejected. A milestone that closes over
  an unstated gap is how the M09 acceptance bookkeeping was lost, and how
  `id.rs` would come to promise something no milestone owns.
- **Accept content-hash identity as out of scope permanently** — rejected: it
  is a real requirement with named downstream consumers, so it is deferred with
  those consumers recorded rather than removed.
