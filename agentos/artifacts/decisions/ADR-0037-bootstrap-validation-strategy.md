# ADR-0037: Bootstrap Validation Strategy

> **Status:** Accepted | **Date:** 2026-07-02 | **Decider:** Principal Architect

---

## Context

To ensure the bootstrap files themselves do not drift, we need to continuously check them inside our validation pipelines.

## Decision

Extend the static validation suite (`validate_agentos.py`) with a **Bootstrap Validation** category:
1. Verify presence of `BOOTSTRAP.md`, `START_PROJECT.md`, and `tools/scripts/bootstrap_project.py`.
2. Parse `PROJECT_CONFIG.yaml` if it exists, verifying that the configured profile matches.
3. Validate that standard context files (`context/vision.md` and `context/state.md`) are populated and trace back to correct profiles.

## Consequences

- Prevents broken initialization scripts from being released.
- Ensures consistent configuration tracking across the workspace.
