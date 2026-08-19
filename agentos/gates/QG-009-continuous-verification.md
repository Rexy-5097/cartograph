# QG-009 — Continuous verification

**Enforces:** [RULE 026](../../PROJECT_RULES.md) ·
[Continuous Verification Protocol](../../docs/development/continuous-verification.md)

A milestone is not verified because its tests pass. It is verified when the
work was *checked as it was built*, adversarially, against real input — and
when the evidence of that checking survives in the repository.

This gate is deliberately not a file-existence check. Each condition below can
fail on a milestone that compiles, passes every other gate, and is wrong.

## What it checks

### 1. The current milestone has tests of its own

Read from `agentos/artifacts/project-state.yaml`. The milestone's declared test
files must exist and contain tests. A milestone that added behaviour without
adding tests fails here.

### 2. Negative tests are present, in proportion

At least **one quarter** of the current milestone's tests must be negative —
asserting that something is *refused*: no edge, no match, unsupported, not
guessed, stays unknown. Detected from assertion content, not from test names
alone.

The ratio matters. Cartograph's failure mode is not "the feature does not
work", it is "the feature claims something it should have refused", and only
negative tests catch that.

### 3. Standing invariants are covered

Tests must exist asserting the invariants that hold across the whole product:

| Invariant | Recognised by |
|---|---|
| Ambiguity produces no accepted edge | `Ambiguous` + no-edge assertion |
| An unknown method never becomes GET | a test naming GET-defaulting |
| Unknown values stay unknown | a test asserting a value is not resolved |
| Accepted edges always carry evidence | an evidence assertion on an edge |

Only invariants relevant to what the repository currently implements are
required; a milestone before the resolver existed is not asked for edge
invariants.

### 4. High-risk milestones show real-repository validation

For M04–M08, M10, M12 and M13, the milestone's checkpoint entry must record
validation against real repositories — named corpora with counts, not a claim
that it was done.

### 5. Verification findings are recorded

The current milestone's entry in `CHECKPOINTS.md` must contain a
**Verification findings** section with substantive content. "None" alone
fails: the section must state what was tested to establish that result.

### 6. No unresolved verification failure

The findings section must not leave a discovered defect open without either a
fix or an explicitly recorded limitation.

## Running it

```bash
make gates          # QG-001 … QG-009
```

## Failure means

Stop. Do not open the pull request. Add the missing verification, or record
honestly what was not verified and why — an acknowledged gap is a finding, a
silent one is a defect.
