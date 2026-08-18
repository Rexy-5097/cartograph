# M14 — GitHub PR integration

Target: Week 19 · Branch: `feature/m14-github-pr-integration` · Tag on acceptance: `cartograph-m14`

Scope: GitHub Action running the binary in the user's CI, posting an
architecture review comment on PRs. Minimum scopes.

Acceptance: Action runs on Cartograph's own PRs; no hosted infrastructure;
gates pass.

Standard exit: PR + CI green + self-review + human acceptance + checkpoint tag + state update + STOP.
