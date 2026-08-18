# ADR-0036: Project Initialization Workflow

> **Status:** Accepted | **Date:** 2026-07-02 | **Decider:** Principal Architect

---

## Context

A project setup can halt due to network errors, permission locks, or missing dependencies. We need a recovery strategy to resume setups instead of overwriting existing progress.

## Decision

Standardize the **Initialization Workflow** in `bootstrap_project.py`:
1. Validate baseline structure before writing files.
2. If files like `context/vision.md` exist, merge them or warn instead of overwriting.
3. Save settings to `PROJECT_CONFIG.yaml` as the source of truth.
4. Support the `--resume` flag to pick up where setup halted.

## Consequences

- Avoids data loss during re-initialization.
- Enables CI/CD pipelines to run non-interactively using `--defaults` or `--config`.
