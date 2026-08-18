# Workflow — Cartograph milestone loop

> Layer: project · Adapted from [../workflows/master.md](../workflows/master.md)

Cartograph executes one milestone at a time. The loop, per milestone:

1. **Branch** — `feature/mNN-slug` from latest `main`. Never work on `main`.
2. **Read** — milestone definition in `milestones/`, relevant ADRs, state.
3. **Implement** — scope of the current milestone only. Conventional, atomic commits.
4. **Validate** — `make check` (fmt, clippy -D warnings, tests, AgentOS validator), then `make gates` (QG-001…008), then milestone-specific acceptance criteria.
5. **Self-review** — diff, dependency list, secrets scan, machine-path scan, docs accuracy. Self-review is not final authority (RULE 024).
6. **PR** — push branch, open PR against `main` with the template filled in. Wait for CI.
7. **Checkpoint** — after the human owner accepts: tag `cartograph-mNN`, update `CHECKPOINTS.md`, `context/state.md`, `artifacts/project-state.yaml`.
8. **STOP** — report to the human owner. Do not begin the next milestone (RULE 025).

A failed gate blocks advancement (RULE 017). A milestone is complete when its
gate passes, not when its code exists.
