# ADR-0041: Context Optimization

> **Status:** Accepted | **Date:** 2026-07-02 | **Decider:** Principal Architect

---

## Context

Loading the entire repository context for simple tasks overflows LLM windows and inflates prompt token charges.

## Decision

Enforce a modular **Context Loading Policy** configured in `runtime/policies/context.yaml`:
1. Load mandatory global context files only (Bootstrap, vision, state).
2. Load specialist agent specs and checklists conditionally based on routed targets.
3. Prevent duplicate loading of identical files during a single runtime run.

## Consequences

- Reduces prompt context size to < 4000 tokens.
- Minimizes cost while focusing the agent's attention on relevant rules.
