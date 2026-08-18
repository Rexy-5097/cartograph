# ADR-0003 — Cartograph owns the domain graph; petgraph sits underneath

**Status:** Accepted · 2026-08-18 · Enforces RULE 003

## Context

The graph needs algorithms (traversal, reachability, eventually layout) that
petgraph provides well. But the domain model — nodes with kinds, edges with
evidence — is the product's foundation and must not be shaped by a library's
types.

## Decision

`cartograph-core` defines the domain model with no petgraph dependency.
`cartograph-graph` wraps `petgraph::StableDiGraph` and never exposes a petgraph
type in any public signature. `StableDiGraph` specifically, because indices
must survive removals when the incremental engine (M10) removes and re-adds
subgraphs.

Enforced three ways: petgraph appears in only one Cargo.toml (QG-006 scans),
no `pub` signature names it (QG-006 greps), and integration tests exercise the
graph entirely from outside.

## Alternatives

- **Use petgraph types directly** — rejected: replacing the library later
  (e.g. for an on-disk representation) would break every caller.
- **Write graph algorithms by hand** — rejected: re-deriving stable-index
  bookkeeping is undifferentiated work.

## Consequences

A thin wrapping layer to maintain; algorithm access goes through deliberate
API additions rather than leaking capability. The backing store can change in
one file.
