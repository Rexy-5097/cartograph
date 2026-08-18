# ADR-0025: Pass/Warning/Fail Decision Model

> **Status:** Accepted | **Date:** 2026-07-02 | **Decider:** Principal Architect

---

## Context

Binary pass/fail checks are sometimes too rigid. For instance, a minor formatting linter warning or a quarantined flaky test shouldn't halt a critical deployment. However, we cannot allow developers to bypass critical errors.

## Decision

Every quality gate must resolve to one of three states:
1. **PASS:** All items are verified YES. Score is 100.
2. **PASS WITH WARNINGS:** Low-risk items fail (e.g. formatting warnings, quarantined tests) but critical items pass. Score is 80-99.
3. **FAIL:** Any critical checklist item fails (e.g. failing tests, missing auth checks, committed secret). Score < 80. Blocks further pipeline progression.

## Consequences

- Balances safety with velocity.
- Allows pipeline execution to continue with warnings under explicit conditions.
- Standardizes scorecard outputs for telemetry tracking.
