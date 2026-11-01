# The resolver

`cartograph-resolver` turns the parser's per-file observations into evidenced,
cross-language edges: a TypeScript client call to the Python handler that serves
it, that handler to the model it uses, and the model to its table. It is the
subsystem the project exists for. The
[technical writeup](../writeup/technical-writeup.md#1-resolver-design) explains
its design in full, alongside what was measured about it.

## Pipeline

```text
TypeScript ─► HTTP call observations ─► symbolic URL values ─┐
                                                             ├─► canonical route ─► match ─► HttpCall
Python ────► route declarations ─► served path ──────────────┘
Python ────► model classes ─► table names ─► model uses ─► OrmAccess, Queries
TypeScript ─► component ─► hook ─► client function ─► Call
```

Every stage is deterministic. No language model participates
([ADR-0007](../adr/ADR-0007-no-llm-graph-construction.md)), and the resolver
never guesses a URL, symbol, route, table or dependency (RULE 010): a value it
cannot determine stays unknown (RULE 009), and evidence that is not enough
produces no edge.

## What is implemented

| Stage | Module | Design |
|---|---|---|
| Canonical routes | `canonical.rs`, `normalize.rs` | [canonical-routes.md](canonical-routes.md) |
| Dynamic URL evaluation | `symbolic.rs`, `evaluator.rs`, `dynamic.rs` | [dynamic-urls.md](dynamic-urls.md) |
| Cross-language matching, and the `HttpCall` edge | `matching.rs`, `edge.rs` | [matching.md](matching.md) |
| Router prefix composition | `routers.rs` | [routers.md](routers.md) |
| Import resolution | `imports.rs` | [imports.md](imports.md) |
| ORM resolution: `OrmAccess` and `Queries` | `orm.rs` | [orm.md](orm.md) |
| Client wrapper tracing: `Call` edges | `client_trace.rs` | [client-tracing.md](client-tracing.md) |
| Per-file cache for ORM access resolution, keyed on what each resolution read | `access_cache.rs`, `dependencies.rs` | [ADR-0014](../adr/ADR-0014-m10-scope-reconciliation.md) |

### What the resolver reads

The parser ([parser architecture](../architecture/parser.md)) supplies the
observations. On the client side, in TypeScript and TSX:

- `fetch(...)`;
- `.get`, `.post` and the other method calls on an object named `axios`,
  `client`, `http` or `api`;
- a request configuration object whose `url` or `endpoint` key holds a path —
  the shape a generated OpenAPI client uses. A `path` key is deliberately not
  read, because frontend router tables use it.

An Express- or Hono-style registration such as `app.get('/x', handler)` is
recognised only so that it is **not** mistaken for a client request; it is not
extracted as a route.

On the server side, in Python:

- FastAPI and Flask decorators with a literal path — `@app.get("/x")`,
  `@router.post(...)`, `@bp.route("/x", methods=[...])`;
- Django URL-conf entries, `path(...)` and `re_path(...)`;
- `APIRouter(prefix=...)` declarations and `include_router(child, prefix=...)`
  mountings, which compose the path a route is actually served on;
- Django mounts, `include(...)` and `*spread` into `urlpatterns`, which compose
  the path a URL-conf entry is served on ([urlconf.md](urlconf.md)). A mount
  is never a route.

### URLs are constructed, not written

```ts
const BASE = '/api/v1';
const url = `${BASE}/orders/${orderId}`;   // → /api/v1/orders/{unknown}
```

A restricted abstract interpretation evaluates each client URL into a symbolic
value, propagating constants through module scope and across files. A segment it
cannot determine stays `{unknown}`: aligned against a route parameter it can
still match, at a lower prior than a fully static match, and a client whose
leading prefix is unknown is refused rather than aligned by guesswork. An
environment variable is named and never read.

### Canonical routes, and matching

Server parameter dialects — `/orders/:id`, `/orders/{id}`, `/orders/<int:id>` —
share the shape `/orders/{*}`, and a path converter becomes a wildcard. A client
URL never declares a parameter. Matching compares a client's canonical route
against an index of served routes, requires at least one shared static segment,
and **produces no edge when the evidence is ambiguous**.

### Confidence

Each edge carries a confidence that is an **uncalibrated prior selected by
evidence class**, not a probability. The values, what selects them, and what M08
measured about them are set out in the writeup's
[confidence section](../writeup/technical-writeup.md#confidence-a-prior-chosen-by-evidence-class)
and [calibration section](../writeup/technical-writeup.md#3-calibration).

## Example output

One complete chain from the M07 corpus — Apache Airflow at its pinned commit
`9b43d6abc0fc` — from a generated client function to a database table:

```text
enqueueConnectionTest                  client function in the generated client
  │ HttpCall   0.98  route-matcher    at …/openapi-gen/requests/services.gen.ts:834
  ▼
enqueue_connection_test                handler
  │ OrmAccess  0.80  orm-resolution   at …/core_api/routes/public/connections.py:427
  ▼
ConnectionTestRequest                  model
  │ Queries    0.80  orm-resolution   at …/airflow/models/connection_test.py:82
  ▼
connection_test_request                table
```

Every edge, confidence and location above is a record in
[`benchmarks/m08/results/dataset.json`](../../benchmarks/m08/results/dataset.json),
produced by the accepted engine (`cartograph-m07`, `2514f77`) and labelled true by
M08's independent reader. The same chain, with the same confidences, is recorded
in [`benchmarks/acceptance/m07-acceptance-evaluation.json`](../../benchmarks/acceptance/m07-acceptance-evaluation.json).

## Known limitations

- **TypeScript, TSX and Python only.** No other language is parsed.
- **No JavaScript backends.** Express, Hono and similar route registrations are
  not extracted as routes.
- **No OpenAPI confirmation.** A specification is not read as evidence, though
  the domain model has a provenance and prior reserved for it.
- **ORM: SQLAlchemy and Django only**, and only direct accesses inside a handler —
  `Model(...)` and `Model.objects.…`. An access made through a service or
  repository layer is not followed; `session.query(Model)` and `select(Model)`
  are not recognised; raw SQL is not linked to models; SQLModel and other ORMs
  are not recognised ([orm.md](orm.md)).
- **Django `re_path` patterns** are recorded but not normalised, so they do not
  match. A `re_path` *mount* composes only when its pattern is an anchored
  literal (`^`, `^api/`).
- **Prefix composition follows named routers only.** An
  `include_router(items.router)` attribute, or a prefix applied by a
  project-local helper, does not compose.
- **Class-based views and DRF routers** are not modelled, nor are deployment-time
  URL rewrites.
- **Bounded traversal.** Call following stops after four hops (`MAX_CALL_HOPS`),
  and re-export following after four (`MAX_REEXPORT_HOPS`).
- **A client URL with an absolute host never matches**, even when it addresses
  this service, and two services in one repository declaring the same route are
  reported as ambiguous rather than told apart.
- **Resolution is syntactic and import-based.** No language server is
  consulted.

The findings measured on real repositories, including those still unsolved, are
in the [technical writeup](../writeup/technical-writeup.md#failures) and
[m07-remediation.md](../benchmarks/m07-remediation.md).
