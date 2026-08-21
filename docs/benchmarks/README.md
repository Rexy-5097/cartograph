# Benchmarks

**Nothing is published.** M07 measured the engine against seven real
repositories; those numbers are INTERNAL, their methodology is
first-generation, and none of them may be quoted outside the project. See
[m07-report.md](m07-report.md). This page defines the standard that has to be
met before any number leaves the repository.

## Publishing standard

A number appears in public material only if it was produced by a benchmark run
on this codebase, with the harness and inputs available. Failures are published
alongside successes. "87.2% precision, 81.4% recall across ten repositories,
ECE 0.04" is the *shape* of claim this project makes — with real values or not
at all.

## Performance targets (targets, not results)

| Scenario | v1 | Later |
|---|---|---|
| 10k files, initial analysis | < 60 s | < 10 s warm |
| Normal file change, incremental | < 250 ms | < 100 ms |
| Rendering | 10k nodes @ 60 FPS | 50k / 100k / 200k benchmarked |

Criterion harness exists from M00 (`cargo bench`, graph construction +
traversal) so M10 has a baseline; its numbers are internal until methodology is
documented.

## What M07 measured

Seven pinned repositories, 58,225 files, ground truth authored from source by
an instrument with no access to analyser output, two full passes, and eleven
attacks on the benchmark's own machinery. Route extraction and model-to-table
resolution reached precision 1.000 and recall 1.000 on their in-scope ground
truth; six chains were verified end to end and none began at a frontend file.
`HttpCall` recall, `OrmAccess` recall and chain recall are **unmeasured**, which
is stated in the report rather than left to be inferred from their absence.

Harness: `benchmarks/run_benchmark.py` measures, `benchmarks/evaluate.py`
scores, and the two never share a judgement — a change to one cannot alter what
the other recorded.

## CrossStack-Bench (M08)

10–15 real full-stack repositories, ~500 manually labelled cross-language
edges, a scoring harness, a public leaderboard, failure cases published.
Metrics: precision, recall, F1, expected calibration error, reliability
diagrams. Golden fixtures (small synthetic repos with expected graphs) run in
CI to catch regressions; real repositories earn credibility. The project needs
both.

**Hard gate:** if F1 < 0.70 at M08, the benchmark is published standalone and
the research direction is reassessed — the resolver is not declared solved.

## Calibration

Per candidate edge, a feature vector: exact path match, method match, template
arity, handler-name similarity, parameter overlap, git co-change, directory
proximity, OpenAPI confirmation. Logistic regression against the labelled set.
An edge marked 0.8 must be correct 80% of the time — a testable claim, tested.
