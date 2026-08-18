# The resolver

> **Status: design only.** `cartograph-resolver` is an empty crate. This
> documents the frozen design the M03–M06 implementations must satisfy.
> Both sides' inputs now exist: M01 produces TypeScript call sites, template
> structure and HTTP observations; M02 produces Python route observations,
> HTTP observations and f-string structure
> ([parser architecture](../architecture/parser.md)). **Nothing is resolved
> yet** — no path is canonicalised and no observation is matched to another.

The single subsystem that justifies the project. Pipeline:

```
AST extraction → HTTP call extraction → string analysis     (frontend, M01/M05)
route extraction → normalization                              (backend, M02/M03)
        └────────────── JOIN on canonical route ─────────────┘         (M04)
route matching → confidence scoring → ORM resolution → graph edge   (M04/M06)
```

## Frontend extraction (M01, M05)

Locate `fetch`, `axios.*`, generated client calls; extract method, URL
expression, location. URLs are constructed, not written:

```ts
const BASE = '/api/v1';
const url = `${BASE}/orders/${orderId}`;   // → /api/v1/orders/{*}
```

Restricted abstract interpretation over string values: constant propagation
through module scope, template concatenation, symbolic unknowns. Unresolvable
segments become `{unknown}` and lower confidence. **The resolver never guesses
a value** (RULE 010). No language model participates (ADR-0007).

## Backend extraction (M02)

FastAPI decorators, Flask `@app.route`, Django `urlpatterns`, Express routers:
path pattern, method, handler symbol, location.

## Normalization (M03)

`/orders/:id` ≡ `/orders/{id}` ≡ `/orders/<int:id>` ≡ `/orders/${id}` →
`(POST, /orders/{*})`. Where an OpenAPI specification exists it is ground truth
and inference is skipped.

## ORM resolution (M06)

SQLAlchemy and Django ORM first; handler → service → repository → model →
table. SQLModel, Prisma, Drizzle, raw SQL later. Quality over breadth.

## Target output (M09)

```
$ cartograph trace CheckoutButton
CheckoutButton
│ HTTP POST /api/orders   0.94 template-eval
▼
create_order()
│ ORM Order(...)          0.97 orm-resolve
▼
orders — table
```
