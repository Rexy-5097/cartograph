# ADR-0028: Artifact System Architecture

> **Status:** Accepted | **Date:** 2026-07-02 | **Decider:** Principal Architect

---

## Context

Static templates are often ignored or drift from standard designs over time. We need to treat all engineering artifacts produced by AgentOS as verifiable system boundaries with strict metadata contracts.

## Decision

Transition the template blueprint layer into the **Artifact System Layer**:
1. Blueprints act as structural schemas rather than narrative markdown examples.
2. Every generated document must parse against the schemas.
3. Define 10 distinct artifact types covering ADRs, experiments, bug reports, feature proposals, plans, risk registers, and incident reports.

## Consequences

- Reviewer agents can automate parsing and checks.
- Guarantees alignment across all repository metadata.
- Prepares documents for automated indexing by MCP databases.
