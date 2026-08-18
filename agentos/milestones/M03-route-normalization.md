# M03 — Canonical route normalization

Target: Day 6 · Branch: `feature/m03-route-normalization` · Tag on acceptance: `cartograph-m03`

Scope: normalization of `/orders/:id`, `/orders/{id}`, `/orders/<int:id>`,
`/orders/${id}` to canonical `(METHOD, /orders/{*})` in `cartograph-resolver`.
Unknown segments stay `{unknown}` (RULE 009).

Acceptance: dialect table tests, property tests on idempotence; gates pass.

Standard exit: PR + CI green + self-review + human acceptance + checkpoint tag + state update + STOP.
