# ADR-0031: Artifact Lifecycles and Archival

> **Status:** Accepted | **Date:** 2026-07-02 | **Decider:** Principal Architect

---

## Context

If all artifacts grow indefinitely, repository size increases, and agents consume too many tokens. We need clear lifetimes for each document category.

## Decision

Define explicit **Archival Policies** per artifact type:
- **ADRs & Releases:** Permanent historical records (never delete/archive).
- **Experiment Logs:** Retain, but compress historical runs at milestone closure.
- **Bug Reports:** Retire once bug status resolves.
- **Meeting Notes:** Archive after 6 months to keep context clean.
- **Sprint Plans:** Archive immediately after sprint closure.

## Consequences

- Keeps the active directory clean and focused.
- Prevents context window bloat during routine operations.
