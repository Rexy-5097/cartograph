# ORM and database resolution (M06)

Implemented in [`cartograph-resolver`](../../crates/cartograph-resolver/) (`orm.rs`).
Completes the chain the product exists to produce:

```
CheckoutButton.tsx → POST /api/orders → create_order() → Order → orders
```

Static source analysis only. Nothing executes Python, imports a module,
connects to a database, reads the environment, or runs an ORM.

## The rule that decides every case

**A class is a model only when the source says so.** Inheriting a recognised
ORM base is the evidence; a name that looks like a model is not.

| Framework | Recognised bases |
|---|---|
| SQLAlchemy | `Base`, `DeclarativeBase`, `db.Model`, `Model` |
| Django | `models.Model` |

Only module-scope classes are considered; a model defined inside a function is
not a module's schema. `class OrderBase` is not `Base`, and `class Plain` with
a `__tablename__` is not a model.

## Table-name resolution

| Case | Result |
|---|---|
| `__tablename__ = "orders"` | `Declared` — read exactly |
| `Meta.db_table = "orders"` | `Declared` — read exactly |
| SQLAlchemy without `__tablename__` | **Unresolved** |
| Django without `db_table` | **Unresolved** |
| `db_table = settings.TABLE` | **Unresolved** — not a string literal |

An explicit declaration always wins. **No name is ever derived from a class
name.** Django's default is `applabel_modelname` and the app label lives in the
app's configuration, not the model file — deriving `order` from `Order` would
be wrong for every app that sets a label. SQLAlchemy 2.0 requires
`__tablename__` on a concrete model, so its absence means abstract or a naming
callback this analysis cannot see. Both refuse.

Real evidence for the refusal being right: Django's own `unmanaged_models`
tests declare `A02` with `db_table = "a01"`. Reading the declaration gives
`a01`; deriving from the class name would have given `a02` — wrong.

**Ownership is decided by span containment, not by name.** Every Django model
nests a class called `Meta`, so matching the attribute by owner *name* attributes
the first `db_table` in a file to every model in it. That was a real defect,
found by inspecting output: a model with a dynamic `db_table` inherited an
unrelated model's table.

## Handler → model access

A deliberate supported subset, not every ORM API.

| Pattern | Kind |
|---|---|
| `Order.objects.get/filter/all/exclude/count/exists/first/last` | `Query` |
| `Order.objects.create/update/delete/bulk_create` | `Persist` |
| `Order(...)` | `Persist` |

An unrecognised manager method produces nothing. An access on a non-model
produces nothing. A module-scope access names no handler and produces no edge —
attributing it to one would be a guess.

**Shadowing disqualifies.** If any variable in the file binds the model's name,
accesses in that file are dropped: the analysis cannot tell which binding a
call site sees.

**Ambiguous names resolve to nothing.** Two classes claiming one name are
removed from the model set entirely and listed in `ambiguous` — 25 such names
in SQLAlchemy's own repository, 158 in Django's.

## Edges

Two distinct relationships, never conflated:

- **handler → model**, `OrmAccess`, evidenced by the call as written.
- **model → table**, `Queries`, evidenced by the declaration that named it.

Both carry provenance `orm-resolution` and its prior (0.80), which remains
**uncalibrated** until M08. A model whose table is unresolved gets no table
edge — the access is still a fact, the table is not.

Repeated accesses from one handler produce **one** edge: the same fact stated
three times is one fact.

## Raw SQL

Not linked to models. A table name inside SQL text is not evidence of an ORM
relationship, and inferring `INSERT INTO orders` → `Order` would be a guess.
Raw SQL mapping is a possible future milestone.

## Known limitations

- SQLAlchemy `session.query(Order)` / `select(Order)` are not recognised:
  the model appears as a call *argument*, and M02 records argument counts
  rather than argument identities. Model construction and Django managers
  cover the common cases; extending this needs an argument-level parser fact.
- Only SQLAlchemy and Django. SQLModel, Peewee, Tortoise and others are not
  recognised — the FastAPI template in the corpus uses SQLModel, and correctly
  yields zero models.
- Django's default table naming is never derived (see above).
- Model inheritance hierarchies are not followed: a subclass of a model class
  is not itself recognised unless it names a recognised base directly.
- Shadow detection is file-wide rather than flow-sensitive, so it is
  conservative — it can drop a valid access.
