# ADR-0039: Runtime Orchestration

> **Status:** Accepted | **Date:** 2026-07-02 | **Decider:** Principal Architect

---

## Context

Executing tasks without a structured pipeline makes debugging runtime state transitions difficult. We need a clear audit trace of how requests are received, parsed, and completed.

## Decision

Enforce a strict 8-stage **Runtime State Machine** in the Harness kernel:
1. `Idle` (waiting for request)
2. `Receive Request` (ingest and parse JSON request object)
3. `Plan` (select context, workflow, agents, and compile execution plan)
4. `Dispatch` (trigger agents and tools)
5. `Monitor` (track elapsed time and retries)
6. `Collect` (gather result checklists)
7. `Validate` (run static validation checks)
8. `Complete` (generate execution report and return control)

## Consequences

- The Harness logs standard state transitions for telemetric logging.
- Provides a standard sequence for failure recovery points.
