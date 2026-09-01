# ADR-0017 — The render scene: layout plus the minimum a reader needs

**Status:** Accepted · 2026-09-01 · Implements [ADR-0001](ADR-0001-rust-core-is-the-product.md),
[ADR-0006](ADR-0006-sigma-before-custom-renderer.md) and
[ADR-0015](ADR-0015-layout-contract.md)

## Context

ADR-0015 fixed the layout contract at `{id, x, y, cluster}` and kept names,
kinds and evidence out of it. That is right for a *layout*: an algorithm that
places points does not need to know what they are called.

It is not enough to draw a map. A screen of unlabelled dots cannot answer
"what is this system?", which is half of M11's acceptance criterion. The
renderer needs at least a label, and it needs to tell a route from a table from
a function, or the picture carries no information a reader can use.

So something has to carry more than `Layout` does. The question is what, and
where it is assembled.

## Decision

A **`Scene`** type in `cartograph-desktop`, composed by `scene::compose(graph,
layout)`. `layout::Layout` is **not widened** — it keeps exactly the shape
ADR-0015 gave it, and the join happens in the client that needs it.

The payload is:

| Node field | Why a renderer needs it |
|---|---|
| `id` | addresses the node for selection and as an edge endpoint |
| `x`, `y` | **copied verbatim** from the layout |
| `cluster` | colour and grouping; also verbatim |
| `label` | the artefact's name; without it the map says nothing |
| `kind` | a route, a table and a function must be distinguishable at a glance |

| Edge field | Why |
|---|---|
| `id`, `source`, `target` | the relationship and its endpoints |
| `kind` | an HTTP call crossing languages is the claim the product exists to make; it must not look like an import |

**Deliberately excluded: `confidence`, `provenance`, `evidence`, `location`.**
Those are the evidence record. Opening it on an edge click is M11's defining
interaction and belongs to the next slice. Shipping them now would mean every
node and edge paid transport for data nothing draws, and the next slice's
boundary would already be blurred before it was designed.

**Coordinates are copied, never computed.** Tests on both sides assert bit
equality rather than approximate equality — `a_scene_copies_layout_coordinates_verbatim`
in Rust, and `uses the exact coordinate it was given` in TypeScript. A
tolerance would pass a frontend transform that scaled everything by 1.0000001,
and a frontend transform is precisely what ADR-0001 forbids.

## Alternatives

- **Widen `layout::Layout` to carry labels and kinds.** Rejected: it would put
  presentation concerns inside a geometry algorithm, and ADR-0015's contract —
  which the layout suite pins — would have to change for a reason that has
  nothing to do with layout.
- **Let the frontend fetch names separately and join them.** Rejected: two
  round trips, and a join in TypeScript keyed on ids is a place for a client to
  disagree with the core about what a node is.
- **Send the whole `ArchitectureGraph`.** Rejected: it ships evidence,
  provenance and confidence for every edge to draw a line, and it makes the
  next slice's boundary meaningless.

## Consequences

- The renderer receives renderer-neutral data. Replacing Sigma means replacing
  code that consumes `{id, x, y, cluster, label, kind}` — no resolver, graph or
  layout change — which is the containment ADR-0006 was written to preserve.
- Slice 4 extends this deliberately, by adding an explicit evidence lookup
  rather than by widening every node.
- `the_scene_carries_no_analysis_payload` pins the serialised key set, so
  adding a field is a deliberate act rather than a drift.
