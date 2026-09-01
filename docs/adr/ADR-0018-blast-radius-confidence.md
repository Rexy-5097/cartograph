# ADR-0018 — Blast radius confidence: weakest link along a path, best route to a node

**Status:** Accepted · 2026-09-01 · Governs M12 · Constrained by
[ADR-0004](ADR-0004-evidence-first-class.md) and M08's calibration measurement

## Context

M12's scope line asks for "reverse reachability across language boundaries with
**confidence-weighted paths**". A blast radius therefore has to answer not only
*which* artefacts are affected but *how well supported* each claim of impact is.

Every edge already carries a `Confidence`. The question is how to combine them
along a path, and how to combine several paths reaching the same node.

The obvious answer is to multiply, as one would with independent
probabilities. **That answer is wrong here, and wrong in a way that matters.**

M08 measured what Cartograph's confidence actually is: an **uncalibrated prior
selected by evidence class**, not a probability. ECE 0.1819 development and
0.1825 holdout, MCE 0.35, seven distinct values across 14,932 edges with 79% at
exactly 0.80 — and the low bands have **zero verified observations**, so
overconfidence could not be measured at all.

Multiplying such numbers does two damaging things:

1. **It asserts a probabilistic semantics the values do not have.** The product
   of two priors is only a joint probability if both are probabilities and the
   events are independent. Neither holds.
2. **It makes depth look like doubt.** With 79% of edges at 0.80, a five-hop
   cross-stack chain would score 0.80⁵ ≈ 0.33 — not because anything about it
   is weakly evidenced, but because it is long. The chain from a React
   component to a database table is exactly what this product exists to show,
   and multiplication would rank it as the least trustworthy thing on screen.

## Decision

**Path confidence is the minimum confidence along that path.**

```
confidence(path) = min(confidence(e) for e in path)
```

**A node's confidence is the maximum over all paths that reach it.**

```
confidence(node) = max(confidence(p) for p in paths(node → target))
```

The two rules say something a reader can check:

- the minimum says *this path is no better supported than its weakest step* —
  a chain through one guessed-at edge is exactly as trustworthy as that guess;
- the maximum says *this artefact is reached by its best-supported route* — a
  second, weaker path to the same place does not make the first one worse.

Worked, from the verified airflow chain:

```
postConnection  --HttpCall  0.980-->  post_connection  --OrmAccess 0.800-->  Connection
```

`postConnection` reaches `Connection` at **0.800**, not 0.784. If a second
route reached it at 0.900, the node would be reported at 0.900.

This is the **bottleneck (widest-path) problem**, and `min`/`max` are chosen
because they are the operations that keep the output in the same space as the
input: every number a blast radius reports is a confidence value that already
appears on some edge in the graph. No new value is synthesised, so nothing is
implied that the analysis did not observe.

**The result is not a calibrated probability and must never be presented as
one.** It is the same uncalibrated prior M08 measured, propagated by a rule
that does not pretend otherwise. Any surface showing it carries the same
caveat the evidence panel carries (ADR-0017).

## Alternatives

- **Product of edge confidences.** Rejected: asserts independence and
  probability semantics the values do not have, and penalises path length,
  which would systematically bury the cross-stack chains the product exists to
  surface.
- **Arithmetic or geometric mean.** Rejected: a mean lets one strong edge
  compensate for a guessed-at one. Impact analysis is a chain — a claim is only
  as good as its weakest link, and averaging hides exactly the step a reviewer
  needs to look at.
- **Count hops instead of confidence.** Rejected: it discards evidence quality
  entirely and M12's scope line explicitly asks for confidence-weighted paths.
- **Report the full path distribution per node.** Rejected for the core slice:
  it is a presentation decision, and the core should not grow a shape the
  specification does not require. The reaching edge is retained, so a later
  surface can show the route without the core deciding how.

## Consequences

- Blast radius output is **closed over the input**: every confidence reported
  is one that appears on an edge. That is checkable, and a test asserts it.
- `min`/`max` are monotone, so the computation is a bottleneck shortest-path
  and a Dijkstra-style relaxation is correct — no fixpoint iteration needed.
- **This is a semantic policy, not a performance choice.** It was not selected
  because it is cheaper to compute, and it must not be traded away for speed.
- The rule composes if edge kinds are ever weighted differently: a per-kind
  ceiling would still be a `min` step and the algebra would not change.
- Deep chains are no longer penalised for depth, so a long cross-stack path and
  a short one carrying the same weakest edge are reported alike. That is the
  intended behaviour and the reason depth is reported separately.
