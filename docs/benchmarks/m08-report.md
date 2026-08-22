# M08 — confidence calibration and measurement

**Status: INTERNAL.** Not a published benchmark. Every figure carries its
denominator and its sample size, and the largest limitation is stated before
the results rather than after them.

Base checkpoint `cartograph-m07` (`2514f77`). Same seven repositories, same
pinned commits, same supported subset, same ground-truth methodology. The M07
reports are unchanged.

---

## The limitation that governs every number below

**A quarter of all produced edges could not be verified from source, and they
are not a random quarter.**

| | |
|---|---|
| Edges produced and labelled | 14,932 |
| Verified (true or false) | **11,221** |
| Unverifiable | **3,711 (24.9%)** |

An edge is unverifiable when the independent reader cannot decide it: a name
bound by a star import, a client URL the source does not fix, a re-export chain
it does not follow. Those are precisely the situations where a resolver is most
likely to be wrong. Every accuracy figure here is therefore measured on the
**more tractable three quarters** of the corpus, and is an upper bound on
accuracy over all of it.

This is stated first because it is the finding most easily lost behind the
numbers that follow.

---

## What confidence currently is

Not a probability. An **uncalibrated prior selected by evidence class**. Seven
distinct values exist across 14,932 edges:

| Confidence | Produced | Verified | Observed accuracy | 95% CI | Sample |
|---|---|---|---|---|---|
| 0.98 | 680 | 544 | 1.0000 | [0.993, 1.000] | adequate |
| 0.90 | 1,737 | 1,039 | 1.0000 | [0.996, 1.000] | adequate |
| 0.80 | 11,810 | 9,630 | 0.9999 | [0.999, 1.000] | adequate |
| 0.784 | 416 | **0** | — | — | **none** |
| 0.72 | 63 | **0** | — | — | **none** |
| 0.65 | 8 | 8 | 1.0000 | [0.676, 1.000] | too small |
| 0.64 | 218 | **0** | — | — | **none** |

Because the value is a deterministic function of (provenance, match reason),
calibration *by confidence* and calibration *by evidence* are the same
partition. A ten-bin reliability diagram leaves six bins empty; it is recorded
in `reliability_bins` for comparability and is not the primary view.

### Calibration error

| | ECE | MCE | Verified | Excluded |
|---|---|---|---|---|
| Development | 0.1819 | 0.35 | 8,391 | 24.8% |
| **Holdout** | **0.1825** | **0.35** | 2,830 | 25.1% |
| All | 0.1820 | 0.35 | 11,221 | 24.9% |

**The error is under-confidence, not over-confidence.** Observed accuracy is
~1.0 where the system claims 0.80–0.98. Development and holdout agree to within
0.0006; since no tuning was performed, that demonstrates stability, not the
validation of a fitted policy.

**Where the system is overconfident: not measurable.** The three bands that
would reveal overconfidence — 0.64, 0.72, 0.784 — have **zero** verified
observations between them. That is not an oversight; those edges are weak
precisely because the source does not fix the values a reader would need.

---

## By relationship class

| Class | Produced | Verified | Unverifiable | Accuracy | 95% CI | ECE |
|---|---|---|---|---|---|---|
| `queries` | 290 | 290 | 0% | 1.0000 | [0.987, 1.000] | 0.200 |
| `http-call` | 1,735 | 881 | 49.2% | 1.0000 | [0.996, 1.000] | 0.059 |
| `orm-access` | 11,464 | 9,284 | 19.0% | 0.9999 | [0.999, 1.000] | 0.200 |
| `call` | 1,443 | 766 | 46.9% | 1.0000 | [0.995, 1.000] | 0.100 |

## By provenance

| Provenance | Produced | Verified | Accuracy | ECE |
|---|---|---|---|---|
| `route-matcher` | 1,735 | 881 | 1.0000 | 0.059 |
| `static-import-resolution` | 1,443 | 766 | 1.0000 | 0.100 |
| `orm-resolution` | 11,754 | 9,574 | 0.9999 | 0.200 |

