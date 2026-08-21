# Import resolution

```text
name at a use site ─► import statement ─► module ─► exported symbol ─► declaration
```

Added by the M07 remediation. M07 measured 286 `OrmAccess` edges whose model
name was imported from somewhere other than the model's own module, and a
sample showed why that matters: Onyx's Airtable connector imports `Document`
from `connectors/models.py`, a Pydantic `BaseModel`, while the SQLAlchemy
`Document` lives in `db/models.py`. Matching a name against a project-wide map
cannot tell them apart.

## What resolves

| Language | Form | Example |
|---|---|---|
| Python | absolute | `from app.models import Order` |
| Python | relative | `from .models import Order`, `from ..models import Order` |
| Python | package | resolved through `__init__.py` |
| Python | alias | `from app.models import Order as OrderModel` |
| Python | re-export | a module that imports a name and is imported for it |
| TypeScript | named | `import { X } from "./m"` |
| TypeScript | default | `import X from "./m"` |
| TypeScript | namespace | `import * as m from "./m"` |
| TypeScript | re-export | `export { X } from "./m"`, `export * from "./m"` |
| TypeScript | build alias | `resolve: { alias: { openapi: "/openapi-gen" } }` in a Vite configuration |

A dotted Python path is matched as a path **suffix anchored at a separator**,
because a project may be rooted under any number of source directories —
`backend/`, `src/`, `airflow-core/src/`. `ics.models` therefore cannot be
satisfied by `analytics/models.py`.

A TypeScript build alias governs only files beneath the configuration that
declares it. Airflow has five Vite configurations, several declaring an alias
named `src`; a global alias list resolves imports into the wrong project.

## What refuses

| Outcome | When |
|---|---|
| `Unresolved` | the module is not one the project contains — a third-party package, an unaliased bare specifier |
| `Unresolved` | a dotted path matches two files in a monorepo |
| `Unresolved` | a re-export chain exceeds four hops, or cycles |
| `Ambiguous` | two barrel files export the same name |
| `NotBound` | nothing in the file binds the name |

`NotBound` is deliberately **not** a refusal. A star import, a conditional
import, or a name bound by machinery this analysis does not model all land
there, and callers decide what it means for them. The ORM resolver treats it as
permissive, because refusing it would trade M07's false positives for false
negatives.

## What is out of scope

`sys.path` manipulation, namespace packages, editable installs, `node_modules`
resolution, `tsconfig` path mapping, conditional exports, and any resolution
that depends on runtime state. Each is a different mechanism; none is guessed
at.
