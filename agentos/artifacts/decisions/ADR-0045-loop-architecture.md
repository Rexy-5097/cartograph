# ADR-0045: Loop Architecture

> **Status:** Accepted | **Date:** 2026-07-02 | **Decider:** Principal Architect

---

## Context

Reviewer agents shouldn't manage iterative cycle loops themselves. Iterative runs are execution parameters that should be handled by a dedicated kernel runtime.

## Decision

Establish the **Loop Engine** as a modular runtime kernel under `runtime/loop/` that interfaces with the shared runtime kernel (`runtime/kernel/`). The Loop Runtime coordinates the standard execute-evaluate-reflect-plan-improve iteration cycle.

## Consequences

- Reviewer agents remain clean, focusing only on evaluation logic.
- Loop controls (retry limits, thresholds, optimization modes) are codified in configuration policies.
