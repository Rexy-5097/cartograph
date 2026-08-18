# ADR-0010: context/workflow.md as Navigation, Not Summary

> **Status:** Accepted | **Date:** 2026-07-02 | **Decider:** Principal Architect

---

## Context

`context/workflow.md` was originally described as "a condensed operational workflow" that summarizes `workflows/master.md`. This design creates a duplication problem: whenever `workflows/master.md` is updated, `context/workflow.md` must also be updated or it drifts.

Duplication drift is one of the most common ways documentation systems fail — summaries become stale and misleading.

## Problem

Should `context/workflow.md` be a summary of the master workflow, or something different?

## Decision

`context/workflow.md` is a **navigation and project customization document**, NOT a summary.

It records:
1. **Which workflows this project uses** (not all projects use all workflows)
2. **Project-specific deviations** from `workflows/master.md` (with justification)
3. **Sprint cadence** and project operating model
4. **Work intake** process for this project
5. **Agent trigger map customizations** (if any)

It does NOT summarize `workflows/master.md`. To understand the full workflow, read `workflows/master.md`.

## Consequences

- Eliminates a duplication relationship that would drift over time
- `context/workflow.md` is now genuinely project-specific (different for every project)
- Agents that need the full workflow read `workflows/master.md`
- Agents that need to know "how does THIS project operate" read `context/workflow.md`
- The two files serve distinct purposes with zero overlap

## Trade-offs

Minor: some context is now split across two files. The mitigation: `context/workflow.md` explicitly references `workflows/master.md` rather than trying to contain the information.
