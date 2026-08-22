# M08 — confidence calibration and measurement

M07 accepted the product criterion and left three measurement gaps open:
`HttpCall` recall, `OrmAccess` recall and chain recall, all **UNMEASURED**.
M08 exists to close what can honestly be closed and to say precisely why the
rest cannot be.

## The finding that shapes this milestone

**Cartograph's confidence is discrete by construction.** It is not a learned
probability; it is an uncalibrated prior selected by evidence class. Across the
seven-repository corpus at `cartograph-m07`, 14,932 edges carry **seven**
distinct values:

| Confidence | Edges | Produced by |
|---|---|---|
| 0.98 | 680 | route matcher — exact method, every segment static and equal |
| 0.90 | 1,737 | static import resolution (1,443) · route matcher, parameter bound (294) |
| 0.80 | 11,810 | ORM resolution (11,754) · route matcher, variable aligned (56) |
| 0.784 | 416 | route matcher, undeclared-method factor applied |
| 0.72 | 63 | route matcher, wildcard suffix |
| 0.65 | 8 | route matcher, further weakened |
| 0.64 | 218 | route matcher, undeclared method + partial |

This has three consequences that the methodology must respect rather than
paper over:

1. **A ten-bin reliability diagram is mostly empty.** Six of ten bins contain
   no observations at all, and one bin holds 79% of the corpus at a single
   value. Reporting "the reliability curve" as though it were continuous would
   misrepresent the system.
2. **Calibration by confidence value is calibration by evidence class.**
   Because the value is a deterministic function of (provenance, match
   reason), grouping by value and grouping by evidence are the same
   partition. M08 reports the evidence-class view as primary, because that is
   what the number actually means.
3. **ECE is computable and reported, but it is an ECE over seven points**, and
   its per-bin terms inherit the sample sizes below. Every figure carries its
   denominator.

## What independence means here

No label comes from Cartograph. Every label is re-derived from repository
source by instruments that share no code with the analyser — the same
discipline M07 established, extended to per-edge labelling. The evaluator
refuses to score results whose corpus, ground-truth or supported-subset
digests do not match the ones recorded with them.
