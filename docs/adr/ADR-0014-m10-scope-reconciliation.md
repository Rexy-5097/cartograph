# ADR-0014 — M10 delivers incremental Stage C; three scope items are deferred

**Status:** Accepted · 2026-08-31 · Accepted by the project owner as the
governing record of M10's delivered scope

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

**M10 may be accepted without stable cross-run node identity.** M10's cache
correctness rests on semantic dependency identities and semantic graph
equality, never on persistent `NodeId` values — verified below, not asserted.

The following are **deferred, not delivered**:

### 1. Content-hash node identity — deferred, with a prerequisite gate

`NodeId` remains graph-local. `cartograph-core/src/id.rs` is byte-identical to
`cartograph-m09`.

**Why M10 does not need it.** Two independent mechanisms were checked in the
code rather than assumed:

- `access_cache::Entry` keys a cached per-file result on `file_content`,
  `model_set`, a read-set of `(repository-relative path, content identity)`
  pairs, and the optional `path_set` / `alias_set` fingerprints. Every
  component is a `u64` content identity or a relative path. No `NodeId`
  participates in cache identity or in reuse eligibility.
- `cartograph-testkit::canonical` compares graphs by *semantic* identity and
  **excludes `NodeId`, `EdgeId`, insertion order and `created_at`** by
  construction. The differential suite therefore already corresponds nodes
  between two independently built graphs.

**What is actually missing is a stable content-addressed *handle*, not the
ability to correspond nodes across runs.** That distinction matters for
planning the successor work, and it narrows it.

**Where the deferral binds.** ADR-0011 deferred stable identity *to M10*, so
deferring again requires naming who inherits it. Checked against the milestone
definitions rather than against this ADR's earlier assumption:

| Milestone | Stated acceptance | Requires cross-run identity? |
|---|---|---|
| M12 — blast radius | "blast query on benchmark repos returns cross-stack impact sets with evidence" | **Not by the criterion as written.** Reverse reachability is computed over *one* graph, and ADR-0011's within-graph identity is what makes the cross-stack chain walkable. A prerequisite arises only if M12's implementation compares across analyses or holds handles across incremental updates. |
| M13 — structural DIFF | "diff of two real branches lists added/removed/changed edges"; `layout(prev_graph, new_graph, prev_positions)` | **Yes, unconditionally.** Two branches are two independent analyses. Both the diff and the mental-map-stable layout are defined as functions of a correspondence between a previous and a new graph. |

M13 additionally needs *more* than what exists today, which the existing keys
do not supply:

- `canonical::node_identity` is `(kind, name, file:line)`. A function that
  moves three lines down is a different identity, so a diff built on it would
  report a removal plus an addition for an unchanged function.
- `node_for`'s identity is `(kind, name, file)` — line-independent, and
  therefore closer — but it still cannot express a moved file or a renamed
  artefact as the *same* node.

So identity work is a real slice with real design content, not a rename of an
existing key.

**Acceptance gate for future identity work.** Before any milestone that
requires correspondence between the same node across independent analyses, a
stable semantic identity slice must land and be checkpointed. It is accepted
only when:

1. identity is derived from content and location semantics, never from
   insertion order or allocation;
2. two independent analyses of the same commit assign the same identity to
   every node — asserted on a real repository, not a fixture alone;
3. identity is stable under a change elsewhere in the file, so an unrelated
   edit does not renumber unaffected nodes;
4. the move and rename cases are given a defined answer — matched, or
   deliberately reported as remove-plus-add — and that answer is tested;
5. the existing canonical equality relation continues to hold, so the M10
   differential suite still passes unchanged.

**Sequencing.**

- **M10** — deferred, as recorded here.
- **M12** — must open with an explicit determination of whether its
  implementation needs cross-run correspondence. If it does, it begins with the
  identity slice above; if it does not, it records that and proceeds. It is a
  decision point at kickoff, not an assumed prerequisite.
- **M13** — **may not assume stable identity exists.** Unless M12 delivered and
  checkpointed it, M13 begins with the identity slice above.

Historical milestones are not renumbered and M10's history is not rewritten.

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
- Four places told the reader that stable identity arrives at M10 —
  `cartograph-core/src/id.rs`, `cartograph-core/src/lib.rs`,
  `cartograph-graph/src/lib.rs` and `docs/graph/README.md`. Those sentences
  become false on acceptance and are corrected to name this ADR and the gate
  above, so that no file promises something no milestone owns.
- ADR-0011 records the same deferral and is now partly overtaken; it carries a
  forward pointer here. Its own decision — within-graph identity — is
  unaffected and still stands.
- **M13 inherits a hard prerequisite; M12 inherits a decision point.** This is
  narrower than the claim this ADR made while it was Proposed, which asserted
  that both required cross-run identity. M12's stated acceptance does not.
- Persistence and watching remain available as a later slice, and the
  dependency model built here is what they would need first.
- No claim is made anywhere that stable identity is implemented. It is not.

## Alternatives

- **Build all four before accepting** — rejected: persistence and watching rest
  on an invalidation model that had to be proven first, and the correctness
  work is complete and independently valuable now.
- **Quietly drop the unbuilt items** — rejected. A milestone that closes over
  an unstated gap is how the M09 acceptance bookkeeping was lost, and how
  `id.rs` would come to promise something no milestone owns.
- **Accept content-hash identity as out of scope permanently** — rejected: it
  is a real requirement with a named downstream consumer, so it is deferred
  with that consumer and an acceptance gate recorded rather than removed.
- **Declare identity a blanket prerequisite for both M12 and M13** — rejected
  as unverified. M12's acceptance criterion is reverse reachability over one
  graph, which ADR-0011 already serves. Recording a prerequisite that the
  milestone definition does not support would front-load work on a guess.
