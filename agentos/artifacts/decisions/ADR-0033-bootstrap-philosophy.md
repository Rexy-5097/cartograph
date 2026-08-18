# ADR-0033: Bootstrap Philosophy

> **Status:** Accepted | **Date:** 2026-07-02 | **Decider:** Principal Architect

---

## Context

Setting up a framework manually creates friction and documentation decay. A platform should be self-configuring and initialize itself automatically, with the AI agent serving as the primary bootstrap executor.

## Decision

Establish the **Bootstrap Layer**:
1. Add `BOOTSTRAP.md` as the universal starting document for AI agents.
2. Add `START_PROJECT.md` specifying the interface instructions for LLM assistants.
3. Decouple setup logic into an executable script (`tools/scripts/bootstrap_project.py`).

## Consequences

- Teammates can start new projects without architecture training.
- Configuration is standardized across all project repositories.
