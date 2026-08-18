# ADR-0034: One-Prompt Initialization Protocol

> **Status:** Accepted | **Date:** 2026-07-02 | **Decider:** Principal Architect

---

## Context

Different AI coding assistants (Claude Code, Antigravity, Gemini) have varying interface constraints. We need a single, portable prompt that works reliably across all target vendor models.

## Decision

Define the **AI Entry Specification** in `START_PROJECT.md`:
1. Use a structured blueprint containing a markdown context guide and vendor compatibility sub-notes.
2. The core setup prompt uses clear, step-by-step imperative directives.
3. Models use their respective local executors to run verification commands.

## Consequences

- Prevents prompt syntax drift.
- Supports future AI tool integrations via modular compatibility files.
