# ADR-0024: Checklist Design Strategy and Gate Metadata

> **Status:** Accepted | **Date:** 2026-07-02 | **Decider:** Principal Architect

---

## Context

Automated CI/CD pipelines and orchestrator agents need metadata to schedule and monitor checklist execution (e.g. knowing who owns it, how long it takes, and whether it can be retried).

## Decision

Enforce a standardized metadata block at the top of every checklist file containing:
- **Gate ID:** Unique `QG-NNN` string for routing.
- **Version:** Checklist version.
- **Estimated Runtime:** Approximate time (e.g. 5 min, 30 min) to help the orchestrator allocate resources.
- **Gate Severity:** `Mandatory`, `Recommended`, or `Optional` to determine if failure is a blocker.
- **Automation Level:** `Automatic`, `Semi-automatic`, or `Manual` to determine execution method.
- **Retry Policy:** Allowed status, max retries, and escalation rules.

## Consequences

- Orchestrators can dynamically route and time quality checks.
- Static parser validators can audit checklists for compliance.
