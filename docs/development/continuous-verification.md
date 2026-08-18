# Continuous Verification Protocol

> Binding on every milestone. Summarised as RULE 026 in
> [PROJECT_RULES.md](../../PROJECT_RULES.md); enforced by
> [QG-009](../../agentos/gates/QG-009-continuous-verification.md).

**Do not implement a large feature and verify only at the end.**

The protocol exists because of what happened at M04. Route matching was
implemented, tested against fixtures, and passed every gate — and then running
it over Flask's own repository showed a single catch-all route producing **138
false edges**. The fixtures were green the whole time. Synthetic tests prove a
subsystem does what you imagined; only real input proves what you imagined was
right.

---

## 1. The micro-implementation loop

For every meaningful subsystem:

```
PLAN → STATE THE INVARIANT → SMALLEST SLICE → COMPILE
     → TARGETED TESTS → INSPECT OUTPUT → NEGATIVE TESTS
     → NO REGRESSION → only then continue
```

Not this:

```
implement resolver, confidence, edges, CLI … then run tests
```

But this:

```
candidate generator → compile → tests → inspect → negatives → pass
compatibility       → compile → tests → inspect → negatives → pass
confidence          → ordering tests → inspect evidence     → pass
edge creation       → invariant tests → no edge on ambiguity → pass
CLI                 → integration tests → inspect output     → pass
```

A subsystem is stable enough for dependent work only when targeted tests pass,
negative tests pass, representative output was inspected, the regression suite
passes, and the relevant gates pass.

## 2. Invariant-first

Write down what a subsystem must never do **before** implementing it, and test
those statements. Cartograph's standing invariants:

- An ambiguous match never creates an accepted edge.
- An unknown method never becomes GET.
- An unsupported route never becomes a verified route.
- An unknown value never becomes a concrete value.
- A parser observation never silently becomes a semantic resolution.
- Confidence is never presented as calibrated while it is not.
- Evidence and provenance stay attached to every accepted edge.
- Source text never enters logs or diagnostics.
- Future-milestone capability never leaks into the current one.

## 3. Output inspection

Passing tests are insufficient when the output itself can be wrong. For
anything producing routes, candidates, matches, confidence, edges, canonical
paths, diagnostics or benchmark results, inspect by hand at least:

1. an obvious positive · 2. a boundary case · 3. an ambiguous case ·
4. a negative case · 5. a malformed or unsupported case

## 4. Adversarial testing

Before calling a subsystem complete, try to break it:

- How could this produce a false positive?
- How could it silently drop a valid case?
- How could two unrelated structures look identical?
- How could unknown information become a guessed value?
- How could one framework's syntax be mistaken for another's?
- How could a catch-all swallow unrelated inputs?
- How could a default create a false claim?

Every real defect found gets a regression test naming where it came from.

## 5. Real-repository validation

Mandatory for resolver and static-analysis milestones. Synthetic fixtures test
the cases you thought of; real repositories supply the ones you did not.

When an unexpected pattern appears, **stop implementing** and classify it: true
match · false positive · false negative · unsupported syntax · unresolved
ambiguity. Then fix it or represent the uncertainty explicitly. Never leave an
unexplained output unexplained.

Do not report accuracy percentages until a labelled benchmark exists
(CrossStack-Bench, M08).

## 6. Before/after counts

When a defect is found in an analyzer or resolver, record before, after and
cause:

> **Flask** — before: 167 accepted edges · after: 29 · cause: a bare catch-all
> route matched client calls from every other module.

That turns a debugging discovery into durable evidence.

## 7. Differential checking

Compare old against new behaviour across fixtures and real corpora when a rule
changes. Investigate large swings. Neither "more matches" nor "fewer matches"
is good in itself — analyse *which* relationships changed.

## 8. Property and invariant tests

For algorithmic components, prefer tests that state a property: normalisation
is deterministic, canonicalisation invents no parameters, method normalisation
is idempotent, serialisation round-trips, accepted edges always carry evidence.
Use the smallest technique that works; do not add a testing framework without
need.

## 9. Failure policy

If verification finds a real defect: **stop moving forward.** Do not patch
around it. Reproduce → isolate → root cause → regression test → fix → rerun
targeted tests → rerun the broader suite → inspect the corrected output →
continue.

## 10. High-risk milestones

M04 route matching · **M05 dynamic URL analysis** · M06 ORM resolution ·
M07 full-stack graph · M08 benchmark and calibration · M10 incremental
computation · M12 blast radius · M13 structural diff.

These require adversarial real-repository validation, and their self-review
must explicitly cover correctness, false positives, false negatives, ambiguity,
unsupported cases, security, performance, backward compatibility, scope
leakage, evidence quality and reproducibility. "Self-review clean" is not a
valid statement unless those categories were actually checked.

### M05 in particular

The dynamic URL problem is genuine research — constant propagation, abstract
interpretation, symbolic string analysis — and the specification is explicit
that a model must never be used to guess a value. Build it in this order, each
step verified before the next begins:

```
literal propagation → template propagation → module constants
  → concatenation → environment-variable unknowns → symbolic unknowns
  → partial URL reconstruction → confidence and evidence
```

Not one "dynamic URL resolver".

## 11. Reporting

Every milestone report carries a **Verification Findings** section: unexpected
behaviours, false positives, false negatives, regressions, fixes, regression
tests added, real-repository observations, unresolved limitations.

If nothing was found, state exactly what was tested to establish that. "None"
alone is not a finding.

## 12. Proportionality

This protects correctness; it is not paperwork. Use the smallest artifact that
proves the check happened. A small change may need only a targeted test and a
self-check. Resolver behaviour needs fixtures, adversarial tests and real
repository inspection.

## 13. The rule that decides ties

Never optimise for *finish the milestone quickly*. Optimise for *finish the
milestone with evidence that it is correct*. When correctness and speed
conflict, choose correctness.
