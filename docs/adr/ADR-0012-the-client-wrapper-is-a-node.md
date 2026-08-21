# ADR-0012 — The client wrapper is a node

**Status:** Accepted · 2026-08-22 · Refines [ADR-0011](ADR-0011-node-identity-within-a-graph.md)

## Context

M07 measured the product's central claim on seven real repositories and found
it unexercised: of 1,458 `HttpCall` edges, twelve had a TypeScript client, and
no complete frontend-to-table chain began at a frontend file.

The largest cause was structural rather than a bug. Real frontends do not write
URLs at call sites. A component calls a hook, the hook calls a generated client,
and the client issues the request:

```text
ParseDagButton.tsx  →  useDagParsing  →  useDagParsingServiceReparseDagFile  →  reparseDagFile
                                                                                     │
                                                                          PUT /api/v2/parseDagFile/{file_token}
```

Only the last of those four files contains a URL. M04 recorded the *file* as
the client node — deliberately, because "Cartograph does not yet resolve the
enclosing function of a call site, so naming one would be a guess". That was
true then. It made two things impossible now: a file node cannot be the target
of a call from another file's function, and a chain through a wrapper has
nowhere to attach.

Attaching the component straight to the backend handler was the obvious
shortcut and is the wrong answer. No line of source states that relationship,
and a reader asking why the two are connected would have no place to look.

## Decision

**An `HttpCall` edge is sourced at the function that issues the request, when
the extractor resolved one; at the file otherwise.**

The enclosing scope of a call site is now recorded by M01 and M02 as
`HttpCallObservation::enclosing`, so naming the function is no longer a guess —
it is a fact the parser already had.

**Calls that reach such a function become `Call` edges**, resolved through the
project's imports rather than matched by name, with provenance
`StaticImportResolution`. Only calls that transitively reach a request produce
an edge: this is not a general call graph, it is the path from a component to
the wrapper it uses.

Identity stays exactly as ADR-0011 defines it — `(kind, name, file)`. A client
wrapper is a `Function` node like any other, so `postPool` declared in
`services.gen.ts` is one node however many callers reach it, and a `postPool`
declared in a different module is a different node.

## Consequences

The chain the product exists to compute is now traversable from hand-written
frontend code on a real repository, with every hop carrying evidence,
confidence and provenance:

```text
ParseDagButton.tsx:30  --call-->  useDagParsing.ts:30  --call-->  queries.ts:2546
  --call-->  services.gen.ts:4538  --http-call-->  dag_parsing.py:32
  --orm-access-->  DagPriorityParsingRequest  --queries-->  dag_priority_parsing_request
```

A client observation at module scope still produces a `File` node, so nothing
that worked before stops working. The spec's own worked example changes shape:
`CheckoutButton.tsx --HttpCall--> create_order` becomes `onSubmit
--HttpCall--> create_order`, with the file kept on the node's location. That is
a more precise statement of the same fact, and the tests were updated to assert
it rather than the file.

Four hops of call following are recorded and no more. Beyond that the path
stops being something a reader could check by hand, which is the standard every
edge in this project is held to.

## Alternatives rejected

**Attach the component directly to the handler.** Loses the only evidence a
reader could verify, and asserts a relationship no source line states.

**Inline the wrapper's URL into the caller.** Would make every caller of a
generated client appear to contain a URL it does not contain, and would
multiply one observation into hundreds of fabricated ones.

**A general call graph.** Would add an edge for every resolved call in the
repository — hundreds of thousands on Airflow — burying the few that matter.
Reachability to a request is what makes a call worth recording here.
