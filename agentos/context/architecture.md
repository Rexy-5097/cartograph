# Architecture — Cartograph (context summary)

> Layer: project · Full document: [../../ARCHITECTURE.md](../../ARCHITECTURE.md)

One Rust core; CLI, desktop and MCP are thin clients of the same graph API.
Two-tier analysis: tree-sitter for syntax, LSP for semantics. Cartograph owns
the domain graph; petgraph supplies algorithms underneath and never leaks.
Every edge carries confidence, provenance, evidence, source location, commit,
created_at — enforced by the type system, not review. Storage is embedded and
local (redb, from M09/M10). Layout is computed in Rust; the frontend only draws.
No source leaves the machine.

Crate dependency direction (checked by QG-006):

```
cli → core        parser → core        testkit → core
graph → core      resolver → core, parser, graph
```

`cartograph-core` depends on no Cartograph crate; nothing depends on the CLI.
