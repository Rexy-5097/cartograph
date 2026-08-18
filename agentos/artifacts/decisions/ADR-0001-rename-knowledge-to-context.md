# ADR-0001: Rename `knowledge/` to `context/`

> **Status:** Accepted | **Date:** 2026-07-02 | **Decider:** Principal Architect

---

## Context

The initial design named the live project context directory `knowledge/`. This name implies a static, general knowledge base. The directory's actual function is to provide **current, project-specific agent context** — a fundamentally different concept.

## Decision

Rename `knowledge/` to `context/`.

## Evaluation

**`knowledge/`** — Implies static, general information. Does not convey the live, current, project-specific nature of the content.

**`context/`** — Precisely describes the function: providing agent context. Aligns with the AI terminology of "context window" in a way that reinforces intent. Compact and unambiguous.

**`project/`** (alternative) — Too generic. Could be confused with the project source code. Rejected.

## Consequences

- All references to `knowledge/` updated throughout the repository.
- `.agentos/config.yml` and `manifest.yml` updated.
- README updated with new structure.
- CHANGELOG updated (MINOR version bump — backward-compatible, new projects can adapt).

## Trade-offs

Minor risk: "context" has a second meaning in AI (context window). This is judged to be low risk since the directory path `context/` is unambiguous in a filesystem setting.
