# M08 — Golden fixtures, CrossStack-Bench, calibration

Target: Week 4 · Branch: `feature/m08-benchmark-calibration` · Tag on acceptance: `cartograph-m08`

Scope: golden fixture repos with expected graphs in CI; CrossStack-Bench (10–15
real repos, ~500 hand-labelled cross-language edges, scoring harness);
logistic-regression confidence calibration; reliability diagram + ECE.

Acceptance: precision/recall/F1 published with failures. HARD GATE 2: if
F1 < 0.70, publish the benchmark standalone and reassess — do not pretend the
resolver is solved.

Standard exit: PR + CI green + self-review + human acceptance + checkpoint tag + state update + STOP.
