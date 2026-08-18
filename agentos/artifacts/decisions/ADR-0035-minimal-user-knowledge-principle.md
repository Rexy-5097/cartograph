# ADR-0035: Minimal User Knowledge Principle

> **Status:** Accepted | **Date:** 2026-07-02 | **Decider:** Principal Architect

---

## Context

Teammates should not need to learn internal AgentOS workflows, state structures, or metrics files simply to configure a new application. However, the system still needs project-specific inputs.

## Decision

Enforce the **Minimal User Knowledge Principle**:
1. The developer is prompted only for project-specific information (Project Name, tech stack, goals, profile).
2. All framework-specific parameters (standards enabled, reviewer agents mapped, metrics tracked) are inferred automatically using profile files inside `profiles/`.

## Consequences

- Reduces developer onboarding time.
- Standardizes configuration settings across team projects.
