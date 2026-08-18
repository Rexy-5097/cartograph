# Cross-language route matching (M04)

Implemented in [`cartograph-resolver`](../../crates/cartograph-resolver/)
(`matching.rs`, `edge.rs`). The first stage permitted to relate **two**
observations, and the first to create a semantic edge.

```
client canonical route ─┐
                        ├─► candidates ─► compatibility ─► classify ─► EDGE
backend route index ────┘
```

## Candidate generation

Comparing every client call against every route is quadratic. Routes are
bucketed by **segment count** — the cheapest property that must agree for any
non-wildcard match — with wildcard routes held separately and consulted for
every query, since their length varies.

Method is deliberately **not** an index key: an undeclared method on either
side is compatible with everything, so indexing on it would silently drop valid
candidates.

Only route *declarations* are indexed, and only ones that canonicalised. A
client call can never enter the index (two client calls must never "match" each
other), and an uninterpreted regex offers no structure to compare.

## Compatibility rules

### Method

| Client | Server | Result |
|---|---|---|
| POST | POST | `Exact` |
| POST | GET | `Incompatible` — not a weak candidate, not a candidate |
| POST | GET, POST | `Exact` (within the declared set) |
| undeclared | POST | `ClientUnknown` — candidate, never exact |
| POST | undeclared | `ServerUnknown` — candidate, never exact |
| undeclared | undeclared | `BothUnknown` — candidate, never exact |

**An undeclared method is never read as GET.** Flask's `@app.route("/x")` does
accept GET at runtime, but that is framework semantics — inference, not
observation. It excludes nothing, so it weakens the claim rather than
constraining it.

### Path

Compared **client-to-server**; the direction is not symmetric.

| Client segment | Server segment | Result |
|---|---|---|
| `Static` | `Static` | must be **exactly equal**, else incompatible |
| `Static` | `Parameter` | `ParameterBound` — a concrete value into a declared parameter |
| `Dynamic` | `Parameter` | `VariableAligned` — aligned, value undetermined |
| `Dynamic` | `Static` | `VariableAligned` — possible, **unverified** |
| anything | trailing `Wildcard` | `WildcardSuffix` — consumes ≥1 remaining segment |

Segment counts must agree unless the route ends in a wildcard. A wildcard
anywhere but the tail is refused rather than given an invented rule. A client
can never declare a parameter, so that shape is refused too.

### Discriminating evidence

**A candidate must share at least one static segment** (the root path `/`
discriminates by equality).

This rule came from running the matcher over real repositories, not from
reasoning about it:

- A bare catch-all `/{path:path}` in one Flask test module was matching client
  calls from **every other module** — 138 false edges from a single route.
- `GET /user_id` matched `GET /{name}` declared in an unrelated module.

Both share zero static segments. A route made entirely of variable positions
accepts a whole class of paths, so aligning with it says nothing about *this*
call. Such candidates are kept and visible, but classified `Ambiguous` and
never accepted. The cost is a false negative on genuinely-parameterised root
routes (a URL shortener's `GET /{code}`); the project prefers that to a false
join.

## Classification and acceptance

| Status | Meaning | Edge? |
|---|---|---|
| `Exact` | One candidate; exact method; every position determined | **yes** |
| `Strong` | One candidate; compatible, something undetermined | **yes** |
| `Ambiguous` | Several equally-plausible candidates, or no discriminating evidence | **no** |
| `NoMatch` | No compatible route | no |
| `Unsupported` | Client not comparable | no |

**Ambiguity produces no edge.** Several plausible routes is an unanswered
question, and an edge is an answer; choosing arbitrarily would make every
downstream confidence number inherit a guess. The candidate set is retained so
a human — or a later milestone with more evidence — can decide.

Clients are refused outright when:

| Reason | Why |
|---|---|
| `ClientPathNotCanonical` | A dynamic expression or regex; no structure to compare |
| `ClientPrefixUnresolved` | `` `${BASE}/orders` `` — the path's own length is unknown, so no alignment is safe. **M05's constant propagation unlocks these** |
| `ClientTargetsAbsoluteHost` | `https://api.stripe.com/...` is not evidence about this repository's service |

## Confidence

Uncalibrated **priors** from the specification's ladder, not measurements.

| Evidence | Prior |
|---|---|
| Exact method + every segment static and equal | 0.98 |
| Exact method + parameters bound to concrete values | 0.90 |
| Exact method + a position undetermined | 0.80 |
| Exact method + matched only via a trailing wildcard | 0.65 |
| Any of the above with an undeclared method | × 0.8 |

An edge marked 0.98 is **not yet** known to be correct 98% of the time.
Establishing that is M08, which fits these against CrossStack-Bench and
publishes a reliability diagram and expected calibration error. No accuracy
claim rests on them before then. The *ordering* is meaningful and enforced by
test.

## Evidence and provenance

Every accepted edge carries provenance `route-matcher` — deterministic static
matching. Not `lsp-symbol-resolution`: no language server was consulted. Not
`model-inference`: no model was consulted, and none ever will be (RULE 007).

The evidence string names both sides, both canonical forms, the backend file
and line, the handler, and the rules that fired:

```
POST /api/orders matched POST /api/orders at api/orders.py:6 (create_order);
exact HTTP method; every path segment static and equal
```

It never contains source text (RULE 015).

## Edge construction

- **Source**: a `File` node for the client file. Cartograph does not yet
  resolve the enclosing function of a call site, so naming one would be a
  guess; the precise position is the edge's own `location`, which is how the
  specification writes a client endpoint (`CheckoutButton.tsx:34`).
- **Target**: a `Function` node named after the route's handler when M02
  extracted one — the decorated function, read syntactically. Otherwise a
  `Route` node, so the limitation is visible in the graph rather than hidden.

The domain model refuses an edge without confidence, provenance, evidence and
a location, so an unexplainable edge cannot be constructed.

## Known limitations

- **Route mount prefixes are not composed.** `APIRouter(prefix=…)`, Flask
  blueprint prefixes and Django `include()` need cross-file resolution; a route
  is compared as declared at its own site. This is the largest source of missed
  matches on real repositories.
- **No same-service scoping.** Two services in one monorepo declaring the same
  route are correctly reported as ambiguous, but Cartograph cannot yet tell
  that a client belongs to one of them.
- **Test-client calls are matched like any other**, because syntactically they
  are HTTP requests.
- **A client's absolute URL never matches**, even when it addresses this
  service, because no observation establishes the deployment host.
- **No OpenAPI confirmation yet.** The milestone definition names it as
  authoritative evidence; it is deferred with the route-matching core delivered
  first, and is the natural next addition to this stage.
