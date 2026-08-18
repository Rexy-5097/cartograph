# ADR-0032: Automated Artifact Auditing

> **Status:** Accepted | **Date:** 2026-07-02 | **Decider:** Principal Architect

---

## Context

Validating schemas manually is tedious and error-prone. We need continuous enforcement of our metadata contracts.

## Decision

Integrate artifact auditing directly into the static validator (`validate_agentos.py`). The validator will recursively scan all markdown files, parse frontmatter, check keys, trace links, and output reports. The validator will fail builds on schema violations.

## Consequences

- Compliance is checked automatically during CI/CD.
- Zero manual audit effort is required to verify repository health.
