# ADR-0049: Quality Thresholds

> **Status:** Accepted | **Date:** 2026-07-02 | **Decider:** Principal Architect

---

## Context

We need a standard scoring system to map iteration quality rather than relying on qualitative logs.

## Decision

Decouple quality score calculation into a dedicated **Quality Evaluator** (`runtime/loop/quality_evaluator.py`) reading thresholds from `runtime/policies/quality_thresholds.yaml` (Minimum: 70, Recommended: 85, Flagship: 95, Production: 100).

## Consequences

- Scoring outputs are uniform and mathematically defined.
- Termination checks are separated from score logic.
