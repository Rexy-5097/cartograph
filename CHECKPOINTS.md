# Checkpoints

Every milestone produces a reproducible checkpoint: an annotated tag, a gate
result and a rollback point. Policy:
[ADR-0010](docs/adr/ADR-0010-checkpoint-tag-policy.md).

## Checkpoint policy

| Class | Tag | Created when | Points at | Guarantees |
|---|---|---|---|---|
| Provisional | `cartograph-mNN-rcK` | Gates pass, PR (re)submitted | Exact PR head | Superseded by `rcK+1` on review changes; retired only while unaccepted |
| Accepted | `cartograph-mNN` | Human owner accepts (merges) the PR | Exact accepted commit on `main` | **Never moved or deleted**; final rc also retained |

**The tag is the authority for its SHA.** Ledger entries reference checkpoints
by tag name; a quoted SHA is informational and `git rev-parse <tag>` wins.
(Recording a tag's SHA in a committed ledger always produces a commit outside
the tag — the defect that ADR-0010 repairs.)

### Acceptance procedure (per milestone)

1. Human owner reviews and merges the milestone PR.
2. Tag the merge commit: `git tag -a cartograph-mNN -m "Accepted checkpoint MNN" <merge-sha> && git push origin cartograph-mNN`
3. Flip the milestone to ACCEPTED in this file and in
   `agentos/artifacts/project-state.yaml` (next milestone unlocks).

Steps 2 and 3 are **acceptance bookkeeping**, not development: they record the
outcome of a review that has already happened, on a commit that already exists.
They are performed directly on `main` and are the single narrow carve-out to
RULE 020 — which exists to stop *unreviewed implementation* reaching `main`, not
to stop `main` from stating the truth about itself. A bookkeeping commit may
touch only this file, `agentos/artifacts/project-state.yaml` and
`agentos/context/state.md`; anything else belongs in a milestone PR.

## Rolling back

```bash
git checkout cartograph-mNN         # accepted checkpoint
git checkout cartograph-mNN-rcK     # provisional state under review
git switch -c fix/from-mNN cartograph-mNN
```

---

## M00 — Engineering foundation

