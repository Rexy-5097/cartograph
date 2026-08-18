# ADR-0030: Verifiable Document Rules

> **Status:** Accepted | **Date:** 2026-07-02 | **Decider:** Principal Architect

---

## Context

Simply having metadata fields is not enough; we need to verify that the *content* sections are valid and not just empty placeholder strings like `[FILL: description]`.

## Decision

Implement **Verifiable Document Rules** in the static validator:
1. Parse the Frontmatter of all markdown files recursively.
2. Scan for placeholder patterns (e.g. `[FILL: ...]` or wildcards) and reject the build if any are found in active status files.
3. Verify that all referenced file links are valid relative or absolute paths.

## Consequences

- Prevents incomplete or draft files from slipping into releases.
- Keeps documentation accurate and fully resolved.
