# M03 — Canonical route normalization

Target: Day 6 · Branch: `feature/m03-route-normalization` · Tag on acceptance: `cartograph-m03`

Scope: normalization of `/orders/:id`, `/orders/{id}`, `/orders/<int:id>`,
`/orders/${id}` to canonical `(METHOD, /orders/{*})` in `cartograph-resolver`.
Unknown segments stay `{unknown}` (RULE 009).

Acceptance: dialect table tests, property tests on idempotence; gates pass.

Implementation notes (recorded during execution):

- The canonical model keeps `Segment::Parameter` (declared by route syntax)
  distinct from `Segment::Dynamic` (a substituted value of unknown content).
  Flattening them would lose the difference between "this position accepts
  anything" and "I am supplying an unknown value", which M04 needs.
- `/orders/123` keeps `123` static. Only syntax declares a parameter; a
  numeric-looking value never becomes `{id}`.
- Trailing slashes are preserved rather than collapsed, since frameworks
  disagree about whether `/orders` and `/orders/` are one route.
- Route mount prefixes (`APIRouter(prefix=…)`, blueprints, Django `include()`)
  are deliberately not applied: composing them needs cross-file resolution and
  belongs with M04.
- `same_shape`/`compatible_method` are structural predicates only. No
  `CanonicalRoute` field can reference another observation, so a match is not
  expressible in the M03 model.

Standard exit: PR + CI green + self-review + human acceptance + checkpoint tag + state update + STOP.
