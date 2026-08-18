# ADR-0023: Quality Gate Philosophy

> **Status:** Accepted | **Date:** 2026-07-02 | **Decider:** Principal Architect

---

## Context

Typical software processes rely on manual checklists that developers fill out at the end of sprints. These checklists are often treated as bureaucratic paperwork, leading to developers checking boxes without actual verification.

## Decision

AgentOS defines **Quality Gates** as deterministic code check-gates rather than passive forms:
1. Every gate answers a binary question: "Is this work safe to advance?"
2. Gates are executed programmatically by reviewer agents or automated scripts.
3. Gates are paired directly with standards and metrics.
4. Human leads oversee gate reviews but the gate criteria themselves are hardcoded in the codebase.

## Consequences

- Prevents skipping security or testing checks under pressure.
- Builds an automated, reproducible trail of quality audits.
- Frees human reviewers to focus on creative tasks instead of verifying linters and metrics.
