# ADR-0011 — Node identity within a graph

**Status:** Accepted · 2026-08-19 · Refines [ADR-0003](ADR-0003-cartograph-owns-the-graph.md)

## Context

Until M06 every graph was populated by one analysis, so `add_node` creating a
fresh node per call was harmless. M06 made two analyses populate one graph:
the route matcher (M04) creates a `Function` node for a route's handler, and
the ORM resolver (M06) creates a `Function` node for the handler that touches a
model.

They are the same handler. They became two nodes.

Every edge was individually correct — `CheckoutButton.tsx --HttpCall--> create_order`
and `create_order --OrmAccess--> Order` — and the chain was still unwalkable,
because the two `create_order`s were different nodes. A graph that silently
splits into disconnected fragments is worse than one that fails loudly: the
edges look right in isolation, and only a traversal reveals the break.

The M00 domain model already warned that `NodeId` is graph-local and not
content-addressed, deferring stable identity to M10. That deferral is about
identity *across* analyses and runs. This is a narrower problem: agreement
*within* a single graph.

## Decision

`ArchitectureGraph::node_for(kind, name, location)` returns the existing node
for an artefact or creates it. Identity is the triple **(kind, name, file)**.

The line number deliberately does not participate. Two analyses legitimately
observe one artefact at different lines — a decorator versus its `def`, a
declaration versus a call site — and treating those as distinct nodes is
exactly the defect being fixed.

`add_node` remains, and still creates unconditionally. Callers that genuinely
want a fresh node keep it; the resolver's edge builders use `node_for`.

## Alternatives

- **Pass node ids between the resolvers.** Rejected: it makes every future
  analysis a parameter of every other, and the brief explicitly ruled out an
  ad-hoc key.
- **Deduplicate as a post-pass over the finished graph.** Rejected: edges would
  already reference the wrong endpoints, so the pass would have to rewrite
  them — more machinery, and a window in which the graph is wrong.
- **Bring M10's content-addressed identity forward.** Rejected as premature:
  that design needs the incremental engine's invalidation model to be settled,
  and this problem does not require it.

## Consequences

Two analyses describing one artefact converge on one node, and the cross-stack
chain is walkable. Nodes are deduplicated only within a graph, so nothing about
M10's cross-run identity is prejudged.

Same-named artefacts in different files stay distinct, which is correct. A
same-named artefact appearing twice *in one file* — two classes called `Order`
in one module — collapses into one node; the ORM resolver already refuses that
case upstream by marking the name ambiguous, and no other analysis produces it
today.
