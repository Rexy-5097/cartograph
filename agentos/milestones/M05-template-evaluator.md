# M05 — Dynamic URL template evaluator

Target: Day 11 · Branch: `feature/m05-template-evaluator` · Tag on acceptance: `cartograph-m05`

Scope: restricted symbolic evaluation over string values — constant propagation
through module scope, template concatenation, `{unknown}` for unresolvable
segments. `template-evaluation` provenance, reduced confidence.

Acceptance: `const BASE='/api/v1'; \`${BASE}/orders/${id}\`` →
`/api/v1/orders/{*}`; never guesses (RULE 010); gates pass.

Standard exit: PR + CI green + self-review + human acceptance + checkpoint tag + state update + STOP.
