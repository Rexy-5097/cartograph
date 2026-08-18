# ADR-0043: Cost Optimization

> **Status:** Accepted | **Date:** 2026-07-02 | **Decider:** Principal Architect

---

## Context

Running redundant checks or allowing failing pipelines to continue wastes tokens and budget.

## Decision

Implement **Cost Optimization Rules** in the Harness kernel:
1. **Decision Cache:** Cache compiled execution plans for duplicate task strings.
2. **Early Stopping:** Terminate execution immediately if any mandatory Quality Gate validation fails, preventing subsequent dispatches.
3. **Context Budgets:** Warn or halt if the estimated token count of loaded files exceeds threshold budgets.

## Consequences

- Prevents unnecessary LLM API calls.
- Keeps pipeline iterations extremely fast and cost-effective.
