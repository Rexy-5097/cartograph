# M05 — Dynamic URL template evaluator

Target: Day 11 · Branch: `feature/m05-template-evaluator` · Tag on acceptance: `cartograph-m05`

Scope: restricted symbolic evaluation over string values — constant propagation
through module scope, template concatenation, `{unknown}` for unresolvable
segments. `template-evaluation` provenance, reduced confidence.

Acceptance: `const BASE='/api/v1'; \`${BASE}/orders/${id}\`` →
`/api/v1/orders/{*}`; never guesses (RULE 010); gates pass.

Implementation notes (recorded during execution):

- Built in verified slices per RULE 026: symbolic model, then expression
  evaluation, then cross-file constants, then reconstruction and integration.
- The symbolic model came *first*, ahead of literal propagation, because every
  later slice depends on unresolved values being unrepresentable as text.
- M05 adds no confidence tier. A resolved URL flows through the unchanged M04
  matcher and earns the prior its evidence supports, which is what stops
  "resolve more URLs" from becoming "produce more matches".
- Real-repository result: zero additional matches across four corpora. Those
  projects build URLs from runtime-configured SDK clients, not module
  constants. Refusing is correct; the limit is on reach, not correctness.

Standard exit: PR + CI green + self-review + human acceptance + checkpoint tag + state update + STOP.
