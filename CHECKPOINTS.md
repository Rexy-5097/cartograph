# Checkpoints

Every completed milestone produces a reproducible checkpoint: a commit, an
annotated tag, a gate result and a rollback point.

**Milestone tags are never deleted, moved or rewritten.** If a later milestone
breaks the project, the previous checkpoint must still be recoverable. That
guarantee is the reason this file exists.

## Rolling back

```bash
git checkout cartograph-mNN     # inspect a checkpoint
git switch -c fix/from-mNN cartograph-mNN
```

Tags follow `cartograph-mNN`.

---

## M00 — Engineering foundation

| Field | Value |
|---|---|
| Status | **PENDING HUMAN REVIEW** |
| Branch | `feature/m00-foundation` |
| Tag | `cartograph-m00` *(created after the pull request is accepted)* |
| Commit | *(recorded on tag creation)* |
| Specification | Frozen Engineering Spec V3 |

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
| QG-001 … QG-008 | *(recorded on merge)* |
| CI | *(recorded on merge)* |

### Known limitations

- No parsing, no resolution, no storage. `cartograph-parser` and
  `cartograph-resolver` are empty by design.
- Node identity is graph-local and not content-addressed (M10).
- No benchmark results exist. Criterion is wired up; no number is published.
- Confidence values are uncalibrated priors (M08).

### Rollback point

This tag. There is no earlier checkpoint — M00 is the first.

---

## Template for subsequent entries

```markdown
## MNN — <title>

| Field | Value |
|---|---|
| Status | PASS / FAIL |
| Date | YYYY-MM-DD |
| Commit | <sha> |
| Tag | cartograph-mNN |
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
