# ADR-0010 — Provisional vs accepted checkpoint tags

**Status:** Accepted · 2026-08-19 · Clarifies RULES 016–018, 025

## Context

M00 exposed a defect in checkpoint mechanics. The tag `cartograph-m00` was
created at *internal completion* (per the M00 execution instructions), pointing
at `35ec77a`. The ledger commit recording that tag (`6c18820`) then necessarily
landed **after** the tag, so the pull request under review contained a commit
the checkpoint did not capture. A checkpoint that does not identify the exact
state under review — and cannot, because recording a tag's SHA in a ledger
always produces a commit outside the tag — is not reproducible in the sense
RULE 018 requires. Separately, tagging `cartograph-m00` before human acceptance
conflated "gates passed locally" with "milestone accepted" (RULE 025).

## Decision

Two distinguishable checkpoint classes:

1. **Provisional checkpoint — `cartograph-mNN-rcK`.** Created when a
   milestone's gates pass and its PR is (re)submitted for review. Points at the
   exact PR head. If review requires changes, `rcK+1` supersedes it; stale rc
   tags of an *unaccepted* milestone may be retired.
2. **Accepted checkpoint — `cartograph-mNN`.** Created only after the human
   owner accepts the milestone (merges the PR). Points at the exact accepted
   commit on `main` (the merge commit). **Never moved or deleted**, and once a
   milestone is accepted its final rc tag is also retained.

Ledger rule: the tag itself is the authority for its SHA. `CHECKPOINTS.md` and
`project-state.yaml` reference checkpoints **by tag name**; any SHA quoted for
the current provisional checkpoint is informational (`git rev-parse <tag>`
wins). This dissolves the self-reference problem that caused the M00 defect.

Applying the policy to M00: the premature `cartograph-m00` tag (at `35ec77a`,
one commit short of the reviewed state) is **retired** and the name reserved
for the accepted checkpoint at merge. Retirement determined safe before
execution: repository one day old, zero forks, zero releases, PR unmerged,
milestone unaccepted — the never-delete guarantee attaches at acceptance and
had not attached. `cartograph-m00-rc1` is created at the current PR head.

## Alternatives

- **Force-move `cartograph-m00` to the PR head** — rejected: moving a published
  tag silently changes what a name means; and the name would still prematurely
  claim acceptance while the repository says "pending human review".
- **Keep the premature tag and add a second "final" name later**
  (`cartograph-m00-final`) — rejected: permanently leaves a misleading tag that
  points into the middle of a PR, and gives accepted checkpoints an inconsistent
  naming scheme from M01 onward.
- **Tag-only, no ledger SHAs** — rejected: the ledger's human-readable history
  is worth keeping; making the tag authoritative fixes the staleness without
  losing it.

## Consequences

Milestone execution ends with an rc tag, an open PR, and a report — never with
the final tag. Creating `cartograph-mNN` becomes part of the *acceptance*
procedure (documented in CHECKPOINTS.md). CI and gates treat rc tags as valid
rollback points for unaccepted work.
