# ADR-0007 — No language model constructs graph structure. Ever.

**Status:** Accepted · 2026-08-18 · Enforces RULES 007–008 · **Immutable**

## Context

Cartograph's entire claim is that its edges are derived and provable. A single
model-proposed edge poisons that claim for every edge, because users cannot
tell which edges to trust. "Model inference" in the confidence ladder refers to
a logistic-regression classifier over structural features (M08) — a
deterministic, auditable statistical model — never a language model.

## Decision

No language model may propose, construct, modify, or fill gaps in graph
structure — not as a fallback, not as a hint, not behind a flag. The pipeline
is: deterministic analysis first, evidence second, statistical inference third,
the model last **and never in the graph**. A language model may explain
already-derived subgraph evidence (ASK, M16), where it interprets and cites
but cannot add.

## Alternatives

- **LLM-assisted edge recovery for unresolvable URLs** — rejected: an edge
  that cannot cite its derivation is a guess wearing a confidence score.
  Unknown stays unknown (RULE 009).

## Consequences

Some real relationships will remain unresolved and the product must be honest
about them — that honesty is the moat. ASK's design (M16) must pass evidence
*to* the model, never accept structure *from* it.
