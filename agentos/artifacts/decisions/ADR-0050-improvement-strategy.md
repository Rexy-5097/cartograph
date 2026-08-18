# ADR-0050: Improvement Strategy

> **Status:** Accepted | **Date:** 2026-07-02 | **Decider:** Principal Architect

---

## Context

Different failure modes require distinct correction mechanisms. For example, syntax issues require code edit corrections, whereas latency budgets require optimization.

## Decision

Introduce the **Improvement Planner** (`runtime/loop/improvement_planner.py`) mapping reflection outputs to target improvement strategies:
- **Error correction:** Fast patch edits.
- **Optimization:** Refactoring loops or database indexing.
- **Security hardening:** Patching access credentials.

## Consequences

- The system chooses targeted refactoring pathways.
- Minimizes random AI search walks.
