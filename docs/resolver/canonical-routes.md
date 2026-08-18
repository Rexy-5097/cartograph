# Canonical route normalisation (M03)

Implemented in [`cartograph-resolver`](../../crates/cartograph-resolver/).
Turns one observation into one canonical form:

```
RouteObservation   ─┐
                    ├─► normalize ─► CanonicalRoute
HttpCallObservation ─┘
```

## The boundary

```
M03 (this page):  raw observation ─► canonical representation
M04 (not built):  client call + backend route ─► MATCH ─► evidenced edge
```

Canonicalisation is **per-observation**. A `CanonicalRoute` never references
another observation — it has exactly five fields (`path`, `methods`, `status`,
`notes`, `provenance`), none of which can name a counterpart. Nothing in M03
compares two observations, decides they describe the same endpoint, attaches
confidence, or produces an edge.

Two structural helpers exist because M04 will need them:
`CanonicalRoute::same_shape` and `compatible_method`. They answer *"could these
be comparable?"* — a question about two shapes. They are not matches: two
unrelated services can declare `/orders/{id}` and share a shape. Deciding a
client call reaches a particular route needs mount prefixes, base URLs and
evidence that M03 neither has nor seeks.

## The distinction that governs the model

| Source | Segment | Why |
|---|---|---|
| `/orders/123` | `Static("123")` | A value. Nothing about a numeric segment says it is an identifier. |
| `/orders/{id}` | `Parameter { name: "id" }` | The **syntax** declared a parameter. |
| `` /orders/${orderId} `` | `Dynamic { source: "orderId" }` | A substituted value this analysis does not know. |

`Parameter` and `Dynamic` stay distinct deliberately. "This position accepts
anything" (server) and "I am supplying an unknown value" (client) are different
facts; flattening them would destroy evidence M04 needs.

## Supported dialects

| Dialect | Example | Result |
|---|---|---|
| Colon | `/orders/:id` | `Parameter { name: id }` |
| Curly | `/orders/{id}` | `Parameter { name: id }` |
| Curly + converter | `/orders/{id:int}` | `Parameter { name: id, constraint: int }` |
| Angle | `/orders/<id>` | `Parameter { name: id }` |
| Angle + converter | `/orders/<int:id>`, `<uuid:id>`, `<slug:s>` | `Parameter { name, constraint }` |
| Path converter | `/files/<path:rest>`, `/files/{rest:path}` | `Wildcard { name: rest }` |
| Star | `/files/*`, `/files/**` | `Wildcard` |
| Template | `` `${BASE}/orders/${id}` `` | `Dynamic` segments, status `Partial` |

Converter order differs by framework — Flask and Django write
`<converter:name>`, Starlette writes `{name:converter}` — and both are read
correctly. Converters are preserved, never interpreted: an `int` constraint is
recorded, not used to validate anything.

All parameter dialects share the shape `/orders/{*}`, which is the form the
specification writes.

**Client URLs do not recognise parameter syntax.** A client fetching a literal
`/orders/:id` is requesting that text; it is not declaring a parameter.

## Normalisation rules

**Methods.** Case-insensitive (`get`/`GET`/`Get` → GET), deduplicated, ordered
deterministically so declaration order cannot change the canonical form. A
missing method is `Methods::Unknown` — **never defaulted to GET**. Flask's
`@app.route("/x")` does accept GET at runtime, but that is framework semantics,
which is inference and belongs to a later milestone with evidence attached.

**Trailing slashes** are preserved, not collapsed. `/orders` and `/orders/`
produce different canonical paths, and `shape()` renders the difference.
Frameworks disagree about whether they are one route; M04 decides, and M03 must
not destroy the evidence needed to decide.

**URLs are decomposed.** `https://example.com/api/orders?x=1#f` yields scheme
`https`, authority `example.com`, segments `api`/`orders`, query `x=1`,
fragment `f`. The scheme is lowercased; the authority is not otherwise
rewritten. Query and fragment are metadata and **never become segments** —
query parameter names are not path parameters.

**Percent-encoding is preserved, not decoded.** `%2F` is an encoded slash;
decoding it would invent a segment boundary the source never contained.

**Repeated slashes** produce empty segments, which are dropped with an
`EmptySegmentsDropped` note — collapsed, but recorded rather than hidden.

**Templates** are consumed structurally, never evaluated. A substitution
becomes one `Dynamic` segment; one glued to adjacent text (`` /v${major}/x ``)
makes that whole segment unknown rather than inventing a value. A *leading*
substitution adds `TemplatePrefixUnknown`, because `` `${BASE}/orders` `` is
not `/orders`. Resolving `BASE` is M05's constant propagation.

## Unsupported cases

| Case | Status | Behaviour |
|---|---|---|
| Django `re_path` regex | `Unsupported` | Not interpreted; raw pattern retained, no structure asserted |
| Dynamic path (`url`, `'/' + param`) | `Unsupported` | Not interpreted; raw source retained |

Building a regex-to-route compiler is out of scope, and a wrong structural
reading is worse than none. The product prefers **"cannot safely normalise"**
over "probably this route". An `Unsupported` route shares a shape with nothing,
including itself — there is no structure to compare.

## Normalisation status and notes

`Exact` (everything determined) · `Partial` (shape known, some positions
unknown) · `Unsupported` (no structure asserted).

Notes describe **the transformation** — `QuerySeparated`,
`TemplatePrefixUnknown`, `RegexNotInterpreted` and so on. They are not
confidence: confidence attaches to edges, and M03 produces none.

## Provenance

Every canonical route retains its observation kind (client call vs route
declaration), language, repository-relative file, span, raw path and handler.
M04 needs this to *explain* a match rather than assert one, and the interface
needs it so clicking an eventual edge shows the original source position.

## Known limitations

- Route mount prefixes (`APIRouter(prefix=…)`, Flask blueprint prefixes,
  Django `include()`) are not applied — a route's canonical path is the path as
  declared at its own site. Composing mounted prefixes needs cross-file
  resolution and belongs with M04.
- No relative-path resolution against a base URL; that is M05's work.
- Regex patterns that *could* be read structurally (a bare `^orders/$`) are
  still refused, because the detector is deliberately generous.
