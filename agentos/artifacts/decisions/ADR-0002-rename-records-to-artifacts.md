# ADR-0002: Rename `records/` to `artifacts/`

> **Status:** Accepted | **Date:** 2026-07-02 | **Decider:** Principal Architect

---

## Context

The initial design named the generated project artifact directory `records/`. An alternative `artifacts/` was proposed. A third option `history/` was also considered.

## Decision

Rename `records/` to `artifacts/`.

## Evaluation

**`records/`** — Neutral and accurate, but carries a bureaucratic connotation. Does not clearly convey that this directory holds generated project outputs.

**`artifacts/`** — Accepted. Precisely describes the content (generated engineering artifacts: ADRs, experiment logs, release notes, post-mortems). Widely understood in engineering contexts. The potential confusion with CI/CD build artifacts is judged to be minimal because this system is documentation-first, not build-output-first.

**`history/`** — Implies read-only immutable archive. This is partially true but undersells the structured nature of the content. Rejected.

## Consequences

- `records/` renamed to `artifacts/` throughout the repository.
- All cross-references updated.
- Naming convention definitions preserved (ADR-NNNN, EXP-YYYY-MM-DD, etc.).

## Trade-offs

Low risk of confusion with CI/CD `artifacts/` — mitigated by the clear subdirectory structure (decisions/, experiments/, incidents/, releases/).
