# ADR-0038: Harness Architecture

> **Status:** Accepted | **Date:** 2026-07-02 | **Decider:** Principal Architect

---

## Context

Reviewer agents must focus strictly on evaluation logic. Mixing execution flow controls (such as task selection, routing, and tool triggering) into the agent layer results in complex, non-deterministic agent loops and duplicate work.

## Decision

Establish the **Harness Engine** as a modular runtime kernel independent of the AI agent reasoning layer. The Harness Engine:
1. Is built under `runtime/harness/` with modular single-responsibility scripts.
2. Is 100% deterministic Python logic.
3. Coordinates task classification, routing, and budget validation before dispatching agents.

## Consequences

- Simplifies reviewer agent contracts.
- Makes orchestration rules reproducible and testable.
- Ensures the platform kernel can evolve without model changes.
