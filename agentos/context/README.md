# Context Layer — Project Intelligence

> **Layer:** Project-owned | **Status:** Production-ready templates
> **Purpose:** Live operational intelligence for agents and engineers

---

## Design Philosophy

The `context/` directory is **not documentation**. It is **live operational intelligence**.

Every file is:
- **Structured** — tables, status tags, and checklists over prose
- **Token-efficient** — strict size budgets per file
- **Cross-referenced** — links instead of duplication
- **Incrementally updateable** — add a row, not rewrite a paragraph

See [ADR-0006](../artifacts/decisions/ADR-0006-context-layer-design.md) and [ADR-0007](../artifacts/decisions/ADR-0007-token-compression-strategy.md) for the architectural rationale.

---

## File Reference

| File | Owner | Agents Load | Budget | Update Freq |
|------|-------|------------|--------|------------|
| `vision.md` | Project Lead | All (always) | ~600 tokens | Pivots only |
| `state.md` | Project Lead | All (always, first) | ~400 tokens | Every session |
| `workflow.md` | Project Lead | All (always) | ~400 tokens | Process changes |
| `architecture.md` | Chief Architect | Conditional | ~800 tokens | Architecture changes |
| `tech_stack.md` | Tech Lead | Conditional | ~700 tokens | Stack changes |
| `memory.md` | All (append) | Conditional | ~1000 tokens | Pattern discovery |
| `decisions.md` | All (append) | Conditional | ~300 tokens | Every decision |

**Always loaded:** `vision.md` + `state.md` + `workflow.md`
**Conditionally loaded:** all others — based on task domain

---

## Token Budget Note

The files in this directory are **templates** — they contain annotation and examples that will be replaced with project-specific content when used. Filled production files will be smaller than the templates. Budget targets apply to filled production files, not to the templates themselves.

---

## Required Context Loading

See `workflows/master.md` § Required Context Loading for the full agent loading protocol.

Quick reference:

```
Any task:           vision.md + state.md + workflow.md
Architecture task:  + architecture.md
Tech choices:       + tech_stack.md
Patterns/gotchas:   + memory.md
Past decisions:     + decisions.md (index) → follow links as needed
```

---

## File Population Guide

When setting up a new project:

1. `context/vision.md` — Fill in all 9 sections. Keep the Problem Statement to 3 sentences.
2. `context/state.md` — Set Phase, Sprint 1, first tasks. Update every session.
3. `context/workflow.md` — Mark which workflows are active; add project deviations.
4. `context/architecture.md` — Draw the component map; fill the component table.
5. `context/tech_stack.md` — Fill the Core Stack table; add ML/AI stack if applicable.
6. `context/memory.md` — Leave mostly empty; it fills naturally during the project.
7. `context/decisions.md` — Already contains the initial ADR index from this template.

---

## Anti-patterns

| Anti-pattern | Why It Fails | Fix |
|-------------|-------------|-----|
| Writing prose instead of tables | Expensive to reload; hard to update | Use table structure |
| Duplicating content across files | Drift and inconsistency | Cross-reference |
| Letting `state.md` go stale | Agents work on wrong priorities | Update every session |
| Growing `memory.md` without archiving | Context overload | Archive at 1000-token limit |
| Putting rationale in `decisions.md` | Index becomes too large | Put reasoning in `artifacts/decisions/` |
| Adding implementation details | Wrong layer | Implementation details belong in code comments |

---

*Context layer — populate when starting a project, maintain throughout.*
