# QG-004 — Tests

`cargo test --workspace` exits 0. Every behavior change lands with a test.
Integration tests exercise crates from outside their public API (this is what
proves petgraph has not leaked — ADR-0003). Golden-fixture comparisons join at
M08; benchmark regression checks at M10.
