# ADR-0044: Failure Recovery

> **Status:** Accepted | **Date:** 2026-07-02 | **Decider:** Principal Architect

---

## Context

Failed quality checks can trigger infinite retry loops if not controlled by escalation boundaries.

## Decision

Implement standard **Failure Recovery Mappings** read from `runtime/policies/retry.yaml`:
1. Limit retries to a configured maximum (default: 2 or 3).
2. If max retries are exceeded, escalate directly to the chief-architect target.
3. Cordon system timeouts and halt execution to guarantee termination.

## Consequences

- Guarantees that execution pipelines always terminate.
- Preserves a clean history trace of gate failures for subsequent debug analysis.
