# ADR-0026: Evidence-Based Verification

> **Status:** Accepted | **Date:** 2026-07-02 | **Decider:** Principal Architect

---

## Context

Saying a check passed is insufficient. We need to audit *why* it passed to ensure compliance.

## Decision

Every checklist check must provide **evidence**:
- Evidence includes specific file paths, build logs, hashes, test outputs, or contrast ratios.
- Reviewer agents must parse this evidence and verify it before signing off.
- The output report must include the evidence links.

## Consequences

- Prevents falsified checks.
- The git commit history acts as a permanent, verifiable audit trail of system compliance.
