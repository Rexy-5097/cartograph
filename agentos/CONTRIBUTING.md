# Contributing to AgentOS

> **Version:** 0.1.0 | **Layer:** Infrastructure

Thank you for contributing to AgentOS — reusable engineering infrastructure for long-term use across AI, scientific, and production software projects.

---

## Repository Philosophy

AgentOS is **not** a coding framework. It is an engineering operating system.

Every contribution must optimize for:
- **Long-term maintainability** over short-term convenience
- **Consistency** over individual style preferences
- **Modularity** — each component has one responsibility
- **Token efficiency** — every file is written to be read by agents within a context window
- **Vendor neutrality** — nothing outside `integrations/` should depend on a specific AI provider

Read [`ENGINEERING_PRINCIPLES.md`](./ENGINEERING_PRINCIPLES.md) before contributing. It is the philosophical foundation for all decisions.

---

## What Can Be Contributed

### ✅ Accepted contributions

| Contribution Type | Where |
|------------------|-------|
| New agents | `agents/` |
| New workflows | `workflows/` |
| New standards | `standards/` |
| New checklists | `checklists/` |
| New templates | `templates/` |
| New MCP integrations | `tools/mcp/` |
| New vendor integrations | `integrations/{vendor}/` |
| Lessons and improvements | `lessons/` |
| Bug fixes in existing documents | Any file |
| Clarity improvements | Any file |

### ❌ Not accepted

- Project-specific files (they belong in the project repository, not the template)
- Vendor-specific files outside `integrations/`
- Duplicate content that already exists in another file (cross-reference instead)
- Changes that violate the WAT architecture (Workflows → Agents → Tools)

---

## Documentation Standards

Every document in this repository must:

1. Include a YAML-style header:
   ```markdown
   > **Layer:** Infrastructure | **Phase:** N | **Status:** [Placeholder | Active | Authoritative]
   ```

2. Be written for **token efficiency** — assume an AI agent will read it under context pressure.

3. **Never duplicate** content from another file — cross-reference with a link instead.

4. Use **consistent heading structure**:
   - `#` — Document title
   - `##` — Major sections
   - `###` — Subsections

5. Include **cross-references** to related files.

---

## Agent Specification Standards

New agents must follow the full specification structure defined in `agents/README.md`:

- Mission statement (one sentence)
- Responsibilities (bullet list)
- Non-responsibilities (explicit exclusions — as important as responsibilities)
- Trigger conditions
- Required files to read
- Required standards
- Output format
- Confidence reporting
- Escalation path
- Completion criteria
- Token-efficient behavior guidelines

**Agent naming convention:** Use kebab-case.
- Reviewers: `{domain}-reviewer.md`
- Unique roles: descriptive name (`orchestrator.md`, `planner.md`, `chief-architect.md`)

---

## Standard Specification Requirements

New standards must define all of:

1. Purpose
2. Scope
3. Principles
4. Best Practices
5. Anti-patterns
6. Acceptance Criteria
7. Review Checklist (brief)
8. Cross-references (no duplication)

---

## Pull Request Process

1. **Open an issue first** for significant changes. Describe the problem and proposed solution.
2. **Create an ADR** in `artifacts/decisions/` for any architectural change. Use `templates/decision_record.md`.
3. **Follow the PR checklist** in `checklists/pull_request.md`.
4. **One reviewer minimum** — use the agent trigger map in `agents/README.md` to identify the correct reviewer.
5. **Update CHANGELOG.md** with the change and version bump per `VERSION_POLICY.md`.
6. **Update `manifest.yml`** in `.agentos/` for any new files.

---

## Review Process

Reviews focus on:
- Does this change violate the WAT architecture?
- Does this introduce duplication?
- Does this reduce modularity?
- Does this hurt token efficiency?
- Is this vendor-specific and outside `integrations/`?
- Is the documentation complete and correctly cross-referenced?

---

## Commit Message Format

```
{type}({scope}): {description}

Types: feat | fix | docs | refactor | standards | agents | checklists | templates
Scope: the directory or component being changed

Examples:
feat(agents): add data-engineer agent
fix(checklists): correct release checklist item 7
docs(standards): clarify security standard scope
refactor(context): restructure state.md format
```

---

## Questions

Open a GitHub issue with the label `question`.

---

*AgentOS CONTRIBUTING.md — Engineering infrastructure contribution guide.*
