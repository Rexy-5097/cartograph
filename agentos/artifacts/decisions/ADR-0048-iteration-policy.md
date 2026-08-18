# ADR-0048: Iteration Policy

> **Status:** Accepted | **Date:** 2026-07-02 | **Decider:** Principal Architect

---

## Context

Different tasks require varying quality-versus-speed trade-offs. Rapid hackathon work shouldn't loop 10 times, whereas critical space flight software demands maximum quality iterations.

## Decision

Introduce **Loop Modes** configured in `runtime/policies/loop.yaml`:
- **Fast:** Max 2 loops, lower target score.
- **Balanced:** Max 3 loops, recommended target score.
- **Research:** Max 8 loops.
- **Production / ISRO:** Max 10 loops, lock at 100/100 target score.

## Consequences

- Developers can adjust pipeline speed dynamically via CLI flags.
- Optimization constraints are aligned to project profiles.
