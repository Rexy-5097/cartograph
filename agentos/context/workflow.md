# Workflow — Cartograph milestone loop

> Layer: project · Adapted from [../workflows/master.md](../workflows/master.md)

Cartograph executes one milestone at a time. The loop, per milestone:

1. **Branch** — `feature/mNN-slug` from latest `main`. Never work on `main`.
2. **Read** — milestone definition in `milestones/`, relevant ADRs, state.
3. **Implement** — scope of the current milestone only, in **verified slices**:
   smallest slice → compile → targeted tests → inspect output → negative tests
   → no regression → next slice. Never accumulate a large unverified change
   set. See [Continuous Verification](../../docs/development/continuous-verification.md)
   (RULE 026, gate QG-009).
4. **Validate** — `make check` (fmt, clippy -D warnings, tests, AgentOS validator), then `make gates` (QG-001…009), then milestone-specific acceptance criteria. For resolver and static-analysis milestones, real-repository validation is mandatory, and unexpected output must be classified rather than ignored.
5. **Self-review** — diff, dependency list, secrets scan, machine-path scan, docs accuracy. For high-risk milestones this must explicitly cover correctness, false positives, false negatives, ambiguity, unsupported cases, security, performance, backward compatibility, scope leakage, evidence quality and reproducibility. Self-review is not final authority (RULE 024).
6. **PR** — push branch, open PR against `main` with the template filled in. Wait for CI.
7. **Checkpoint** — after the human owner accepts: tag `cartograph-mNN`, update `CHECKPOINTS.md`, `context/state.md`, `artifacts/project-state.yaml`.
8. **STOP** — report to the human owner. Do not begin the next milestone (RULE 025).

A failed gate blocks advancement (RULE 017). A milestone is complete when its
gate passes, not when its code exists.
