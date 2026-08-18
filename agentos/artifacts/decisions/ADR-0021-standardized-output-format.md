# ADR-0021: Standardized Output Format

> **Status:** Accepted | **Date:** 2026-07-02 | **Decider:** Principal Architect

---

## Context

If agents write reports in free-form narrative prose, downstream agents (like the Orchestrator) and static parser tools cannot reliably extract the status, confidence, and recommended fixes, leading to fragile integrations.

## Decision

Mandate a strict **Standardized Output Format** for all reviewer reports:
1. **Status:** Must be one of `PASS`, `FAIL`, or `CONDITIONAL_PASS`.
2. **Confidence:** `HIGH`, `MEDIUM`, or `LOW`.
3. **Evidence:** Concrete lines of code, test logs, or benchmark values.
4. **Findings:** Structured list categorized by severity (`[CRITICAL]`, `[MAJOR]`, `[MINOR]`).
5. **Recommendations:** Specific actions to address findings.
6. **Risks:** Technical or operational risks.
7. **Next Action:** Clear next step for the developer or pipeline.

## Consequences

- The Orchestrator can parse, filter, and merge reports programmatically.
- Quality gates can extract the `Status` value to fail builds or block merges.
