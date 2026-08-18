# ADR-0013: Metrics Philosophy

> **Status:** Accepted | **Date:** 2026-07-02 | **Decider:** Principal Architect

---

## Context

Subjective code reviews ("this looks clean," "this feels fast") are inconsistent, dependent on the reviewer's mood or the agent's prompt, and difficult to automate. High-performance software engineering requires objective, reproducible measurements.

## Problem

How do we eliminate subjective bias in AgentOS quality gates while keeping the evaluation loop fast and automated?

## Decision

Adopt the **Metrics-First Philosophy**:
1. Every standard is paired with a metrics file (`performance.md`, `quality.md`, `benchmarks.md`).
2. Subjective terms are replaced by measurable metrics (e.g. instead of "fast API," target is "P50 latency < 100ms").
3. Trends matter as much as snapshots: a metric that degrades for 3 sprints in a row constitutes a quality gate warning, even if the absolute value is currently inside the acceptable range.
4. Metrics define the *targets*; standards define the *best practices*; checklists *verify* the metrics.

## Consequences

- Reviewer agents evaluate PRs by comparing test/benchmark results against the metrics files.
- Human review is reserved for design intent and architectural decisions, not syntax or performance budgets.
- All metrics have an owner, measurement method, and collection frequency defined.
