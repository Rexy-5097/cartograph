# ADR-0046: Reflection Strategy

> **Status:** Accepted | **Date:** 2026-07-02 | **Decider:** Principal Architect

---

## Context

Simply repeating execution on failure is inefficient. We need a structured mechanism to analyze failures and direct next-step optimizations.

## Decision

Implement the **Reflection Engine** under `runtime/loop/reflection.py`:
1. Parse error logs and build diagnostic outputs.
2. Categorize failure root causes (e.g. syntax, imports, assertions).
3. Generate structured reflection reports recommending target strategies.

## Consequences

- Prevents repeating previous code compilation mistakes.
- Speeds up pipeline convergence toward compliance targets.
