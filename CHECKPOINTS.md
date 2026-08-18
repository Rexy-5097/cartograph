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
| fmt / clippy / tests / CI / QG-001…008 | |

### Benchmark state
### Known limitations
### Rollback point
```
