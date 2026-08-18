# ADR-0042: Execution Planning

> **Status:** Accepted | **Date:** 2026-07-02 | **Decider:** Principal Architect

---

## Context

Without a structured plan contract compiled before execution, we cannot verify if the platform is about to run out of budget, run an incorrect workflow, or dispatch missing agents.

## Decision

The Harness Engine must compile a structured **Execution Plan Object** before running any external tools or agents. This plan defines the classification, workflows, dispatches, context, and stopping conditions, serving as the execution contract.

## Consequences

- The plan can be reviewed by the developer or verified by static checkers before dispatch.
- Serves as the source of truth for the runtime execution monitor.
