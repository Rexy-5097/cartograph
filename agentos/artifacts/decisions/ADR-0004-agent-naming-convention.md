# ADR-0004: Agent Naming Convention

> **Status:** Accepted | **Date:** 2026-07-02 | **Decider:** Principal Architect

---

## Context

The initial agent naming was inconsistent — mixing `_reviewer`, `_agent`, and `_manager` suffixes with underscores. Consistency and semantic clarity are required.

## Options Considered

**Option A: All `*-reviewer`** — Proposed by review. Simple and consistent for reviewers. Problem: not all agents review. The planner plans. The orchestrator routes. The chief architect decides. Forcing `*-reviewer` onto non-reviewing roles creates semantic confusion.

**Option B: All `*-agent`** — Generic but loses meaning. Does not convey the agent's function.

**Option C: Role-appropriate naming with kebab-case** — Accepted.

## Decision

**Kebab-case filenames throughout.**

**Suffix by function:**
- Domain reviewers → `{domain}-reviewer` (e.g., `ai-reviewer`, `security-reviewer`)
- Unique orchestration → `orchestrator`
- Unique planning → `planner`
- Unique authority → `chief-architect`

**Rationale:** Naming should convey function. Reviewers review. The planner plans. The orchestrator routes. The chief architect decides. These are distinct roles that deserve distinct names.

## Result

| Old Name | New Name |
|----------|----------|
| `chief_architect.md` | `chief-architect.md` |
| `planner.md` | `planner.md` |
| `ai_reviewer.md` | `ai-reviewer.md` |
| `science_reviewer.md` | `science-reviewer.md` |
| `security_reviewer.md` | `security-reviewer.md` |
| `performance_reviewer.md` | `performance-reviewer.md` |
| `ui_reviewer.md` | `ui-reviewer.md` |
| `qa_agent.md` | `qa-reviewer.md` |
| `docs_agent.md` | `docs-reviewer.md` |
| `release_manager.md` | `release-reviewer.md` |
| *(new)* | `orchestrator.md` |

## Consequences

All agent cross-references updated throughout the repository.
