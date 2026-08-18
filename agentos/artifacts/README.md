# Records

> **Layer:** Generated | **Accumulates over project lifetime**

---

## Purpose

Records are **project-generated artifacts** — filled instances of templates created during the project lifecycle.

Unlike templates (pristine shells), records accumulate and are never deleted.

---

## Record Types

| Directory | Contents | Naming Convention |
|----------|---------|-----------------|
| `decisions/` | Architecture Decision Records | `ADR-{number:04d}-{slug}.md` |
| `experiments/` | Completed experiment logs | `EXP-{YYYY-MM-DD}-{slug}.md` |
| `incidents/` | Post-mortem records | `INC-{YYYY-MM-DD}-{slug}.md` |
| `releases/` | Release notes history | `REL-{version}.md` |

---

## Rules

- Never delete records — they are the audit trail of the project.
- Superseded decisions are marked `status: Superseded`, not deleted.
- Index decisions in `context/decisions.md`.
- Agents read the index; they follow links only when needed.

---

*Records accumulate here over the project lifetime.*
