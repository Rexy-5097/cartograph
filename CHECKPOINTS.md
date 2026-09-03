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

## M14 — GitHub PR integration

| Field | Value |
|---|---|
| Status | **ACCEPTED** |
| Accepted | 2026-09-03 by the project owner |
| Base | `cartograph-m13` — commit `5c34e16` |
| Contribution model | Open-source fork: `ronitsaha11/cartograph` → `Rexy-5097/cartograph` |
| Pull requests | [#33](https://github.com/Rexy-5097/cartograph/pull/33) · [#34](https://github.com/Rexy-5097/cartograph/pull/34) · [#36](https://github.com/Rexy-5097/cartograph/pull/36) · [#37](https://github.com/Rexy-5097/cartograph/pull/37) |
| **Accepted checkpoint** | **`cartograph-m14`** — merge commit `caba5ef`, immutable |
| Dogfood evidence | [#35](https://github.com/Rexy-5097/cartograph/pull/35) — kept as the record, closed rather than merged |

### Delivered in two slices, plus a correction the first live run forced

| Slice | PR | Merge | What landed |
|---|---|---|---|
| 1 — review content | #33 | `80b810d` | `cartograph diff --markdown`, the review the Action posts |
| 2 — delivery | #34 | `a401b41` | The workflow, the comment upsert, its 20 tests |
| correction | #36 | `c59f5a6` | Head tree fetched as an archive rather than checked out |
| prerequisite | #37 | `caba5ef` | `version::MILESTONE` and `current_milestone` advanced together |

### Acceptance criterion — the Action runs on Cartograph's own PRs

**PASS**, on [#35](https://github.com/Rexy-5097/cartograph/pull/35), a real pull
request from the fork.

| | run | result |
|---|---|---|
| first | [33733818164](https://github.com/Rexy-5097/cartograph/actions/runs/33733818164) | `created comment 5522919367` |
| second | [33734082898](https://github.com/Rexy-5097/cartograph/actions/runs/33734082898) | `updated comment 5522919367` |

Both `pull_request_target`, both green through all ten steps. A third dispatch on
#37 ([33735233760](https://github.com/Rexy-5097/cartograph/actions/runs/33735233760))
behaved identically.

**The workflow was sourced from `main`, and that is provable rather than assumed.**
The head commit `81c0285` carries its own copy of the workflow, whose step 8 reads
*"Check out the head tree"*. `main` reads *"Fetch the head tree"*. The run executed
**Fetch**. A fork cannot alter the workflow that reviews it.

**Every step did real work**, read from the runner log rather than inferred from a
green tick: base checked out at `ref: a401b419…`; `Finished release profile in
29.30s`, compiled from the base tree; `head tree: 663 files, no .git: yes`;
`review is 418 bytes`; `PR_NUMBER: 35`.

### Idempotency — one comment, updated

`id=5522919367`, author `github-actions[bot]`, **created `08:32:23Z`, updated
`08:35:41Z`**. Marker comments on the pull request after both runs: **exactly 1**.
No second comment was appended.

The comment is found by the hidden marker `<!-- cartograph-architecture-review -->`
alone — never by author or prose, so a person quoting the review is not mistaken
for it, and a duplicate would resolve to the lowest id rather than alternating
between runs.

### `pull_request_target`, and why it is safe here

A contributor pull request comes from a fork, and on `pull_request` a fork's token
is read-only: `pull-requests: write` cannot be granted and the comment 403s. The
milestone's own criterion would be unreachable for a fork contribution.

That trigger is dangerous when a workflow *executes* pull-request code. This one
does not, and the separation is the design:

- **The binary is built from the base commit.** `crates/cartograph-cli/build.rs` is
  the only build script in the workspace, so building the head would run a
  contributor's code. `cargo build` is step 5; the head arrives at step 7 — **the
  head does not exist on disk when the build runs.**
- **The head is data.** It is unpacked outside the Cargo workspace, whose `members`
  is an explicit `crates/*` list, so nothing there can be drawn into a build.
- **The analyser reads files and never executes them.** No `Command::new` in
  `cartograph-parser`, `-resolver`, `-pipeline`, `-graph` or `-core`.

### The first live run failed, and the failure improved the design

Run [33722281806](https://github.com/Rexy-5097/cartograph/actions/runs/33722281806)
failed at the head checkout: `actions/checkout` now refuses a fork's head under
`pull_request_target` unless `allow-unsafe-pr-checkout: true` — a guardrail aimed
at exactly the mistake this workflow was built to avoid.

The opt-in was **not** taken. #36 fetches the head as a tarball instead. It unpacks
with no `.git`, no remote and no stored credential, so *the head is data* became a
property of the filesystem rather than of our intentions, and the guardrail stays
armed for whoever edits the file next. `HEAD_REPO` and `HEAD_SHA` travel through
`env` rather than into the script text, because a fork's name is
contributor-controlled and contributor-controlled text must never become part of a
command. An empty unpack is a hard failure: a missing tree would diff as *every
relationship removed*, the most confident wrong review the workflow could post.

### Minimum scopes, verified from the runner

The scope line requires them, SECURITY.md records them as an M14 invariant, and the
runner reported the grant itself on both runs:

```
Contents: read
Metadata: read          # GitHub's implicit grant, not requested
PullRequests: write
```

Contents stays read-only: a review that could push is a review that could be turned
into one.

### No hosted infrastructure

The binary runs in the repository's own CI. No service, no backend, no analysis
logic in YAML — the workflow obtains two trees, runs `cartograph diff --markdown`,
and hands the result to a comment upsert. ROADMAP.md's business-model line holds:
*"Runs the binary in the user's own CI — no server, no hosting cost."*

### Leakage

Comment bodies from both runs match none of `ghp_ ghs_ gho_ ghu_ ghr_ C:\ /home/
/Users/ AppData scratchpad secret token runner/work` — **0 hits**. Both run logs
contain **0** token-shaped strings; every token mention is `GH_TOKEN: ***` or
`GITHUB_TOKEN: ***`. No runner-local absolute path reached a public comment.

### The 65,536-character limit

GitHub refuses a longer body, which would fail the run outright. Established by
controlled test, not by a live pull request: a 2,075,746-character, 20,009-line
review produces a **64,663-character** body — 661 rows emitted, **0 malformed**, no
unterminated fence, byte-identical across runs, and a notice saying 19,341 lines
were omitted and how to read the whole review locally. Whole lines are dropped, so
a table row can never be cut in half and misalign every column after it. Quietly
showing less would let a reader conclude a relationship was not reported when it
merely did not fit.

### Gate results

Run on Windows 11, `x86_64-pc-windows-msvc`, Rust 1.97.1, at `caba5ef`.

| Gate | Result |
|---|---|
| `cargo fmt --all -- --check` | PASS |
| `cargo clippy … -D warnings` | PASS — 0 warnings |
| `cargo test --workspace` | PASS — **824 passed, 0 failed, 25 ignored** |
| `node .github/scripts/review-comment.test.js` | PASS — **20/20** |
| QG-001 … QG-009 | PASS — 9/9 |
| AgentOS validator | PASS — 98/100 |
| CI on each pull request | PASS — 10 checks each |

### Verification findings

**The helper is dependency-free Node**, like the npm wrapper. A comment upsert does
not justify a framework, and a workflow with no lockfile cannot have one poisoned.
Its tests stub the GitHub API at `fetch`, the boundary the helper owns — which is
why the Action's real behaviour was established on a real pull request instead.

**A workflow cannot validate a change to itself.** `pull_request_target` is read
from the base branch, so #34 and #36 were both reviewed by the version of the
workflow they were replacing. #34's own review never ran; #36's failed at the step
it fixes. The first pull request after each merge is the first real test, and that
is a permanent property of the trigger rather than a defect.

### Accepted limitations

- **No reusable `action.yml`.** The acceptance sentence asks for the Action on
  Cartograph's own PRs, and that is what exists: a workflow in this repository.
  Packaging it so another project can consume it — the "user's CI" half of the
  scope line — is not done.
- **A populated relationship table has never been observed live.** Every dogfood
  pull request changed only Markdown or Rust, so each review correctly read *"No
  architectural change… 8 were compared."* The populated form is covered by the
  binary's own tests, not by a live run.
- **One page of 100 comments is searched.** Beyond that a second comment would be
  created rather than someone else's corrupted — a deliberate failure direction.
- **Truncation drops a suffix**, so on a very large review the last sections are the
  ones lost; it does not summarise.
- **No desktop integration.** MAP does not consume the review.
- **HARD GATE 4** — 7-day return under 15% after 30 days — is a product-usage clock,
  not an engineering criterion, and is the owner's to start. Its 30-day measurement
  period began only with M13's acceptance on 2026-09-03 and has not elapsed. M09's
  HARD GATE 3 was treated the same way.

## M13 — Structural DIFF

| Field | Value |
|---|---|
| Status | **ACCEPTED** |
| Accepted | 2026-09-03 by the project owner |
| Base | `cartograph-m12` — commit `d04e767` |
| Contribution model | Open-source fork: `ronitsaha11/cartograph` → `Rexy-5097/cartograph` |
| Pull requests | [#30](https://github.com/Rexy-5097/cartograph/pull/30) · [#29](https://github.com/Rexy-5097/cartograph/pull/29) · [#31](https://github.com/Rexy-5097/cartograph/pull/31) · [#32](https://github.com/Rexy-5097/cartograph/pull/32) |
| **Accepted checkpoint** | **`cartograph-m13`** — merge commit `5c34e16`, immutable |
| Scope records | [ADR-0014](docs/adr/ADR-0014-m10-scope-reconciliation.md) identity gate, satisfied |

### Delivered in three slices

| Slice | PR | What landed |
|---|---|---|
| 1 — identity | #30 | Stable cross-run `(kind, name, file)` identity beside `NodeId` |
| 2 — diff + CLI | #29 | Edge correspondence, `cartograph diff`, schema 1.2 |
| 3 — layout | #31 | `layout(prev_graph, new_graph, prev_positions)` |
| prerequisite | #32 | `version::MILESTONE` and `current_milestone` advanced together |

### The prerequisite ADR-0014 deferred twice

M13 could not begin with the diff. [ADR-0014](docs/adr/ADR-0014-m10-scope-reconciliation.md)
states it plainly — *"M13 may not assume stable identity exists"* — and M12 did
not deliver it. Two branches are two independent analyses, and nothing about a
handle survives between them: `NodeId` restarts at zero in each graph, so the
same number names a different artefact on each side.

**Identity is `(kind, name, file)`.** The line is deliberately excluded: including
it would make an edit anywhere above an artefact rename it, so every function
below a new import would read as removed-plus-added. It is a separate value
beside `NodeId`, which is unchanged and stays a graph-local handle.

**The five-point gate, satisfied on real repositories rather than fixtures.**
Two independent analyses of Airflow `9b43d6abc0fc` assigned identical
identities to all **3,104** nodes; Zulip `0ce8f6278cde`, all **1,308**. Zero
mismatches, and identity ordering differs from allocation ordering in both, so
identity is not a relabelling of the handle.

**Move is matched; rename is remove-plus-add, by policy.** Matching renames
means guessing from similarity, and a wrong guess produces a confidently wrong
diff — it claims one artefact evolved into another when the author split,
merged or replaced it.

### Acceptance criterion — a diff of two real branches, with evidence

**PASS.** Airflow `ed68491d8b` → `9b43d6abc0`, two revisions extracted
read-only:

**20 added · 5 removed · 3,304 unchanged**, and 3,304 + 20 equals the after
graph's edge count exactly. The result is legible as a real refactor:
`DagsFilters` stopped calling `useDagTagsInfinite` and
`useDagTimetableTypesInfinite` directly, and those calls appear in new
`TagsFilter` and `TimetableTypeFilter` components.

**Every reported edge carries evidence** — kind, provenance, confidence,
location and the observation itself. Zero of 25 entries lacked any of it.

**Correspondence is by identity, and the numbers show why that matters:**
**2,257 of 3,071** shared artefacts received a different `NodeId` between the
two analyses. Under handle correspondence nearly the whole graph would have
read as churn.

**The changed classification needed real source to exercise.** The upstream
pair produced no changed edges, so one line — `method: 'GET',` — was removed
from a client call. The artefact still requests the same endpoint, so the
relationship holds; what weakens is the evidence *about the method*. Same
source identity, same target identity, same kind, confidence **0.98 → 0.784** —
exactly `UNDECLARED_METHOD_FACTOR`. **0 added, 0 removed, 1 changed, 3,323
unchanged.** Not a contrived path: the unmodified corpus already contains 17
edges at 0.784.

### Mental-map-stable layout

The scope line's second half, delivered as a tested API. `layout` promises
determinism and explicitly not stability, so a second revision moves everything.
`layout_stable` holds artefacts where the reader last saw them.

Across the same Airflow pair: **3,071 survivors, 9 added, 3 removed — and all
3,071 survivors kept their exact positions, where a plain layout would have
moved every one of them.** 3,080 positions reproduced identically on a repeat.

Cluster ids come from the new graph, and a preserved coordinate outside the
extent is refused rather than returned, because every record must satisfy the
layout contract.

### Gate results

Run on Windows 11, `x86_64-pc-windows-msvc`, Rust 1.97.1, at `5c34e16`.

| Gate | Result |
|---|---|
| `cargo fmt --all -- --check` | PASS |
| `cargo clippy … -D warnings` | PASS — 0 warnings |
| `cargo test --workspace` | PASS — **813 passed, 0 failed, 25 ignored** |
| QG-001 … QG-009 | PASS — 9/9 |
| AgentOS validator | PASS — 98/100 |
| CI on each slice | PASS — 8 checks each |

### Verification findings

**`NodeId` semantics are unchanged.** It remains a graph-local handle; identity
is a separate value. Nothing carries a handle between two graphs — `NodeId` does
not appear as a value in either the diff engine or the stable layout.

**Schema 1.2 is additive.** The 1.1 `trace` and `blast` branches are preserved
byte-identically as `oneOf[0..2]`, the command enum gains only `diff`, and a
test asserts a trace document still matches only its own branch. `minItems` was
implemented in the conformance validator rather than dropped from the schema.

**M10 and M12 are untouched.** Canonical equality holds — the differential suite
passes unchanged at its recorded counts, and blast radius results are unmoved.

### Accepted limitations

- **Graph construction merges some artefacts.** `node_for` deduplicates on
  `(kind, name, file)`, so two same-named methods in different classes of one
  file are a single node before identity is computed. The diff inherits that and
  cannot distinguish what was never distinguished. Changing it would move M10's
  differential oracle and M12's blast results.
- **`provenance` and `file` changes are unit-tested, not observed.** Provenance
  is fixed per resolver path, and `file` moves with the source node's identity,
  so neither varies naturally for a surviving edge.
- **The changed-edge case is a synthesised one-line edit** to a real repository,
  not two upstream commits.
- **The stable layout has no desktop consumer.** The API is tested; no MAP user
  sees the benefit yet.
- **Real-corpus tests are opt-in** and do not run in CI.
- **HARD GATE 4** — 7-day return under 15% after 30 days — is a product-usage
  clock, not an engineering criterion, and is the owner's to start. M09's
  HARD GATE 3 was treated the same way.

## M12 — Blast radius

| Field | Value |
|---|---|
| Status | **ACCEPTED** |
| Accepted | 2026-09-02 by the project owner |
| Base | `cartograph-m11` — commit `17f114e` |
| Contribution model | Open-source fork: `ronitsaha11/cartograph` → `Rexy-5097/cartograph` |
| Pull requests | [#24](https://github.com/Rexy-5097/cartograph/pull/24) · [#25](https://github.com/Rexy-5097/cartograph/pull/25) · [#26](https://github.com/Rexy-5097/cartograph/pull/26) · [#28](https://github.com/Rexy-5097/cartograph/pull/28) |
| **Accepted checkpoint** | **`cartograph-m12`** — merge commit `d04e767`, immutable |
| Scope records | [ADR-0018](docs/adr/ADR-0018-blast-radius-confidence.md) |

### Delivered in three slices

| Slice | PR | What landed |
|---|---|---|
| 1 — core | #24 | Reverse reachability over `edges_to`, bottleneck relaxation, `is_dependency` allow-list |
| 2 — CLI | #25 | `cartograph blast`, schema 1.1 (`oneOf` on `command`), exit codes |
| 3 — desktop | #26 | Tauri command, highlight projection, find-by-name |
| prerequisite | #28 | `version::MILESTONE` and `current_milestone` advanced together |

### The decision the milestone was really about

**Confidence along a path is the weakest step, not the product.** A five-hop
chain of 0.98 edges multiplies to 0.90, and of 0.80 edges to 0.33 — which would
rank a long certain chain below a short doubtful one and make depth, not
evidence, the thing the number measures. [ADR-0018](docs/adr/ADR-0018-blast-radius-confidence.md)
takes the **minimum along a path, maximised across paths**: a route is as strong
as its weakest link, and an artefact is scored by its best-supported route.

**The number is not a probability and says so.** `calibrated: false` travels
with every payload on both surfaces rather than living in a surface's memory.
M08 measured ECE 0.18, with 79% observed at a stated 0.80; RULE 009 and that
finding point the same way.

**One representative route is reported per artefact,** not every route. The
field is named `routes: "representative"` so no reader concludes the set is
exhaustive.

**The clients decide nothing.** `cartograph-desktop` and the CLI each call
`blast_radius` once and shape the result; there is no traversal, no confidence
arithmetic and no edge-kind policy in either (RULE 002). The desktop answer was
checked against the core's exhaustively, entry by entry, on real repositories.

### Acceptance criterion — cross-stack impact sets with evidence

**PASS.** Apache Airflow at `9b43d6abc0fc` (3,104 nodes · 3,350 edges), target
`Connection` — the Python class, not the `connection` table or the function
that shares its name:

**767 impacted artefacts, maximum depth 5, of which 9 are TypeScript** — 7
`.ts` and 2 `.tsx`, at depths 2–5: `AddConnectionButton.tsx` and
`TestConnectionButton.tsx`, `postConnection`/`testConnection` in
`services.gen.ts`, the `useAddConnection`/`useTestConnection` hooks, and
`connections.spec.ts`. The remaining 758 are Python. A query rooted in a
Python class reaches React components through the HTTP boundary, which is the
cross-stack claim the milestone exists to make.

**Every impacted artefact carries evidence.** All 767 `via` ids resolve to
edges in the same document — none dangling — each with kind, provenance,
confidence and location.

Zulip at `0ce8f6278cde` (1,308 · 1,469), target `Stream`: **113 impacted,
maximum depth 1**, all Python, all `via` resolving. The depth is 1 because
every dependent is a direct ORM access, not because traversal stopped early.

**The three surfaces agree exactly.** Desktop equals core entry for entry —
Airflow `Connection` 767 (core 767), `DagRun` 107 (core 107); Zulip `Stream`
113 (core 113) — and the CLI reports the same sets.

### Gate results

Run on Windows 11, `x86_64-pc-windows-msvc`, Rust 1.97.1, at `d04e767`.

| Gate | Result |
|---|---|
| `cargo fmt --all -- --check` | PASS |
| `cargo clippy … -D warnings` | PASS — 0 warnings |
| `cargo test --workspace` | PASS — **745 passed, 0 failed, 18 ignored** |
| frontend typecheck · test · build | PASS — 85 tests |
| QG-001 … QG-009 | PASS — 9/9 |
| AgentOS validator | PASS — 98/100 |
| CI on `d04e767` | PASS — all 8 checks |

### Verification findings

**No cross-run identity was introduced.** M12 runs over one graph, which
ADR-0011's within-graph identity already serves; `crates/cartograph-core/src/id.rs`
is unchanged. ADR-0014's gate stands untouched for M13.

**A selection cannot outlive its graph.** `NodeId` restarts at zero each
analysis, so a node id held across a re-analysis would name a real but
different artefact — a confident wrong answer rather than an error. The desktop
must present the `AnalysisId` it was looking at; a mismatch is refused as
`StaleSelection`, an absent node as `UnknownNode`, and neither is ever reported
as an empty radius. An empty radius is a true answer and stays distinguishable.

**Highlighting moves nothing.** The blast overlay sets attributes on the loaded
graph and never writes `x`, `y` or `cluster`; layout, topology, order and size
hashes are identical before and after, asserted in the suite and observed on
Airflow in the real window.

### Accepted limitations

- **Confidence is an uncalibrated prior**, not a likelihood, and must not be
  thresholded as one. M08's ECE of 0.18 is unchanged by this milestone.
- **One representative route per artefact.** Other routes may exist and are not
  enumerated.
- **Coincident geometry.** Where many edges converge on one node, a *particular*
  highlighted route edge can be hard to click; every click still selects a real
  edge and returns that edge's evidence. Pre-existing layout behaviour, not
  introduced here.
- **A node can be unclickable.** In Airflow the class `Connection` sits 0.19px
  from a larger neighbour whose hit disc contains it at every zoom, which is why
  Slice 3 added find-by-name. The CLI could always address it.
- **The M11 renderer benchmark harness stalls** on its own `afterRender`
  listener ordering; documented in `docs/benchmarks/m11-renderer.md` and not
  fixed here.

## M11 — Desktop application, MAP

| Field | Value |
|---|---|
| Status | **ACCEPTED** |
| Accepted | 2026-09-01 by the project owner |
| Base | `cartograph-m10` — commit `8690ebf` |
| Contribution model | Open-source fork: `ronitsaha11/cartograph` → `Rexy-5097/cartograph` |
| Pull requests | [#18](https://github.com/Rexy-5097/cartograph/pull/18) · [#19](https://github.com/Rexy-5097/cartograph/pull/19) · [#21](https://github.com/Rexy-5097/cartograph/pull/21) · [#22](https://github.com/Rexy-5097/cartograph/pull/22) · [#23](https://github.com/Rexy-5097/cartograph/pull/23) |
| **Accepted checkpoint** | **`cartograph-m11`** — merge commit `17f114e`, immutable |
| Scope records | [ADR-0015](docs/adr/ADR-0015-layout-contract.md) · [ADR-0016](docs/adr/ADR-0016-desktop-boundary.md) · [ADR-0017](docs/adr/ADR-0017-render-scene.md) |

### Delivered in four slices

| Slice | PR | What landed |
|---|---|---|
| 1 — layout contract | #18 | Deterministic Rust layout emitting `{id, x, y, cluster}` |
| 2 — desktop foundation | #19 | `cartograph-pipeline` extracted from the CLI binary; testable session; Tauri v2 shell; repository lifecycle |
| 3 — renderer | #21 | Graphology + Sigma.js v3; the 10k measurement |
| 4 — evidence | #22 | Edge click opens the evidence record; `AnalysisId` stale-selection guard |
| prerequisite | #23 | `version::MILESTONE` and `current_milestone` advanced together |

### The architecture the milestone was really about

M11 added a **second client**, not a second implementation. ADR-0001 claimed the
CLI, desktop and MCP server are peers of one core; that was not quite true,
because the *sequence* turning a directory into a graph lived in
`cartograph-cli`, a binary crate nothing can depend on. Slice 2 moved it to
`cartograph-pipeline` verbatim, so the claim became true rather than aspirational.

```
Rust analysis → Rust layout → {id,x,y,cluster} → Scene → Tauri → React → Graphology → Sigma
                                                                                    (drawing only)
```

**Layout is computed in Rust and copied without arithmetic.** Both sides assert
bit equality rather than a tolerance, because a tolerance would pass a frontend
transform that scaled every position by 1.0000001 — which is precisely the
defect that would quietly falsify ADR-0001 and uncontain the renderer swap
ADR-0006 protects.

**Evidence is a lookup, not part of the render payload.** ADR-0017 kept
confidence, provenance and evidence out of the scene so ten thousand nodes do
not carry data nothing draws; the record is fetched for the one edge clicked.

**Stale selection is refused at the Rust boundary.** `EdgeId` is graph-local and
restarts at zero per analysis (ADR-0011), so an id held across a re-analysis
almost certainly still *exists* in the new graph — as a different relationship.
A naive lookup would return confident, well-formed, wrong evidence. Every
analysis is stamped with an `AnalysisId` and a mismatch is refused. The window
also clears its selection, but a guarantee that depends on a client remembering
to do something is not a guarantee.

### Acceptance criterion 1 — MAP answers "what is this system?"

**PASS — human owner judgement**, on Apache Airflow at `9b43d6abc0fc`:
**3,104 nodes · 3,350 edges · 313 clusters**.

The cross-stack claim the product exists to make is visible: **175 `HttpCall`
edges** from TypeScript in `ui/openapi-gen/` into FastAPI handlers
(`get_assets`, `create_asset_event`, `materialize_asset`), **60 tables** reached
through 60 `Queries` edges, and clusters named after the real architecture —
`api_fastapi/core_api/routes/public` (89), `airflow/models` (73),
`ui/openapi-gen/queries` (453), `ui/src/queries` (63).

**Not every benchmark repository produces a map this rich, and that is recorded
rather than smoothed over:**

| Corpus | Commit | Nodes | Edges | Clusters | Result |
|---|---|---:|---:|---:|---|
| Apache Airflow | `9b43d6abc0fc` | 3,104 | 3,350 | 313 | **satisfies the criterion** |
| Zulip | `0ce8f6278cde` | 1,308 | 1,469 | 32 | thin |
| full-stack-fastapi | `162344da111e` | 2 | 1 | 2 | thin |

**Zulip is thin because of resolver coverage, not rendering.** It has **zero
route nodes**, one table, and 11 `HttpCall` edges most of which are test
utilities, because it registers URLs through `rest_path(...)` — a project-local
helper `benchmarks/corpus.json` already records as unsupported. There are no
route nodes for the desktop to draw. **full-stack-fastapi** yields 2 nodes for
the same class of reason: M09's "23 routes" were *observations* that never
became matched edges.

The same desktop draws all three faithfully. The limitation is inherited M07/M08
coverage, correctly attributed — which does not make it acceptable, only
correctly located.

### Acceptance criterion 2 — 10,000 nodes at 60 FPS

**PASS**, measured over **six foregrounded runs** of the real Tauri window on
the deterministic fixture (**10,000 nodes · 11,444 edges · 115 clusters**),
generated by the real layout engine and the real `scene::compose`.

| Run | Median | p95 | Worst | Frames | Over 16.67 ms |
|---|---:|---:|---:|---:|---:|
| 1 | 9.60 | 13.00 | 39.80 | 993 | 1.31% |
| 2 | 9.40 | 13.60 | 44.20 | 997 | 1.10% |
| 3 | 9.80 | 13.70 | 40.10 | 966 | 0.83% |
| 4 | 10.00 | 14.30 | 46.80 | 925 | 1.41% |
| 5 | 7.60 | 11.50 | 37.90 | 1,228 | 0.49% |
| 6 | 6.70 | 11.10 | 31.50 | 1,326 | 0.08% |

**Every median (6.70–10.00 ms) and every p95 (11.10–14.30 ms) sits inside the
16.67 ms budget**, across runs spanning idle and loaded machine states.

**A correction is recorded here deliberately.** An earlier report explained a
6.1 ms median as a vsync floor on a 165 Hz display and inferred unbounded
headroom. That was wrong: the median moved between 6.70 and 10.00 ms across
these runs, which it could not if it were hard-bound to the refresh interval.
Headroom is finite. The superseded reasoning is corrected in
`docs/benchmarks/m11-renderer.md` rather than quietly deleted.

Environment: Intel Core 7 240H · 23.6 GB · Windows 11 26200 · WebView2/Chromium
151 · Tauri 2.11.5 · Sigma 3.0.3 · Graphology 0.26.0 · **debug build** · 165 Hz.

### Gate results

Run on Windows 11, `x86_64-pc-windows-msvc`, Rust 1.97.1, at `17f114e`.

| Gate | Result |
|---|---|
| `cargo fmt --all -- --check` | PASS |
| `cargo check --workspace --all-targets` | PASS |
| `cargo clippy … -D warnings` | PASS — 0 warnings |
| `cargo test --workspace` | PASS — **690 passed, 0 failed, 14 ignored** |
| frontend typecheck · test · build | PASS — 43 tests |
| npm wrapper tests | PASS |
| `cargo bench --workspace --no-run` | PASS |
| QG-001 … QG-009 | PASS — 9/9 |
| AgentOS validator | PASS — 98/100 |
| CI on `17f114e` | PASS — all 8 checks, including both desktop-shell jobs |

### Verification findings

**M09 and M10 semantics are unchanged.** differential 23 · access_cache 12 ·
atomicity 7 · read_dependencies 56 · layout_contract 19 · imports 15 · orm 38 ·
cross_stack 12 · full_stack 5 · matching 38 · routers 12 ·
dynamic_resolution 15 · evaluator 21 · symbolic 18 · client_trace 10 ·
canonicalization 59 — every suite at its recorded count.

**No stable cross-run identity was introduced.** `crates/cartograph-core/src/id.rs`
differs from `cartograph-m09` by documentation only. The `AnalysisId` added in
Slice 4 is a within-process generation counter, explicitly not persisted and
meaningless across runs; ADR-0014's gate is untouched.

**Security.** No network, no credentials, no source upload, no source logging,
no absolute path in any panel or payload (asserted on the serialised record).
`Command::new` appears only in `build.rs`, capturing the git commit at build
time, and in tests invoking the CLI binary — never at runtime, and never
constructed from a repository path.

**Two defects were found by the work rather than by review**, and both are
recorded because the class matters more than the instance: Vite watched
`src-tauri/target` and killed `tauri dev` with an unrelated-looking error; and
the development-only benchmark was being bundled into production builds because
guarding the JSX is not enough — the guard has to wrap the dynamic import. CI
now asserts the second.

### Accepted limitations

- **Worst frames of 31–47 ms occur**, roughly one in a hundred, and are visible
  stutter. Median and p95 are inside budget; the tail is not.
- The 10k measurement is a **debug build on one machine** with a 165 Hz display.
  A 60 Hz panel would produce a different-looking median for the same renderer.
- **Zulip and full-stack-fastapi produce thin maps** for documented resolver
  coverage reasons, not rendering ones. MAP was judged on Airflow.
- **The Windows MSI bundle cannot be produced**: `tauri.conf.json` lists only
  `icons/icon.png`, while the bundler requires a `.ico` — which exists at
  `src-tauri/icons/icon.ico` but is not declared. The application binary builds
  and runs; only the installer does not. Deliberately not fixed during the
  acceptance run.
- The evidence panel is tested through its **pure presentation helpers and the
  Rust contract**, not a rendered DOM.
- The desktop shell sits **outside `cargo test --workspace`** and is covered by
  a dedicated CI job (ADR-0016). No Windows desktop CI — Windows is validated
  locally only.
- **No stable cross-run node identity.** M13 requires it; ADR-0014 states the
  gate.
- Evidence is fetched per click; a future multi-select would want batching.
- M08's measurement limits carry forward unchanged — confidence remains an
  uncalibrated prior, and the panel says so from the data.

---

## M10 — Incremental analysis engine

| Field | Value |
|---|---|
| Status | **ACCEPTED** |
| Accepted | 2026-08-31 by the project owner |
| Base | `cartograph-m09` — commit `e8416f9` |
| Branch | `feature/m10-incremental-engine` · PR [#17](https://github.com/Rexy-5097/cartograph/pull/17) (merged `8690ebf`) |
| Provisional checkpoint | none — ADR-0010: none was needed |
| **Accepted checkpoint** | **`cartograph-m10`** — commit `8690ebf`, immutable |
| Scope record | [ADR-0014](docs/adr/ADR-0014-m10-scope-reconciliation.md) — **Accepted** |

> **The milestone's scope line is wider than what shipped.** Of three named
> scope items, one was built. ADR-0014 records the difference rather than
> letting the milestone close over it, and is accepted as the governing record.
> Read it before treating the milestone file's scope line as delivered.

### Scope delivered

A parse fact cache keyed by content hash; recorded semantic read-dependency
tracking threaded through the resolver; a per-file Stage C ORM access cache
with dependency-aware reuse; and failure-safe publication. The invariant
throughout is `incremental_result == clean_rebuild_result`, checked by
canonical semantic graph equality, with the clean rebuild **retained as the
oracle** rather than replaced — a cache that becomes the only implementation
cannot be checked against anything.

A Stage C entry is reusable only when *all* hold: its recorded resolution
context is `complete`; its own content identity matches; the model-set
fingerprint matches; the path set matches *if it was consulted*; the alias set
matches *if it was consulted*; and every recorded read still exists with the
same content identity. There is no partial reuse. Completeness is a one-way
switch — a resolution that could not fully record what it read is never reused.

### Deferred, and not claimed (ADR-0014)

| Item | Evidence it is absent |
|---|---|
| content-hash node identity | `crates/cartograph-core/src/id.rs` is byte-identical to `cartograph-m09` |
| notify file watching | no `notify` dependency in `Cargo.lock` |
| local graph database (ROADMAP 0.2) | no `redb` dependency in `Cargo.lock` |
| broader global-index caching | measured, then deliberately declined — see below |
| `<250 ms` per change | **superseded**: never adopted as a criterion |

**Stable cross-run node identity carries a forward obligation.** ADR-0011
deferred it *to M10*; M10 deferred it again, because the cache keys per-file
results by content and dependency identity and never by `NodeId`, and canonical
equality compares semantic identity rather than handles. Checked against the
milestone definitions: **M13 (structural diff) requires it unconditionally** —
two branches are two independent analyses, and
`layout(prev_graph, new_graph, prev_positions)` is defined as a correspondence
between them. **M12 (blast radius) does not**, by its stated acceptance:
reverse reachability runs over one graph, which ADR-0011's within-graph
identity already serves. ADR-0014 states the five-point gate the future
identity slice must pass.

### Why the remaining rebuilds are not cached

Each was measured before a decision was taken, not assumed:
`ExportedConstants::collect` 1.8 ms, `ModuleIndex::build` 0.29 ms,
`RouteIndex::build` 0.032 ms, `RouterIndex::build` 0.005 ms — against a Stage C
cost of 894.8 ms. Caching any of them buys the machinery and not the benefit.
`ModuleIndex` was investigated in depth as the most plausible candidate: its
`by_path` field derives nothing, and its `aliases` field decomposes into a
per-file contribution plus a deterministic **stable** merge. Tests pin that
decomposition, so a future change adding derived state fails loudly rather than
silently invalidating the decision.

### Gate results

Run on Windows 11, `x86_64-pc-windows-msvc`, Rust 1.97.1, at `8690ebf`.

| Gate | Result |
|---|---|
| `cargo fmt --all -- --check` | PASS |
| `cargo check --workspace --all-targets` | PASS |
| `cargo clippy --workspace --all-targets --all-features -- -D warnings` | PASS — 0 warnings |
| `cargo test --workspace` | PASS — 622 passed, 0 failed, 10 ignored |
| `benchmarks/m08/test_calibration.py` | PASS — 23 tests |
| `cargo bench --workspace --no-run` | PASS |
| QG-001 … QG-009 | PASS — 9/9 |
| AgentOS validator | PASS — 98/100, 3 warnings |
| CI on `8690ebf` | PASS — all six required checks |

GNU `make` is unavailable on this Windows machine, so `make check`, `make
gates` and `make validate` were run as their documented equivalents: the cargo
targets above, `python agentos/tools/scripts/run_gates.py`, and `python
agentos/tools/scripts/validate_agentos.py`.

### Verification findings

**M09 semantics are unchanged.** All eleven M09 resolver suites pass with their
M09 counts: imports 15 · orm 38 · cross_stack 12 · full_stack 5 · matching 38 ·
routers 12 · dynamic_resolution 15 · evaluator 21 · symbolic 18 ·
client_trace 10 · canonicalization 59. ADR-0013 permitted resolver change
provided M09's answers did not move; they did not. The CLI surface is
identical — the `Command` enum matches `cartograph-m09` exactly, and the only
change to `main.rs` is a single `mod incremental;` declaration.

**Correctness.** 42 dedicated tests (23 differential · 12 access-cache · 7
atomicity) plus 56 resolver dependency tests, every one comparing against a
clean rebuild through canonical semantic equality — which excludes `NodeId`,
`EdgeId`, insertion order and `created_at`, and nothing else. Every mutation
class is covered: no-op, edit, create, delete, rename, dependency change,
transitive dependency change, path-set change, model ambiguity, alias change,
unrelated change, revert, recreate, multiple mutations, malformed and repaired.

**Real-repository mutation differential, at pinned commits.** Canonical
checkouts are never mutated: analysable files are copied into a disposable
temporary tree, mutated there, and removed on drop.

`zulip` (`0ce8f627`) — 1,539 files, 1,308 nodes, 1,469 edges, 1,067 bundles.
Fourteen mutations, every one `incremental == clean`, and **every miss
attributable to a dependency term**: 20 on a model-file edit (the file plus the
19 bundles recording it as a read); 246 on any `.py` create or delete (the 245
path-set consumers plus the new file); ~1,068 on any model-set change, coarse
by construction because the model set is global. Creating a colliding path
takes the graph to 1,263/1,421 and deleting it restores the baseline exactly;
a competing model *adds* two nodes while removing a resolved access — a cache
assuming "new file = more facts" would be wrong in both directions.

`airflow` (`9b43d6abc0fc`) — 917 files, 920 nodes, 848 edges. Seven mutations,
all equal to clean. Retargeting or removing the Vite aliases collapses the
graph by 702 semantic differences and restoring returns it exactly, while
**Stage C misses stay at zero throughout**: the 13 Python bundles never consult
the alias list, so no TypeScript or alias mutation may invalidate them. That is
the language separation proven on a real repository rather than asserted.

**Failure atomicity.** `analyze` had no failure path, so one was built in order
to be tested. Per-file results are computed into a local map; `self.entries =
fresh` is the single publication point; a failure returns before it and the
previous generation is untouched. `published` records whether an attempt became
state, so no combination of hit and miss counts can imply success after an
abandoned update. On zulip, failure injected **500 entries into a 1,068-entry
recomputation** leaves the generation at 1,067 with `published=false`; the
retry equals a clean rebuild; reverting restores 1,308/1,469 exactly. Building
that test found a real mistake first — it originally edited one model file,
which invalidates only about twenty entries, so a failure at 500 never fired
and the test passed vacuously.

**Performance is recorded as observation, not as a criterion.** Stage C 894.8 →
4.34 ms; resolve total 950.9 → 60.4 ms on zulip. A warm run now spends 318.7 ms
of 379 ms (84%) in filesystem walk, read and hash — the floor of any
content-addressed design, escapable only with file watching. The claim is
**work avoided** (1,067 hits / 0 misses on an unchanged tree), not
milliseconds; absolute wall-times on this machine varied several-fold between
sessions with page-cache state.

**Real-repository evidence is recorded, not re-run at acceptance.** The pinned
zulip and airflow mirrors live outside the repository and are not vendored; on
the acceptance machine they were unavailable, so all ten opt-in harnesses
(three real-repository, one warm-cache baseline, one profile, five resolver
real-repository tests) were skipped rather than run. The tables above are the
evidence recorded when those suites were executed during development, retained
in `docs/development/incremental-engine.md`. Nothing was re-measured at
acceptance and nothing is claimed to have been.

### Accepted limitations

- **Both caches are process-local.** The CLI exits after each run, so it cannot
  benefit from either across invocations — every measured speedup is
  in-process. Persistence is deferred.
- `DefaultHasher` (SipHash-1-3) is an **internal invalidation identity only**;
  it is not stable across Rust releases and is unsuitable for an on-disk
  format. Persistence would need a pinned hash, a schema version and a
  corruption story.
- **Failure atomicity is proven at one injected boundary.** Failures inside
  `discover_accesses` or graph construction are not injected, because no
  current path there returns an error. Publication atomicity is what this
  proves; arbitrary internal failure coverage is not claimed.
- Model-set invalidation is coarse by construction — any model appearing or
  disappearing invalidates every bundle.
- Neither real repository exercises both mechanisms at full scale: airflow's
  checkout is the UI subtree with 13 Python files, and zulip has no build
  aliases.
- **Stable cross-run node identity does not exist**, and is not claimed to.
- M08's measurement limits carry forward unchanged; M10 shipped no accuracy
  change.

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