`route-matcher` is the best-calibrated source because it is the only one that
varies its confidence with the evidence it found. The other two emit a single
value regardless, so their ECE is just the distance from that value to 1.0.

---

## Recall — the gaps M07 left open

M07 reported these as UNMEASURED. Each is now measured **completely inside a
stated scope**. None is recall over the corpus, and none should be quoted as
such.

| Class | Scope | Denominator | Recovered | Recall |
|---|---|---|---|---|
| `HttpCall` | Airflow's generated client: every operation it declares, that a route serves | 129 | 117 | **0.907** |
| `OrmAccess` | Airflow: import-bound `Model(...)` and `Model.objects.…` uses of models with a declared table | 1,883 | 1,401 | **0.744** |
| **End-to-end chain** | Airflow: chains whose client operation, route, handler and model are all independently readable | 5 | 3 | **0.600** |

The chain denominator is 5. **That is a very small sample** and cannot support
a stable estimate; it is reported as "3 of 5", not as 60%.

Misses are real. `DELETE /api/v2/pools/{pool_name}` is declared
`@pools_router.delete("/{pool_name:path}")` — a path converter that consumes
multiple segments, so a single-segment client does not align. Both missed
chains (`patch_pool`, `get_pools`) are of that shape.

---

## Precision, and the one false positive

**1 false positive in 11,221 verified edges.**

```
posthog  products/batch_exports/.../test_utils.py:29    Element(...)
```

PostHog declares `class Element(models.Model)` in `posthog/models/element/element.py`.
The test declares a **function-local** `class Element: pass` and constructs it.
Cartograph treats only module-scope declarations as shadowing, and no import
binds the name, so the name falls through to the permissive branch and the
access is attributed to the ORM model.

It is left unfixed deliberately: M08 is a measurement milestone, and the
measurement is more honest with it counted than with it patched out. Recorded
as future work FW-12.

---

## Integrity and adversarial testing

Results are bound to the digests of the corpus, the supported subset, the
measurement, the labeller, **and the labelled records themselves**.

- **16/16 integrity checks pass.**
- **14/14 attacks on the evaluation pipeline detected.** The first run caught
  ten: deleting false positives, relabelling one, editing confidences and
  leaking the holdout all went unnoticed, because the digests covered the
  dataset's *inputs* and not its *records*. A record digest closed all four.
- **8/8 attacks on the QG-009 extension detected.** The first run caught none —
  the new check was written but never called, and a string replacement had
  silently failed to wire it in. Worth recording: a gate that is not invoked
  passes every test you give it.

---

## Statistical honesty

Proportions use Wilson score intervals; a normal approximation on 290/290 would
report [1.0, 1.0] and assert a certainty no sample of 290 supports. Any group
with fewer than 30 verified observations is flagged `sample_adequate: false`
and its accuracy is reported as a count, not an estimate. Groups with zero
verified observations report no accuracy at all, and the gate fails if one ever
does.

## Performance

Labelling 14,932 edges across seven repositories takes ~4 minutes, dominated by
reading 58,225 source files. Calibration itself is milliseconds. Analysis
performance is unchanged from M07 (58,225 files, ~100 s, peak RSS 860 MB) and
is recorded as a historical baseline, not a target.

## Security

No source upload, no model, no database, no environment reads, no repository
execution, no network. Evidence records paths, lines and structural facts; the
dataset stores no source text. Verified by QG-005.

## Dependencies

**None added.** Wilson intervals, binning and ECE need only `math`; a plotting
library would produce nothing the tables do not already say.

## Known limitations

1. **24.9% of edges are unverifiable**, concentrated where evidence is weakest.
2. **The low-confidence bands are entirely unmeasured** — 707 edges, no verified observations.
3. **Recall is scope-bounded**, one repository and one client family.
4. **The chain denominator is 5.**
5. Confidence remains discrete and unlearned; nothing here changes a threshold.
6. One known false positive, unfixed by choice.
