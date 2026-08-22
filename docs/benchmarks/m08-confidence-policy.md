# Confidence policy (specification, not an implementation)

A semantic specification for later UI and API work, derived from the M08
measurements. **No production threshold changes on the strength of this
document.** Changing one is an architecture decision and needs its own ADR.

## What a confidence number currently means

It is an **uncalibrated prior selected by evidence class**, not a probability.
Seven values exist. Grouping edges by confidence and grouping them by evidence
are the same partition, which is why the policy below is written in terms of
evidence rather than numbers.

## Categories

| Category | Evidence | Measured basis |
|---|---|---|
| **HIGH** — safe to state plainly | Route matched on an exact method with every path segment static and equal (0.98); a name resolved to its declaration through an import (0.90) | 544 and 1,039 verified observations, no false positive in either |
| **MEDIUM** — state with visible uncertainty | Model-to-table and handler-to-model resolution (0.80) | 9,630 verified, one false positive |
| **LOW** — state as inference, never as fact | A route matched with an undeclared method, a wildcard suffix, or a client value the source does not fix (0.64–0.784) | **707 produced, zero verified.** Accuracy here is unmeasured |
| **REFUSED** — no edge | Ambiguity, unresolvable imports, dynamic prefixes, non-literal table names | 1,544 ambiguous decisions produced no edge |

## The rule this measurement suggests

**Do not raise confidence to match observed accuracy.** Observed accuracy is
~1.0 in every measured band, which would argue for pushing 0.80 toward 1.0.
That would be fitting to a filtered subset: a quarter of all edges could not be
verified, and the unverifiable ones are concentrated exactly where the evidence
is weakest. The measured gap is real, but the population it was measured on is
not the population the number is applied to.

**Do not lower the acceptance threshold below 0.8.** The threshold table shows
nothing to gain: every band from 0.0 to 0.8 retains the same single false
positive and the same true positives, because 0.784 and below produce edges
that were never verifiable either way.

**Treat the LOW band as display-only.** It is 4.7% of edges and has no measured
accuracy. Until it does, it should not feed anything automatic.

## What would change this

Measuring the low band needs labels for edges whose client value is not fixed
in source — which is, by construction, what makes them unverifiable to a static
reader. That is a genuine open problem, recorded rather than assumed away.
