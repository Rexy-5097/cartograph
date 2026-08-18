# ADR-0027: Checklist Sequencing and Post-Release Monitoring

> **Status:** Accepted | **Date:** 2026-07-02 | **Decider:** Principal Architect

---

## Context

Checking code quality stops when the code is deployed. However, systems can fail in production due to traffic load, memory exhaustion, or environment issues. The operating system must close the loop after deployment.

## Decision

1. Map quality gates in a non-circular linear sequence (QG-001 through QG-008).
2. Append a post-release **Monitoring** phase after deployment.
3. Deployed builds are monitored against performance budgets (`metrics/performance.md`).
4. If production monitoring alerts are triggered, the orchestrator raises a rollback instruction immediately.

## Consequences

- The development lifecycle is complete only when production metrics are stable.
- Bridges the gap between static testing and live runtime tracking.
