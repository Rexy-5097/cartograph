# M06 — ORM resolution

Target: Day 13 · Branch: `feature/m06-orm-resolution` · Tag on acceptance: `cartograph-m06`

Scope: SQLAlchemy + Django ORM model→table resolution; handler→model access
edges (`OrmAccess`, `Queries`) with `orm-resolution` provenance.

Acceptance: fixture models resolve to tables incl. `__tablename__` overrides;
gates pass.

Standard exit: PR + CI green + self-review + human acceptance + checkpoint tag + state update + STOP.
