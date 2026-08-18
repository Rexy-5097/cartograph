# ADR-0017: Agent Contract and Lifecycle Design

> **Status:** Accepted | **Date:** 2026-07-02 | **Decider:** Principal Architect

---

## Context

To automate agent orchestration and monitor execution, we cannot treat agent invocation as a black box. We need to track the internal progress of agents through distinct operational stages.

## Decision

Every agent in AgentOS must implement and expose a 7-stage lifecycle state machine:
1. `Idle` - Waiting for task assignment.
2. `Invoked` - Initialized with task payload (modified files, user prompt, diff).
3. `Loading Context` - Reads context and config files specified in its contract.
4. `Reviewing` - Evaluates changes against standards and metrics.
5. `Decision` - Runs the binary checklist and decides output status.
6. `Output` - Formats and logs the finding report.
7. `Completed` - Cleans up resources and yields execution flow.

## Consequences

- Orchestrators can track execution progress and identify bottlenecks.
- Enables dry-run simulation of state transitions.
- Simplifies recovery when an agent fails in a specific state.
