# M10 — Incremental analysis engine

Target: Week 8 · Branch: `feature/m10-incremental-engine` · Tag on acceptance: `cartograph-m10`

Scope: content-hash node identity, hand-built dependency invalidation (salsa
deliberately deferred), notify file watching. Targets: <250ms per normal file
change (later <100ms).

Acceptance: measured incremental latency on the benchmark corpus; correctness
vs full re-analysis on golden fixtures; gates pass.

Standard exit: PR + CI green + self-review + human acceptance + checkpoint tag + state update + STOP.

---

## Delivered scope (ADR-0014)

Of the three scope items above, **one was built**: hand-built dependency
invalidation, applied to the Stage C per-file ORM access contribution, which
measurement showed was 94.1% of a cold resolve on zulip.

**Deferred, with a governance record in
[ADR-0014](../../docs/adr/ADR-0014-m10-scope-reconciliation.md):**

| Item | Status | Why |
|---|---|---|
| content-hash node identity | **deferred** | not needed by the cache; **M12 and M13 require it** |
| notify file watching | **deferred** | a lifecycle feature; needs persistence, which is also deferred |
| local graph database (ROADMAP 0.2) | **deferred** | needs a pinned hash, schema version and corruption story |
| `<250 ms` per change | **superseded as a criterion** | never adopted; the priority was correctness over speed, and the evidence is work avoided |

The three **acceptance** criteria — measured incremental latency on the
benchmark corpus, correctness against full re-analysis, gates pass — are met.
Read ADR-0014 before treating the scope line above as what shipped.
