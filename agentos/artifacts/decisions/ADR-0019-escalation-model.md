# ADR-0019: Escalation Model

> **Status:** Accepted | **Date:** 2026-07-02 | **Decider:** Principal Architect

---

## Context

When agents encounter errors, missing context, or conflicting constraints, they must have a predictable path to resolve issues. If escalation paths are circular (e.g. Agent A escalates to Agent B, which escalates to Agent A), the system will hang or consume infinite tokens.

## Decision

Enforce a **Linear, Non-Circular Escalation Model**:
1. **Specialist Reviewers** escalate only to the `chief-architect` (for design/rules conflicts) or the `orchestrator` (for routing queries).
2. **Orchestrator** escalates only to the `chief-architect` (for domain conflicts) or the **Human Engineer** (for intake/capacity queries).
3. **Chief Architect** escalates only to the **Human Engineer**.
4. **Human Engineer** is the terminal de-escalation authority.

No agent is allowed to escalate down the tree or route in loops.

## Consequences

- Every escalation path terminates in a finite number of steps.
- Static validation suite (`validate_agentos.py`) checks and guarantees zero circular dependencies.
