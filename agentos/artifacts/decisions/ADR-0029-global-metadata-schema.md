# ADR-0029: Global Metadata Schema

> **Status:** Accepted | **Date:** 2026-07-02 | **Decider:** Principal Architect

---

## Context

To trace components across code, standards, metrics, decisions, and workflows, we need a unified metadata block present on all documents.

## Decision

Every AgentOS artifact must include a YAML Frontmatter block containing exactly 12 mandatory keys:
- `id`: Unique identifier (e.g. `ART-ADR-0001`).
- `title`: Short descriptive title.
- `version`: Version number.
- `status`: Lifecycle state (e.g., active, draft, superseded).
- `owner`: Responsible agent/role.
- `created` & `modified`: Timestamps.
- Tracing keys: `related_adr`, `related_standard`, `related_checklist`, `related_workflow`, `related_agent`.

## Consequences

- Scripts can automatically build dependency graphs of documents.
- Enables validation checks to flag orphan or untraced artifacts.
