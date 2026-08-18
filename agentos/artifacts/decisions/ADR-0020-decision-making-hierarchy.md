# ADR-0020: De-escalation and Decision-Making Hierarchy

> **Status:** Accepted | **Date:** 2026-07-02 | **Decider:** Principal Architect

---

## Context

Reviewer agents can produce conflicting outputs on cross-domain changes. For example, a performance optimization change that speeds up data ingestion (PASS from `performance-reviewer`) might violate query validation limits (FAIL from `security-reviewer`).

## Problem

How do we resolve conflicts between specialist reviewer agents without stalling or creating infinite negotiation loops?

## Decision

Establish a strict **De-escalation and Decision-Making Hierarchy**:
1. **Reviewers do not negotiate.** They report findings independently to the Orchestrator.
2. **Orchestrator identifies conflicts** and escalates them directly to `chief-architect`.
3. **Chief Architect acts as the final technical authority**, reviews both arguments against `ENGINEERING_PRINCIPLES.md`, and makes the binding decision.
4. **Decisions are codified as ADRs** in `artifacts/decisions/` to update system memory and prevent re-evaluating the same conflict.
5. **Human Engineer** retains absolute veto power over all decisions.

## Consequences

- Eliminates multi-agent debate loops, saving significant tokens.
- Codifies conflict resolutions as permanent architectural records.
