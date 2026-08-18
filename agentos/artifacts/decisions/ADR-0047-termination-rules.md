# ADR-0047: Termination Rules

> **Status:** Accepted | **Date:** 2026-07-02 | **Decider:** Principal Architect

---

## Context

Execution loops can consume massive resources if not limited by strict termination bounds.

## Decision

Enforce structured **Termination Rules** evaluated at the end of each iteration:
- **Quality achieved:** Halt loop immediately when quality target matches or exceeds thresholds.
- **Max iterations:** Stop execution if cycle counts exceed mode limits.
- **Negligible delta:** Terminate if improvement delta is zero or negative across iterations.

## Consequences

- Assures that execution pipelines always terminate.
- Restricts unnecessary token usage.
