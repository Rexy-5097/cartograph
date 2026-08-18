# ADR-0009: State File as Mandatory-Read Project Heartbeat

> **Status:** Accepted | **Date:** 2026-07-02 | **Decider:** Principal Architect

---

## Context

Before any agent begins work, it needs situational awareness: what is the project doing right now, what is blocked, what decisions were recently made?

## Problem

Without a compact, always-current state file, agents must read all context files to reconstruct current state — expensive and unreliable.

## Decision

`context/state.md` is the **mandatory first read** for every agent, every session. It is the project heartbeat.

Design constraints:
1. **~400 token budget** — strict. Must remain small enough that mandatory loading is cheap.
2. **Updated every session** — within 5 minutes of starting or ending work.
3. **Status tags** instead of prose (`IN_PROGRESS`, `BLOCKED`, `DONE`)
4. **No duplication** with other context files — only current state
5. **Recent decisions** — last 3 only (full index in `context/decisions.md`)

## Consequences

- Every agent starts each session with current project awareness at minimal token cost
- Stale `state.md` is a process failure — it must be maintained
- Engineers must update `state.md` as part of their session routine
- The ~400 token budget is a hard constraint — exceeding it requires archiving completed tasks

## Anti-patterns This Prevents

- Agents working on the wrong priority because state was unknown
- Agents unaware of blocking issues
- Agents making decisions already resolved in recent ADRs
