# ADR-0003: Add Orchestrator Agent

> **Status:** Accepted | **Date:** 2026-07-02 | **Decider:** Principal Architect

---

## Context

The initial design gave the Chief Architect two unrelated responsibilities:
1. Routing tasks to the correct specialist reviewer
2. Making final architectural decisions and resolving conflicts

These are different concerns. Mixing them overloads the Chief Architect and violates the Separation of Concerns principle.

## Decision

Add an `orchestrator` agent as a distinct coordination layer between the human engineer and the specialist reviewers.

## Architecture

```
Human Engineer
      ↓
 Orchestrator          ← NEW: routing, bundling, merge
      ↓
Chief Architect        ← unchanged: decisions and conflict resolution
      ↓
Specialist Reviewers
```

## Responsibilities of Orchestrator

- Determine which reviewers are needed based on what changed
- Prevent redundant reviews
- Bundle parallel reviews when multiple domains are touched
- Merge review outputs
- Minimize token usage by routing precisely
- Escalate to Chief Architect only on genuine conflict or architectural ambiguity

## Responsibilities of Chief Architect (clarified)

- Architectural decision-making
- Conflict resolution between reviewers
- Final authority on design direction
- ADR creation for significant decisions

## Naming

`orchestrator` — not `orchestrator-reviewer` because orchestration is not a review function.

## Consequences

- `agents/orchestrator.md` added.
- `agents/README.md` updated with new agent roster and routing diagram.
- `.agentos/config.yml` trigger map updated.
