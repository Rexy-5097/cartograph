# ADR-0051: Infinite Loop Prevention

> **Status:** Accepted | **Date:** 2026-07-02 | **Decider:** Principal Architect

---

## Context

Flaky tests or contradictory agent logic can trigger endless back-and-forth iteration cycles.

## Decision

Enforce absolute **Loop Protection Constraints** in the execution monitor:
1. Max cycles hardcoded to 10.
2. Max execution duration restricted to 600 seconds.
3. If constraints are exceeded, halt loop and escalate directly to `chief-architect`.

## Consequences

- Guarantees complete termination safety in all environments (including local CLI and CI/CD pipelines).
- Protects API token budgets from runaway billing.
