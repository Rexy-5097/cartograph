# ADR-0055: Validation Metrics

> **Status:** Accepted | **Date:** 2026-07-02 | **Decider:** Principal Architect

---

## Context

We need a mathematical scorecard to aggregate test runs rather than reading logs manually.

## Decision

Define and calculate the following metrics:
- **Pass Rate:** Total passes divided by total runs.
- **Subsystem Coverage:** Number of subsystems covered by passed tests divided by total subsystems.
- **Deltas Trend:** Iterations run and final quality scores achieved.
Results compile into a markdown scorecard (`validation/reports/dashboard.md`).

## Consequences

- Clear, scannable compliance metrics are integrated directly into reports.
- Provides historical logs of framework stability.
