# M06 — ORM resolution

Target: Day 13 · Branch: `feature/m06-orm-resolution` · Tag on acceptance: `cartograph-m06`

Scope: SQLAlchemy + Django ORM model→table resolution; handler→model access
edges (`OrmAccess`, `Queries`) with `orm-resolution` provenance.

Acceptance: fixture models resolve to tables incl. `__tablename__` overrides;
gates pass.

Implementation notes (recorded during execution):

- The domain model needed no change: `NodeKind::Table`/`Column`,
  `EdgeKind::OrmAccess`/`Queries` and `Provenance::OrmResolution` were all in
  place from M00. No ADR was required for the domain.
- Two parser facts were added because M06 needed them and guessing would have
  been worse: class-body attribute assignments and class base names.
- `ADR-0011` was required: M04 and M06 each created a node for the same
  handler, so the chain was unwalkable despite every edge being correct.
- No table name is derived from a class name. Django's default needs an app
  label absent from the model file; Django's own `unmanaged_models` tests
  confirm the refusal is right (`A02` declares `db_table = "a01"`).
- SQLAlchemy `session.query(Order)` is unsupported: the model is a call
  argument and M02 records argument counts, not identities.

Standard exit: PR + CI green + self-review + human acceptance + checkpoint tag + state update + STOP.