| Field | Value |
|---|---|
| Status | **ACCEPTED** |
| Accepted | 2026-08-19 by the project owner |
| Branch | `feature/m00-foundation` · PR [#1](https://github.com/Rexy-5097/cartograph/pull/1) (merged) |
| Provisional checkpoint | `cartograph-m00-rc1` (retained for traceability) |
| **Accepted checkpoint** | **`cartograph-m00`** — merge commit `ab196a1`, immutable |
| Specification | Frozen Engineering Spec V3 |

> **Checkpoint repair (2026-08-19).** The original `cartograph-m00` tag was
> created prematurely at internal completion and pointed one commit short of
> the reviewed PR state. Per [ADR-0010](docs/adr/ADR-0010-checkpoint-tag-policy.md)
> it was retired (safety determined first: zero forks, zero releases, PR
> unmerged, milestone unaccepted) and replaced by provisional
> `cartograph-m00-rc1` at the exact PR head. The name `cartograph-m00` is
> reserved for the accepted checkpoint.

### Scope delivered

Rust workspace with six crates; domain model with evidence, provenance and
confidence; architecture graph over `petgraph`; `cartograph version`; AgentOS
v1.0.0 vendored and adapted; GitHub repository, CI and pull request governance;
quality gates QG-001 – QG-008; milestone and checkpoint systems; nine ADRs.

### Gate results

| Gate | Result |
|---|---|
| `cargo fmt --all -- --check` | PASS |
| `cargo clippy --workspace --all-targets --all-features -- -D warnings` | PASS |
| `cargo test --workspace` | PASS — 50 tests |
| `cargo check --workspace --all-targets` | PASS |
| `cargo bench --no-run` | PASS |
| AgentOS validator | PASS — 99/100 |
| QG-001 … QG-008 | PASS — 8/8 (local run 2026-08-18; CI re-runs on the PR) |
| CI | *(recorded on merge)* |

### Known limitations

- No parsing, no resolution, no storage. `cartograph-parser` and
  `cartograph-resolver` are empty by design.
- Node identity is graph-local and not content-addressed (M10).
- No benchmark results exist. Criterion is wired up; no number is published.
- Confidence values are uncalibrated priors (M08).

### Rollback point

`cartograph-m00` (accepted). `cartograph-m00-rc1` remains as the pre-merge
provisional state. There is no earlier checkpoint — M00 is the first.

---

## M01 — TypeScript extraction

| Field | Value |
|---|---|
| Status | **ACCEPTED** |
| Accepted | 2026-08-19 by the project owner |
| Branch | `feature/m01-typescript-extraction` · PR [#2](https://github.com/Rexy-5097/cartograph/pull/2) (merged) |
| Provisional checkpoint | `cartograph-m01-rc3` (retained for traceability; rc1/rc2 superseded pre-acceptance) |
| **Accepted checkpoint** | **`cartograph-m01`** — merge commit `88c2b88`, immutable |

### Scope delivered

tree-sitter TypeScript/TSX extractor behind a language-neutral fact model
(imports, symbols with enclosing/export tracking, call sites, string and
template structure preserved for M05, HTTP observations that are explicitly
not edges); structured error-tolerant diagnostics; `cartograph parse` CLI
command with `--json`; 22-category fixture corpus; `ts_extraction` criterion
group; two real-repository smoke tests. Also: checkpoint policy repair
(ADR-0010) applied to M00.

### Gate results

| Gate | Result |
|---|---|
| `cargo fmt --all -- --check` | PASS |
| `cargo clippy --workspace --all-targets --all-features -- -D warnings` | PASS |
| `cargo test --workspace` | PASS — 82 tests |
| QG-001 … QG-008 | PASS — 8/8 (local run 2026-08-19; CI re-runs on the PR) |
| AgentOS validator | PASS — 99/100 |
| Smoke tests | zustand 35 files / swr 194 files, 0 failed parses, 0 false HTTP positives |

### Benchmark state

`ts_extraction` group runs (small fixture ~45 µs; 500-fn module ~9 ms;
100-file synthetic project ~27 ms on the dev machine). Internal only — not
published as performance claims.

### Known limitations

Arrow-function consts recorded as variables; destructuring not extracted;
module-scope variables only; grammar gaps on advanced type-level syntax
(diagnostics + continued extraction); sequential processing.

### Rollback point

`cartograph-m01` (accepted). Previous accepted checkpoint: `cartograph-m00`.
Provisional predecessor `cartograph-m01-rc3` remains available.

---

## M02 — Python extraction

| Field | Value |
|---|---|
| Status | **ACCEPTED** |
| Accepted | 2026-08-19 by the project owner |
| Branch | `feature/m02-python-extraction` · PR [#3](https://github.com/Rexy-5097/cartograph/pull/3) (merged) |
| Provisional checkpoint | `cartograph-m02-rc1` (retained for traceability) |
| **Accepted checkpoint** | **`cartograph-m02`** — merge commit `eaba30b`, immutable |

### Scope delivered

tree-sitter Python extractor as a peer of the TypeScript one behind a single
dispatch; imports, symbols (with `__all__` handling and no invented exports),
decorators, calls, strings and f-strings, HTTP observations, and a new shared
`RouteObservation` for verb decorators, `@app.route` and URL-conf entries.
CLI walks mixed `.ts`/`.tsx`/`.py` trees. 32-category fixture corpus,
`py_extraction` benchmarks, three real-repository smoke corpora.

### Gate results

| Gate | Result |
|---|---|
| `cargo fmt --all -- --check` | PASS |
| `cargo clippy --workspace --all-targets --all-features -- -D warnings` | PASS |
| `cargo test --workspace` | PASS — 132 tests |
| `cargo check --workspace --all-targets` | PASS |
| `cargo bench --workspace --no-run` | PASS |
| QG-001 … QG-008 | PASS — 8/8 |
| AgentOS validator | PASS — 99/100 |
| Smoke tests | 384 real Python files, 0 failed parses, 0 diagnostics |

### Benchmark state

`py_extraction`: small fixture ~56 µs; 500-route module ~12.4 ms; 100-file
project ~36.9 ms. Internal only, not published.

### Known limitations

Route receivers must be recognisable; non-literal Django patterns skipped;
test-client calls counted as HTTP observations; decorator arguments beyond the
path not extracted; tuple/attribute assignment targets not recorded.

### Rollback point

`cartograph-m02` (accepted). Previous accepted checkpoint: `cartograph-m01`.
Provisional predecessor `cartograph-m02-rc1` remains available.

---

## M03 — Canonical route normalisation

| Field | Value |
|---|---|
| Status | **ACCEPTED** |
| Accepted | 2026-08-19 by the project owner |
| Branch | `feature/m03-route-normalization` · PR [#4](https://github.com/Rexy-5097/cartograph/pull/4) (merged) |
| Provisional checkpoint | `cartograph-m03-rc1` (retained for traceability) |
| **Accepted checkpoint** | **`cartograph-m03`** — merge commit `ca0d206`, immutable |

### Scope delivered

Canonical route model (`Segment`, `Methods`, `CanonicalPath`,
`NormalizationStatus`, `NormalizationNote`, `RouteProvenance`) and a
deterministic normalisation pipeline in `cartograph-resolver`, consuming both
Python route declarations and TypeScript/Python client-call URLs. Five
parameter dialects, URL decomposition, template structure, and explicit refusal
of regexes and dynamic paths. `cartograph normalize` CLI command. 59
canonicalisation tests, `canonicalization` benchmarks.

### Gate results

| Gate | Result |
|---|---|
| `cargo fmt --all -- --check` | PASS |
| `cargo clippy --workspace --all-targets --all-features -- -D warnings` | PASS |
| `cargo test --workspace` | PASS — 191 tests |
| `cargo check --workspace --all-targets` | PASS |
| `cargo bench --workspace --no-run` | PASS |
| QG-001 … QG-008 | PASS — 8/8 |
| AgentOS validator | PASS — 99/100 |

### Benchmark state

`canonicalization`: 8-route fixture ~2.06 µs; 1000 literal routes ~350 µs;
1000 templates ~551 µs. Internal only, not published.

### Known limitations

Route mount prefixes not applied (needs cross-file resolution, M04); no
relative-path resolution against a base URL (M05); regex detection is
deliberately generous, so a structurally readable pattern is still refused.

### Rollback point

`cartograph-m03` (accepted). Previous accepted checkpoint: `cartograph-m02`.
Provisional predecessor `cartograph-m03-rc1` remains available.

---

## M04 — Cross-language route resolution

| Field | Value |
|---|---|
| Status | **ACCEPTED** |
| Accepted | 2026-08-19 by the project owner |
| Branch | `feature/m04-cross-language-resolution` · PR [#5](https://github.com/Rexy-5097/cartograph/pull/5) (merged) |
| Provisional checkpoint | `cartograph-m04-rc1` (retained for traceability) |
| **Accepted checkpoint** | **`cartograph-m04`** — merge commit `1082f9a`, immutable |

> Gates were re-run on `main` after PR #6, which added QG-009 and the pinned
> toolchain. No crate file changed between `1082f9a` and that run, so the
> product code validated is exactly the code accepted.

### Scope delivered

The first semantic join and the first `HttpCall` edges. Candidate generation
over an arity-bucketed route index; deterministic method and path compatibility
rules; a discriminating-evidence requirement; ambiguity that produces no edge;
explicit refusals for unresolved prefixes, uncanonicalisable paths and absolute
hosts. Edge construction with confidence, provenance `route-matcher`, and
evidence naming both sides and the rules that fired. `cartograph match` CLI.

### Gate results

| Gate | Result |
|---|---|
| `cargo fmt --all -- --check` | PASS |
| `cargo clippy --workspace --all-targets --all-features -- -D warnings` | PASS |
| `cargo test --workspace` | PASS — 241 tests |
| `cargo check --workspace --all-targets` | PASS |
| `cargo bench --workspace --no-run` | PASS |
| QG-001 … QG-008 | PASS — 8/8 |
| AgentOS validator | PASS — 99/100 |

### Benchmark state

`matching`: per-decision 136–399 ns; index build 1000 routes ~470 µs; match
against 1000 routes ~7.9 µs; 1000 routes × 100 clients end-to-end ~1.22 ms.
Internal only, not published.

### Verification findings

Fixtures were green throughout while the matcher was producing false edges on
real input; both defects below were found by inspecting real-repository output
by hand, not by any synthetic test.

**False positives found and fixed (2).**

| Corpus | Before | After | Cause |
|---|---|---|---|
| pallets/flask | 167 accepted edges | 29 | A bare catch-all `/{path:path}` in one test module matched client calls from every other module — 138 false edges from a single route. |
| pallets/flask | included above | — | `GET /user_id` matched `GET /{name}` declared in an unrelated module. |

Both candidates shared **zero static segments** with the client path. Root
cause: a route made entirely of variable positions accepts a whole class of
paths, so aligning with it is not evidence about any one call. Fix: an accepted
match now requires at least one shared static segment (the root path
discriminates by equality). Regression tests
`a_bare_catch_all_route_does_not_swallow_unrelated_calls` and
`a_single_parameter_route_does_not_capture_every_one_segment_call` name where
each came from.

**Differential check after the fix.** Flask 167 → 29 edges; the 138 removed
were all catch-all artefacts. The 29 survivors were inspected by hand and are
genuine — Flask's tutorial test suite calling its own blog routes
(`POST /1/update` → `GET|POST /{id}/update` → `update`). Django-REST-Framework
was unchanged at 50, all same-module `urlpatterns` calls at 0.784, correctly
reduced for Django's undeclared method.

**Investigated, not a defect.** `full-stack-fastapi-template` reports 100%
unsupported (63 of 63). Classified as a true refusal: that template builds
every URL from an `${API_URL}` prefix, so the path length is unknown and no
alignment is safe. M05's constant propagation is what unlocks it. Recorded
rather than silently accepted.

**Output inspected by hand:** an exact literal match, a parameter-bound
boundary case, an ambiguous pair, a method-mismatch negative, and an
uninterpreted regex — plus the evidence string and candidate reasons for each
accepted edge.

**Defects found during implementation:** four clippy findings and one
`PathCompatibility` fallback arm, each fixed before the next slice began.

**Unresolved limitations** are listed below; none is an unexplained output.

### Known limitations

Route mount prefixes not composed (largest source of missed matches); no
same-service scoping; test-client calls matched like any other; absolute-host
clients never matched; OpenAPI confirmation deferred.

### Rollback point

`cartograph-m04` (accepted). Previous accepted checkpoint: `cartograph-m03`.
Provisional predecessor `cartograph-m04-rc1` remains available.

---

## M05 — Dynamic URL and endpoint resolution

| Field | Value |
|---|---|
| Status | **ACCEPTED** |
| Accepted | 2026-08-19 by the project owner |
| Branch | `feature/m05-dynamic-url-resolution` · PR [#7](https://github.com/Rexy-5097/cartograph/pull/7) (merged) |
| Provisional checkpoint | `cartograph-m05-rc1` (retained for traceability) |
| **Accepted checkpoint** | **`cartograph-m05`** — merge commit `5770b59`, immutable |

> Verified at acceptance: the milestone's own criterion still holds on the
> accepted commit — `` `${API_BASE}/orders` `` resolves to `/api/v1/orders`
> and reaches `create_order` at 0.98, while the environment-backed URL is still
> refused. No crate file changed between `cartograph-m05-rc1` and `main`.

### Scope delivered

Symbolic string value model; restricted expression evaluation over module
constants, templates, f-strings, concatenation and environment reads;
cross-file constant propagation through exported bindings; partial URL
reconstruction into an ordinary `UrlObservation`; integration with the
unchanged M04 matcher. `cartograph match` resolves dynamic URLs.

### Gate results

| Gate | Result |
|---|---|
| `cargo fmt --all -- --check` | PASS |
| `cargo clippy --workspace --all-targets --all-features -- -D warnings` | PASS |
| `cargo test --workspace` | PASS — 297 tests |
| `cargo check --workspace --all-targets` | PASS |
| `cargo bench --workspace --no-run` | PASS |
| QG-001 … QG-009 | PASS — 9/9 |
| AgentOS validator | PASS — 99/100 |

### Benchmark state

`evaluation`: literal constant 35.9 ns · transitive constant 166 ns ·
environment read 29.5 ns · scope from 200 constants 18.8 µs · 100-importer
project 51.8 µs. Internal only.

### Verification findings

**A real defect, found only by running the binary.** The evaluator was wired
into `cartograph match`, the workspace compiled, and all 54 resolver unit tests
passed — while the command still ran the M04-only path. One of four edits had
silently failed to apply because its anchor no longer matched after an earlier
reformat. Fixture tests could not catch it because they call the library
directly. Found by inspecting real-repository output and noticing
`Dynamic URLs 0 fully resolved, 0 partially` when 63 templates should have
reached the evaluator. Fixed, and two CLI-level regression tests added
(`match_resolves_an_imported_constant_end_to_end`,
`match_still_refuses_an_environment_backed_url`) so the gap between "the
library works" and "the command works" is now covered.

**Real-repository before/after — no change, and that is the honest result.**

| Corpus | M04 decisions | M05 decisions | Edges before → after | Dynamic URLs seen |
|---|---|---|---|---|
| tiangolo/full-stack-fastapi-template | 63 unsupported | 63 unsupported | 0 → 0 | 63 partial |
| pallets/flask | 23/6/288/0/14 | 23/6/288/0/14 | 29 → 29 | 12 partial |
| encode/django-rest-framework | 0/50/271/95/22 | 0/50/271/95/22 | 50 → 50 | 15 partial |
| vercel/swr | 4 no-match, 6 unsupported | identical | 0 → 0 | 6 partial |

**Zero additional matches; zero regressions.** Classified: the FastAPI
template's 63 refusals are **safe refusals**, not bugs. Its generated SDK builds
every URL from a base supplied at client-construction time
(`import.meta.env.VITE_API_URL ?? ""`), so the prefix is genuinely not
statically determined. Resolving it would require assuming the environment
variable is unset — a deployment assumption, not an observation. The remaining
partial resolutions are the same pattern.

The capability is proven by fixtures (an imported constant resolves
`` `${API_BASE}/orders` `` to `/api/v1/orders` and produces an exact-match
edge); its real-world reach is limited by a construction pattern M05 cannot
resolve. Stated plainly rather than presented as a win.

**Differential check:** every corpus produced byte-identical decision counts to
M04. Since a literal URL is declined by the evaluator and takes the unchanged
M04 path, previously-correct behaviour cannot shift — asserted by
`literal_urls_behave_exactly_as_they_did_before_m05`.

**Adversarial testing.** Ten refusal cases assert `NO EDGE IS PRODUCED`, not
merely an empty vector: unexported constant, package import, environment
prefix, unsupported expression, wrong resource, wrong method, catch-all,
two-equal-routes, empty URL, and cyclic constants. The environment-leak test
uses `PATH` so a leak would be visible.

**Output inspected by hand** across all five case classes — fully known, known
prefix with unknown suffix, wholly unknown, environment-backed, unsupported —
plus the CLI's rendering of each.

**Other defects fixed during implementation:** six clippy findings, and an
`unsafe` env-var test rewritten to respect the workspace's `unsafe_code` ban.

### Known limitations

Runtime-configured SDK clients unresolvable (the dominant real-world pattern);
object/config-field bases unknown; no tsconfig aliases or `node_modules`
resolution; locals and parameters never bound; conditional expressions
unsupported rather than explored.

### Rollback point

`cartograph-m05` (accepted). Previous accepted checkpoint: `cartograph-m04`.
Provisional predecessor `cartograph-m05-rc1` remains available.

---

## M09 — CLI v0.1.0

| Field | Value |
|---|---|
| Status | **ACCEPTED** |
| Accepted | 2026-08-31 by the project owner |
| Base | `cartograph-m08` — commit `9dbb38e` |
| Branch | `feature/m09-cli-v010` · PR [#15](https://github.com/Rexy-5097/cartograph/pull/15) (merged `c578b42`) |
| Acceptance fix | `fix/m09-windows-rooted-path` · PR [#16](https://github.com/Rexy-5097/cartograph/pull/16) (merged `e8416f9`) |
| Provisional checkpoint | none — ADR-0010: none was needed |
| **Accepted checkpoint** | **`cartograph-m09`** — commit `e8416f9`, immutable |

> The accepted state is PR #15 **plus** PR #16. Tagging `c578b42` was rejected:
> its gates fail on Windows. This follows M07, which folded its acceptance
> fixes (PRs #12, #13) into the accepted checkpoint rather than tagging a state
> that did not pass.

### Scope delivered

`cartograph <path>`, `trace`, `parse`, `normalize`, `match` and `version`, each
composing the same shared pipeline so two commands cannot report on differently
built graphs; a versioned `--json` document (`schema_version` 1.0) with a
schema conformance suite; documented, tested exit codes; and an npm launcher
that finds the native binary and forwards argv, stdio and the exit code without
reimplementing any analysis.

### Why PR #16 was required

Bringing the project onto Windows found a privacy defect. `display` and
`Repository::describe` gated redaction on `Path::is_absolute()`, which on
Windows requires a drive or UNC prefix. A path rooted on the current drive was
therefore reproduced in full — in the serialised `repository.requested` field
and in the human-readable error:

| Input | Before | After |
|---|---|---|
| `\dev\…\fixtures` | `"requested": "/dev/cartograph/crates/…"` | `"requested": "<absolute>"` |
| `\rooted\secret-project` | `` `/rooted/secret-project` does not exist `` | `` `secret-project` does not exist `` |
| `C:\workspace\secret-project` | already redacted | unchanged |
| `relative/path` | echoed as typed | unchanged |

`discovery::is_rooted` (`is_absolute() || has_root()`) now owns the question and
both call sites use it. On Unix the two predicates are equivalent, which is why
CI never saw the defect. The analyser was not touched.

### Gate results

Run on Windows 11, `x86_64-pc-windows-msvc`, Rust 1.97.1, at `e8416f9`.

| Gate | Result |
|---|---|
| `cargo fmt --all -- --check` | PASS |
| `cargo check --workspace --all-targets` | PASS |
| `cargo clippy --workspace --all-targets --all-features -- -D warnings` | PASS |
| `cargo test --workspace` | PASS — 497 tests, 0 failed |
| `benchmarks/m08/test_calibration.py` | PASS — 23 tests |
| `cargo bench --workspace --no-run` | PASS |
| `cargo build --release -p cartograph-cli` | PASS |
| QG-001 … QG-009 | PASS — 9/9 |
| AgentOS validator | PASS — 98/100, 3 warnings |
| CI on `e8416f9` | PASS — all six required checks |

### Verification findings

**The analyser is byte-identical to `cartograph-m08`.** `git diff
cartograph-m08 -- crates/cartograph-{core,graph,parser,resolver,testkit}` is
empty. Verified behaviourally as well: the pre-fix binary at `c578b42` and the
accepted binary produce identical `parse` and `normalize` output on the pinned
`full-stack-fastapi` corpus, and identical `summary`/`match` output apart from
timing fields.

**Real-repository regression, at the pinned commits.**
`full-stack-fastapi` (`162344da`) — 156 files, 23 routes, 0 failed.
`zulip` (`0ce8f627`) — 1,539 files, 235 routes, 1,308 nodes, 1,469 edges, 0
failed; `trace` returns a cross-language chain with evidence, confidence,
provenance and source locations. No filesystem path appeared in any output.

**Windows privacy regression.** Five rooted forms — drive-qualified, drive-less
backslash, forward-slash, UNC and verbatim — leak nothing to stdout, stderr or
JSON, all exit 3, none panics, and JSON stays well formed. Relative paths are
still echoed exactly as typed, so the redaction is not implemented by redacting
everything.

**The accuracy figures are M08's, unchanged.** M09 shipped no analyser change,
so it inherits M08's limits rather than improving on them: confidence is an
uncalibrated prior, and 24.9% of edges remain unverifiable from source.

### Accepted limitations

- Memory scales with repository size — 920 MB on a 32,000-file repository.
- No incremental analysis, no file watching, no cache between runs (M10).
- TypeScript, TSX and Python only.
- The supported subset only; anything outside it is refused, not guessed.
- `trace` walks outward only — no reverse traversal, no blast radius (M12+).
- Node ids are stable within a run, not across runs.
- Confidence is not a calibrated probability and must not be thresholded.
- **Windows is validated for development, not for release artifacts.** CI
  builds and smoke-tests `x86_64-unknown-linux-gnu` and `aarch64-apple-darwin`
  on every pull request. `x86_64-apple-darwin` and `x86_64-pc-windows-msvc` are
  configured in `release.yml` but **not yet produced** — the workflow runs on
  tag push and no `v*` tag exists. Build from source on those targets.

---

## M08 — Confidence calibration and measurement

| Field | Value |
|---|---|
| Status | **ACCEPTED** |
| Accepted | 2026-08-22 by the project owner |
| Base | `cartograph-m07` — commit `2514f77` |
| Branch | `feature/m08-calibration` · PR [#14](https://github.com/Rexy-5097/cartograph/pull/14) (merged `9dbb38e`) |
| Provisional checkpoint | none — ADR-0010: none was needed |
| **Accepted checkpoint** | **`cartograph-m08`** — commit `9dbb38e`, immutable |

> A measurement milestone. The analyser is **byte-identical to
> `cartograph-m07`** — `git diff cartograph-m07 main -- crates/` is empty — so
> no production confidence value or threshold changed, and M07's graph
> behaviour holds by construction rather than by re-measurement.

### Scope delivered

An independent per-edge labeller that re-derives every relationship from
source, including a second implementation of router prefix composition written
from FastAPI's semantics; a labelled dataset of 14,932 edges bound to the
digests of its inputs **and of its own records**; calibration by confidence
value, provenance and relationship class; threshold trade-offs; recall for the
three classes M07 left unmeasured; a holdout split computed from edge identity;
sixteen integrity checks; and QG-009 enforcement of all of it.

### Gate results

| Gate | Result |
|---|---|
| `cargo fmt --all -- --check` | PASS |
| `cargo check --workspace --all-targets` | PASS |
| `cargo clippy --workspace --all-targets --all-features -- -D warnings` | PASS |
| `cargo test --workspace` | PASS — 405 tests |
| `benchmarks/m08/test_calibration.py` | PASS — 23 tests (run by `make gates`) |
| `cargo bench --workspace --no-run` | PASS |
| QG-001 … QG-009 | PASS — 9/9 |
| AgentOS validator | PASS — 100/100 |
| CI on `9dbb38e` | PASS |

### Findings

**Confidence is an uncalibrated prior selected by evidence class, not a
probability.** Seven distinct values across 14,932 edges, 79% at exactly 0.80.
Grouping by confidence and grouping by evidence are the same partition.

| | |
|---|---|
| ECE | **0.1819** development · **0.1825** holdout |
| MCE | **0.35** |
| Direction | **under**-confidence — observed ~1.0 against stated 0.80–0.98 |
| `HttpCall` recall | **117/129** within the generated-client scope |
| `OrmAccess` recall | **1401/1883** within the import-bound scope |
| Chain recall | **3 of 5** |
| False positives | **1** in 11,221 verified edges |
| Integrity / adversarial | 16/16 · 14/14 pipeline · 8/8 gate |

**These measurements are bounded by the independently verifiable subset and
should not be interpreted as corpus-wide accuracy.**

### Accepted limitations — preserved, not resolved

1. **24.9% of edges (3,711 of 14,932) are unverifiable from source**, and not a
   random quarter: an edge is unverifiable for the same reasons it is weak.
2. **The low-confidence bands have zero verified observations** — 0.64, 0.72
   and 0.784, 697 edges between them. 0.65 has eight, too few to estimate from.
3. **Overconfidence is therefore unmeasured.** Only under-confidence could be
   observed.
4. **The chain-recall denominator is five.** Reported as a count, never a rate.
5. **One false positive remains, deliberately.** A function-local `class Element`
   in a PostHog test shadows a real ORM model; M08 measures rather than patches.
6. **Recall is scope-specific**, one repository and one client family — not
   corpus-wide.

M08 is accepted as a trustworthy measurement baseline, not as completed
calibration science.

## M07 — Full-stack verification on real repositories

| Field | Value |
|---|---|
| Status | **ACCEPTED** |
| Accepted | 2026-08-22 by the project owner |
| Base | `cartograph-m06` — commit `ef26ae2` |
| Validation | PR [#10](https://github.com/Rexy-5097/cartograph/pull/10) (merged `7986348`), containing PR [#11](https://github.com/Rexy-5097/cartograph/pull/11) remediation (merged `cb1f1db`) |
| Acceptance-audit corrections | PR [#12](https://github.com/Rexy-5097/cartograph/pull/12) (merged `7a3e8a5`) · PR [#13](https://github.com/Rexy-5097/cartograph/pull/13) (merged `2514f77`) |
| Provisional checkpoint | none — ADR-0010: none was needed |
| **Accepted checkpoint** | **`cartograph-m07`** — commit `2514f77`, immutable |

> The accepted commit is the PR #13 merge, not the PR #10 merge. M07 took four
> merges: the validation that produced the negative finding, the remediation
> that answered it, and two acceptance audits that each found one false
> positive and refused it. Accepting an earlier commit would accept a corpus
> with a known false positive in it.

### Acceptance criterion

**PASS** — four genuine frontend-originating complete chains on a real
repository (`apache/airflow` at `9b43d6a`), each traversing shared graph nodes
with confidence, provenance, evidence and a source location on every edge:

| Origin (hand-written React) | Handler | Model | Table |
|---|---|---|---|
| `ui/src/components/DagActions/ParseDagButton.tsx:30` | `reparse_dag_file` | `DagPriorityParsingRequest` | `dag_priority_parsing_request` |
| `ui/src/pages/Connections/AddConnectionButton.tsx:29` | `post_connection` | `Connection` | `connection` |
| `ui/src/pages/Connections/TestConnectionButton.tsx:37` | `test_connection` | `Connection` | `connection` |
| `ui/src/pages/Pools/AddPoolButton.tsx:28` | `post_pool` | `Pool` | `slot_pool` |

**HttpCall recall: UNMEASURED · OrmAccess recall: UNMEASURED · chain recall:
UNMEASURED.** No independent denominator exists for these classes. They are
limitations of the evaluation, not evidence of absence, and M07 is accepted
with them open because the product criterion it was created to test is met.

### Scope delivered

A pinned seven-repository corpus (Superset, full-stack-fastapi, Onyx, PostHog,
Zulip, Airflow, AutoGPT) covering React/TS + FastAPI, Flask and Django, Next.js
+ Python, SQLAlchemy, Django ORM, and three repositories chosen for patterns
known to be out of scope. A supported-subset definition committed before the
first measurement. Ground truth authored from source by an instrument with no
access to analyser output. Two evaluation passes over the whole corpus, with
metrics, explicit denominators and ten adversarial checks on the benchmark
machinery itself. `cartograph match --json` now exports the graph it built, so
chains are verified by traversal rather than inferred from names.

No product capability was added. The five source changes are defect fixes the
corpus exposed.

### Gate results

| Gate | Result |
|---|---|
| `cargo fmt --all -- --check` | PASS |
| `cargo check --workspace --all-targets` | PASS |
| `cargo clippy --workspace --all-targets --all-features -- -D warnings` | PASS |
| `cargo test --workspace` | PASS — 354 tests |
| `cargo bench --workspace --no-run` | PASS |
| QG-001 … QG-009 | PASS — 9/9 |
| AgentOS validator | PASS |

### Benchmark state

Seven repositories, 58,225 files, 68.4 s total, peak RSS 825 MB (PostHog).
Route declarations: precision 1.000, recall 1.000 over 101 in-scope records.
Model-to-table: precision 1.000, recall 1.000 over 48 in-scope records, and 290
of 290 produced edges confirmed against source. Full report:
[docs/benchmarks/m07-report.md](docs/benchmarks/m07-report.md). INTERNAL — the
methodology is first-generation and these numbers are not for publication.

### Verification findings

**The headline is a negative result, and it is the point of the milestone.**
Across seven real repositories Cartograph produced **six** fully verified
frontend-to-table chains, and **none begins at a frontend file**. Of 1,458
`HttpCall` edges, 12 have a TypeScript client and 8 are in non-test TypeScript;
the rest are Python test suites calling their own API. Three documented causes:
real frontends call through generated client wrappers so no URL appears at the
call site; `APIRouter(prefix=…)` composes the served path at registration; and
Django's chain breaks because the route's handler node and the view's handler
node differ by file under ADR-0011. Ten future-work items are recorded; none
was built.

**Five defects were found by reading output, each fixed with a fixture.**

1. **`Model` read as Django.** Django, Flask-SQLAlchemy and Flask-AppBuilder
   all use a base named `Model`; testing Django first sent table resolution
   looking for a `Meta.db_table` that is not there while `__tablename__` sat a
   line below. Superset resolved 3 tables out of 33. Flavour is now read from
   what the class declares. Superset 3 → 32; `Queries` recall 0.729 → 1.000.
2. **ORM discovery ran over TypeScript.** `new Theme({config})` in a `.tsx`
   file produced an `OrmAccess` edge to a Python model of the same name — 33
   across the corpus, now 0.
3. **Model names resolved without imports.** Airflow's architecture diagrams
   call `User("DAG Author")` having imported `User` from the `diagrams`
   package; 218 were attributed to the Flask-AppBuilder model.
4. **Route decorators required a listed receiver.** Superset's four
   `@health_blueprint.route(...)` declarations were invisible — the only route
   misses left in the corpus.
5. **TypeScript route declarations read as client calls.** `app.get('/_health',
   (c) => …)` is Express and Hono declaring a route; three in PostHog's Node
   services were matched to a Python endpoint in a different service.

**Six defects were found in the benchmark instrument, not in Cartograph**, and
three of them corrected numbers in Cartograph's favour — which is why they are
listed rather than quietly applied. A multi-line `class SqlaTable(` did not
close the previous class's scope, so its `__tablename__` was recorded against
`SqlMetric`; route declarations inside docstrings were counted as routes, so
Cartograph was charged three false negatives for correctly declining to extract
documentation; and ORM flavour was read from the base name, mislabelling all 32
Superset and all 61 Airflow models as Django.

**The first scope definition was circular and was rewritten before any
measurement.** Version 1 defined "supported" as the set of patterns
`cartograph-parser` recognises, which makes recall 1.0 by construction. Version
2 declares scope by framework capability family and predicts seven specific
implementation gaps in advance, so a miss found later cannot be reclassified as
out of scope.

**Refusals were validated at scale on real input.** 1,886 client calls resolved
to `Ambiguous` and produced no edge. `full-stack-fastapi` produced zero edges
from 23 routes and 63 client calls, and its 13 SQLModel classes produced no
table, satisfying all 13 `must_not_produce` assertions. Zulip declares 74
Django models and one literal `db_table`; exactly one table was resolved, the
other 73 correctly unresolved because Django's default name needs an app label
that is not in the model file.

**What the metrics do not cover.** `HttpCall` recall, `OrmAccess` recall and
chain recall are unmeasured. Four of the seven predicted gaps would surface in
`OrmAccess` recall; their absence from the tables is a gap in the evaluation,
not evidence of their absence from the engine. 286 `OrmAccess` edges (2.4%)
remain unconfirmable without Python import resolution and are reported rather
than suppressed.

## M06 — ORM and database resolution

| Field | Value |
|---|---|
| Status | **ACCEPTED** |
| Accepted | 2026-08-19 by the project owner |
| Branch | `feature/m06-orm-resolution` · PR [#8](https://github.com/Rexy-5097/cartograph/pull/8) (merged `88d2fda`) |
| Verification PR | [#9](https://github.com/Rexy-5097/cartograph/pull/9) (merged `ef26ae2`) — tests and ledgers only, no product source |
| Provisional checkpoint | `cartograph-m06-rc1` at `81fd32e` (the PR #8 head; retained) |
| **Accepted checkpoint** | **`cartograph-m06`** — commit `ef26ae2`, immutable |

> The accepted commit is the PR #9 merge, not the PR #8 merge: the independent
> verification pass added four regression tests and corrected the ledgers, and
> the accepted state must include them. PR #9 touched no product source, so
> M06's behaviour at `ef26ae2` is exactly what PR #8 delivered.

### Scope delivered

SQLAlchemy and Django model discovery; `__tablename__` and `Meta.db_table`
resolution with no name ever derived from a class name; handler-to-model access
over a deliberate subset; `OrmAccess` and `Queries` edges; ADR-0011 node
identity; the complete cross-stack chain walkable end to end.

### Gate results

| Gate | Result |
|---|---|
| `cargo fmt --all -- --check` | PASS |
| `cargo clippy --workspace --all-targets --all-features -- -D warnings` | PASS |
| `cargo test --workspace` | PASS — 335 tests |
| `cargo bench --workspace --no-run` | PASS |
| QG-001 … QG-009 | PASS — 9/9 |
| AgentOS validator | PASS — 99/100 |

### Benchmark state

`orm`: 1 model 473 ns; 500 models 639 us; 100 models x 100 handlers 67.5 us;
graph build 143 us; full-stack chain 1.92 us. Internal only.

### Verification findings

**Two real defects, both found by inspecting output rather than by tests.**

1. **`Meta` name collision.** Django nests a class called `Meta` in every model,
   and matching the `db_table` attribute by its owner's *name* attributed the
   first `db_table` in a file to every model in it. A model with a dynamic
   `db_table = settings.TABLE` inherited an unrelated model's table. Root cause:
   identity by name where names are universal. Fixed by span containment;
   regression test `a_dynamic_db_table_expression_is_refused`.
2. **Graph identity — the chain was unwalkable.** M04 created a `Function` node
   for a route's handler and M06 created a second for the same handler. Every
   edge was individually correct and the path was still broken. Found only by a
   full-stack traversal test. Fixed by `ArchitectureGraph::node_for`
   (ADR-0011); five graph regression tests added, one reproducing the
   two-analyses case directly.

**A third issue caught during slice 0**, before any ORM code existed: M02
skipped class-body assignments, so `__tablename__` was invisible. Without
fixing the parser, M06 would have had to guess which string in a class was a
table name. Two parser facts were added instead: class attributes and class
bases.

**Real-repository validation**

| Corpus | Ref | Models | With table | Ambiguous | ORM edges |
|---|---|---|---|---|---|
| sqlalchemy/sqlalchemy | `16d00ec` | 37 | 35 | 25 | 228 |
| django/django | `d992705` | 869 | 34 | 158 | 1957 |
| encode/django-rest-framework | `751a19f` | 73 | 0 | - | 119 |
| tiangolo/full-stack-fastapi-template | `162344d` | 0 | 0 | - | 0 |

**Ground truth checked by hand.** SQLAlchemy's `AddressAssociation` resolves to
`address_association`, matching its source exactly. Django's `A02 -> a01` looked
like a false positive and was investigated: `tests/unmanaged_models/models.py`
genuinely declares `db_table = "a01"` on `A02`. Reading the declaration gave the
right answer where deriving from the class name would have given `a02` - the
clearest evidence that refusing to derive is correct.

The FastAPI template yields zero models, correctly: it uses SQLModel, outside
the supported set. Django's 869 models yielding only 34 tables is the app-label
refusal working at scale, not a failure.

**Before/after - zero regressions.** M04 and M05 decision counts are identical
on every previously-tested corpus (DRF 0/50/271/95/22 with 50 edges; FastAPI 63
unsupported with 0 edges). Every new edge is an ORM edge; no HTTP decision moved.

**Independent verification pass.** Every slice was re-verified after
implementation rather than trusted from its own build: focused test runs per
slice, parser output inspected directly against all four mandated cases,
ground-truth checks against corpus source, and a before/after comparison.
Four gaps were found and closed:

1. A **conditionally declared** `__tablename__` (`if DEBUG:`) was correctly
   refused but untested — the parser records class-body assignments, not ones
   nested in control flow, so choosing a branch is impossible by construction.
   Regression test added.
2. A **non-model class carrying `__tablename__`** was correctly refused but
   untested. Added.
3. The **Meta-collision regression** asserted only that the table name was
   absent, not that **no edge** was produced. Rewritten to the exact
   construction and to assert one table node and one `Queries` edge.
4. Four refusal tests asserted an empty collection **without proving why**.
   Each now carries a positive control — the model resolved, the call exists —
   so the test proves the refusal is for the intended reason rather than a
   failure to find anything.

A fifth check was added at pipeline level: `the_handler_is_exactly_one_node_across_both_analyses`
reproduces the ADR-0011 defect through the real analyzers rather than a
synthetic graph, asserting one handler node with one inbound and one outbound
edge and four nodes total.

**Negative coverage:** 17 refusal cases assert no edge - non-model classes,
similar base names, dynamic `db_table`, malformed `Meta`, unrecognised manager
methods, shadowed names, module-scope access, ambiguous model names, raw SQL,
and unresolved tables.

### Known limitations

`session.query(Order)` unsupported (the model is a call argument and M02 records
argument counts, not identities); only SQLAlchemy and Django; Django default
table names never derived; model inheritance not followed; shadow detection is
file-wide rather than flow-sensitive.

### Rollback point

`cartograph-m06` (accepted). Previous accepted checkpoint: `cartograph-m05`.
Provisional predecessor `cartograph-m06-rc1` remains available.

---

## Template for subsequent entries

```markdown
## MNN — <title>

| Field | Value |
|---|---|
| Status | IN REVIEW / ACCEPTED / FAILED |
| Date | YYYY-MM-DD |
| Provisional checkpoint | cartograph-mNN-rcK |
| Accepted checkpoint | cartograph-mNN (merge commit; at acceptance only) |
| Branch | feature/mNN-slug |
| Pull request | #N |

### Gate results
| Gate | Result |
|---|---|
| fmt / clippy / tests / CI / QG-001…009 | |

### Verification findings
<!-- Required by QG-009. What was found, what was fixed, before/after counts,
     real-repository observations. If nothing was found, state what was run to
     establish that — "none" alone fails the gate. -->

### Benchmark state
### Known limitations
### Rollback point
```
