# ADR-0007: Token Compression Strategy

> **Status:** Accepted | **Date:** 2026-07-02 | **Decider:** Principal Architect

---

## Context

Every token in a context file is reloaded every time an agent starts a session. A 2000-token context file loaded 500 times costs 1,000,000 tokens — the equivalent of a full book. Across a long project, context verbosity becomes a significant cost.

## Problem

How do we maintain rich context without paying unbounded token costs as the project grows?

## Decision

Apply a multi-layer token compression strategy:

### Layer 1: Format Compression
- Tables instead of prose paragraphs
- Status tags instead of status descriptions
- Bullet points instead of sentences where possible
- Cross-references instead of content duplication

### Layer 2: Size Budgets
Each context file has a defined token budget (see ADR-0006). Files must not exceed their budget.

### Layer 3: Selective Loading
Agents load only what their task requires:
- `state.md` + `vision.md` + `workflow.md` — always
- `architecture.md`, `tech_stack.md` — conditional
- `memory.md` — conditional
- `decisions.md` — conditional

### Layer 4: Archive Rules
Files with unbounded growth potential (`memory.md`, `state.md` completed tasks) have explicit archive rules: move old entries to `artifacts/` when size budget is exceeded.

### Layer 5: Index Pattern
`context/decisions.md` is an index — one row per decision. Full reasoning lives in `artifacts/decisions/`. Agents read the index first and follow links only when they need the full reasoning.

## Consequences

- Every context file remains within its token budget throughout the project
- Agent sessions start faster with less context loading
- The project can run for years without context files becoming unwieldy
- Engineers must discipline themselves to archive rather than append indefinitely

## Measurement

Token budgets should be verified monthly by checking file size against targets.
