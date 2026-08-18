# ADR-0018: Context Loading Strategy

> **Status:** Accepted | **Date:** 2026-07-02 | **Decider:** Principal Architect

---

## Context

AI agent context windows are expensive and suffer from "lost in the middle" degradation. Loading all documentation, standards, and code files for every task causes extreme token waste and dilutes model focus.

## Decision

Implement a **Tiered Selective Context Loading Strategy**:
1. **Mandatory Base:** All agents read `context/state.md`, `context/vision.md`, and `context/workflow.md` to get basic project alignment.
2. **Conditional Domain:** Agents read standards and metrics *only* if the files modified fall into their trigger domain (e.g. `ui-reviewer` only loads `standards/ui_ux.md`).
3. **Index Pattern:** For large directories (like decisions or experiments), load the index file first. Follow specific links only if the context is relevant to the task.

## Consequences

- Average agent session context footprint is reduced by up to 70%.
- Reviews are more targeted and accurate since the agent's focus is limited to the active domain standard.
