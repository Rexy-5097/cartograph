# Router prefix composition

```text
@pools_router.post("")                        the decorator states ""
pools_router = AirflowRouter(prefix="/pools")
public_router = AirflowRouter(prefix="/api/v2")
public_router.include_router(authenticated_router)
authenticated_router.include_router(pools_router)
                                              ─► POST /api/v2/pools
```

Added by the M07 remediation. A route's served path is not the path its
decorator states, and comparing a frontend's `/api/v2/pools` against a
decorator's `""` compares a served path to a fragment.

## What composes

A declaration `X = <Something>Router(prefix="/p")` and a mounting
`parent.include_router(child, prefix="/q")`. **Any constructor whose name ends
in `Router` counts**, because projects subclass it — Airflow declares every
route on `AirflowRouter`, and requiring the literal `APIRouter` finds none of
them.

Names at an inclusion site are resolved through the project's imports, so a
router declared in one module and mounted in another composes correctly.

## What refuses

| Outcome | When |
|---|---|
| `Unresolved` | a `prefix=` is present but not a string literal |
| `Unresolved` | a `prefix=` at the inclusion site is not a literal |
| `Unresolved` | the inclusion chain cycles, or is deeper than twelve |
| `Ambiguous` | a router is mounted in more than one parent |
| — | the receiver names no router this project declares |

**Composition can only make a path more complete.** Every refusal above leaves
the route exactly as declared. An unknown prefix never becomes an empty one.

A trailing slash survives composition, because M03 keeps `/items` and `/items/`
distinct: `@router.post("")` under `/pools` serves `/pools`, and
`@router.get("/")` serves `/pools/`.

## What is out of scope

Prefixes contributed by a framework at startup through anything other than
`include_router` — Onyx's `include_router_with_global_prefix_prepended` is a
project-local helper, and its global prefix is not composed. Deployment-time
rewriting, such as a Next.js `rewrites()` mapping `/api/*` onto a backend, is a
different mechanism again.
