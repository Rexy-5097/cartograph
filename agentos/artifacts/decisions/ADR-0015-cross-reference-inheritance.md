# ADR-0015: Cross-Reference and Inheritance Strategy

> **Status:** Accepted | **Date:** 2026-07-02 | **Decider:** Principal Architect

---

## Context

As the number of standards grows, the risk of rule duplication increases. For example, security concerns apply to database design, API design, and AI/ML serving. If security rules are written in all four files, updating a security policy requires updating four separate documents, leading to drift.

## Problem

How do we ensure that lower-level, specialized standards can incorporate security and code quality rules without repeating them?

## Decision

Enforce a strict **Inheritance by Reference** strategy:
1. Higher-level standards (Tiers 1 & 2) contain the canonical rules.
2. Lower-level standards (Tiers 3 & 4) must reference the parent file's standard path (e.g. `[standards/security.md](../../standards/security.md)`) for cross-cutting concerns.
3. Lower-level files must restrict themselves strictly to the *additional* rules unique to their domain/component.
4. No copy-pasting of parent standards is allowed.

## Consequences

- Maintainability is preserved: a security policy update is made in exactly one file (`standards/security.md`) and instantly applies to APIs, databases, and AI models.
- Context window size is minimized during multi-domain reviews since the agent doesn't read duplicate instructions.
