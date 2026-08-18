# ADR-0008: Master Workflow as Orchestrator Reference Brain

> **Status:** Accepted | **Date:** 2026-07-02 | **Decider:** Principal Architect

---

## Context

The `workflows/master.md` file needs to serve two audiences:
1. Human engineers reading it to understand the process
2. The `orchestrator` agent reading it to route tasks correctly

These audiences have different needs. Engineers want narrative and rationale. The orchestrator needs precise rules and lookup tables.

## Problem

Should master.md be written for humans (narrative) or for the orchestrator (structured reference)?

## Alternatives Considered

**Option A: Narrative SOP (human-first)**
- Easy to read but hard for agents to parse
- Agents must infer rules from prose
- Rejected: introduces ambiguity in agent decisions

**Option B: Pure structured reference (agent-first)**
- Optimal for the orchestrator but alienates human engineers
- Rejected: humans need to understand and maintain this file

**Option C: Structured-first with narrative context**
- Accepted: tables and flowcharts for the orchestrator; brief prose for human context

## Decision

Design `workflows/master.md` as a **structured reference with human context**:
- Lifecycle as a numbered sequence of phases
- Agent trigger map as a lookup table
- Quality gate matrix as a lookup table
- Escalation as a decision tree (flowchart-style ASCII)
- Continuous improvement as a numbered procedure
- Anti-patterns table at the end

The orchestrator can reliably extract rules from the tables and flowcharts without parsing prose.

## Consequences

- The orchestrator has an authoritative, unambiguous routing reference
- Human engineers can navigate the lifecycle phases
- The trigger map and gate matrix are the single source of truth for routing and gating decisions
- Changes to routing require updating exactly one table

## Cross-References

The master workflow references — but does not duplicate — these files:
- `checklists/` for gate verification procedures
- `standards/` for quality definitions
- `context/state.md` for current project state
