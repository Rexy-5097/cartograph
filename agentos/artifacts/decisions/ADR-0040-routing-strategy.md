# ADR-0040: Routing Strategy

> **Status:** Accepted | **Date:** 2026-07-02 | **Decider:** Principal Architect

---

## Context

Routing requests to agents based solely on vague semantic descriptions is unreliable and leads to incorrect reviewer mapping.

## Decision

Decouple routing rules into a YAML policy configuration (`runtime/policies/routing.yaml`) and implement a three-tiered routing precedence:
1. **File overrides:** Explicit mapping matching file names (e.g. `auth.py` -> `security-reviewer`).
2. **File extensions:** Default mappings matching extensions (e.g. `.py` -> `ai-reviewer`).
3. **Domain defaults:** Fallback maps based on domain categories (e.g. `science` -> `science-reviewer`).

## Consequences

- Zero semantic guessing is used for routing.
- Changing agent mappings is done by editing the policy file instead of code.
