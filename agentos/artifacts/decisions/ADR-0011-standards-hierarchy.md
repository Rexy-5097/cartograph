# ADR-0011: Standards Hierarchy

> **Status:** Accepted | **Date:** 2026-07-02 | **Decider:** Principal Architect

---

## Context

AgentOS manages diverse project types: software engineering, AI/ML, scientific research, and UI development. If all standards are flat, it becomes hard for agents and humans to understand which standards apply when, leading to context bloat or missing rules.

## Problem

How do we organize the engineering standards to ensure they are modular, easy to navigate, and prevent duplication while scaling across different domains?

## Alternatives Considered

**Option A: Flat Standards File**
- One massive standards file covering everything.
- Rejected: Causes severe context window bloat and is hard for specialized agents to consume.

**Option B: Independent Domain Folders**
- Completely separate standards for each project type (e.g. all code quality, security, and testing rules rewritten for UI vs ML).
- Rejected: Leads to massive duplication and drift across domains.

**Option C: Four-Tier Dependency Hierarchy**
- Organized by specificity: Foundation -> Cross-cutting -> Domain -> Component.
- Accepted.

## Decision

Organize the standards into four distinct tiers:
1. **Tier 1 — Foundation (Universal):** `code_quality`, `testing`, `documentation`. Always apply.
2. **Tier 2 — Cross-cutting (Broad):** `security`. Appoints security as a design constraint across all components.
3. **Tier 3 — Domain (Project-specific):** `ai_ml`, `research`. Loaded based on the project type.
4. **Tier 4 — Component (Component-specific):** `api_design`, `data_engineering`, `ui_ux`. Loaded based on the component being modified.

Tiers inherit by reference from top to bottom.

## Consequences

- Reviewer agents load only the standard that matches the domain or component changed, keeping context minimal.
- Foundation standards are written once and referenced by lower tiers.
- Adding a new component (e.g. `mobile_app`) only requires creating one Tier 4 standard that inherits from Foundation.
