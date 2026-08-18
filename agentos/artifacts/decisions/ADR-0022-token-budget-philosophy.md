# ADR-0022: Token Budget Philosophy

> **Status:** Accepted | **Date:** 2026-07-02 | **Decider:** Principal Architect

---

## Context

Uncontrolled context growth makes agent runs slower, more expensive, and less focused. We need structural limits to keep token usage within strict boundaries.

## Decision

Every agent specification file must declare and adhere to **Context and Output Token Budgets**:
1. **Core Reviewers:** Context budget < 2500 tokens; Output budget < 800 tokens.
2. **Orchestrator:** Context budget < 2000 tokens; Output budget < 800 tokens.
3. **Maximum Limit:** Under no circumstances should an agent session exceed 4000 tokens.
4. **Archiving:** If files grow beyond budgets, the agent must trigger archiving rules to move historical data to `artifacts/` snapshots.

## Consequences

- Agent execution remains fast and cost-effective.
- Force design discipline where developers archive stale records instead of appending to live context files.
