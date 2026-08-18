# ADR-0004 — Evidence, provenance and confidence are first-class edge fields

**Status:** Accepted · 2026-08-18 · Enforces RULES 004–006

## Context

The interface-defining question is "show me exactly why you think these two
things are connected." Competitors assert relationships; Cartograph's moat is
producing evidence for them. Retrofitting evidence onto a bare A→B graph later
is expensive and, for already-analysed repositories, impossible.

## Decision

Every edge carries kind, confidence, provenance, evidence, source location,
commit and created_at from the first commit of the domain model. `Edge` has no
`Default`, no public fields, no partial builder — the only constructor requires
all of them, so an unevidenced edge does not compile. Provenance (the method)
decides derived-vs-inferred rendering, not a threshold on the confidence
number.

## Alternatives

- **Optional metadata fields** — rejected: optional means absent under
  deadline pressure.
- **Confidence threshold for "derived"** — rejected: whether a claim was
  computed or estimated is a property of the method, not the number.

## Consequences

Constructing edges is verbose (testkit absorbs this in tests). Storage costs
more per edge. The benchmark (M08), the DIFF surface (M13) and the edge-record
UI (M11) all build on fields that already exist.
