# M10 — Incremental analysis engine

Target: Week 8 · Branch: `feature/m10-incremental-engine` · Tag on acceptance: `cartograph-m10`

Scope: content-hash node identity, hand-built dependency invalidation (salsa
deliberately deferred), notify file watching. Targets: <250ms per normal file
change (later <100ms).

Acceptance: measured incremental latency on the benchmark corpus; correctness
vs full re-analysis on golden fixtures; gates pass.

Standard exit: PR + CI green + self-review + human acceptance + checkpoint tag + state update + STOP.
