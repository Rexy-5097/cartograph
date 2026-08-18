# ADR-0006: Context Layer as Operational Intelligence

> **Status:** Accepted | **Date:** 2026-07-02 | **Decider:** Principal Architect

---

## Context

The `context/` directory could have been designed as:
A) Static project documentation (README-style prose)
B) A project wiki (comprehensive, narrative)
C) Operational intelligence (structured, compact, agent-optimized)

The choice shapes how every agent interaction works for the entire life of the project.

## Problem

Documentation written as prose is:
- Token-inefficient when loaded repeatedly into AI context windows
- Hard to update incrementally
- Prone to becoming stale because updates require rewriting paragraphs
- Hard to parse programmatically

## Alternatives Considered

**Option A: Prose documentation**
- Human-friendly but token-heavy
- Stales quickly
- Rejected: too expensive at scale across hundreds of sessions

**Option B: Database/structured data (YAML/JSON)**
- Machine-readable but not human-editable without tooling
- Rejected: breaks the "plain text, no tools required" principle

**Option C: Structured markdown with tables, tags, and size budgets**
- Accepted: human-readable + AI-parseable + incrementally updateable

## Decision

Design every `context/` file as **structured operational intelligence**:
- Tables over prose
- Status tags over descriptions (`IN_PROGRESS`, `BLOCKED`, etc.)
- Explicit size budgets per file (token targets)
- Cross-references instead of duplication
- Archive rules to prevent unbounded growth

## File-Level Size Budgets

| File | Token Budget | Rationale |
|------|-------------|-----------|
| `state.md` | ~400 tokens | Mandatory first read — must be minimal |
| `vision.md` | ~600 tokens | Stable — loaded less frequently |
| `memory.md` | ~1000 tokens | Conditional load — more content acceptable |
| `architecture.md` | ~800 tokens | Conditional load — needs diagrams |
| `tech_stack.md` | ~700 tokens | Conditional load — needs context per tech |
| `workflow.md` | ~400 tokens | Mandatory read — must be minimal |
| `decisions.md` | ~300 tokens | Index only — absolute minimum |

## Consequences

- All context files use consistent structure (owner, consumers, size budget header)
- Engineers know exactly how much information each file should contain
- Agents know which files to read based on task type
- Token costs remain predictable across the project lifetime

## Trade-offs

The structured format requires engineers to maintain structure discipline. Prose is easier to write. The payoff — hundreds of more efficient agent sessions — justifies the initial friction.
