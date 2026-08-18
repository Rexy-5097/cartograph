# ADR-0006 — Sigma.js v3 before any custom renderer

**Status:** Accepted · 2026-08-18

## Context

Graph rendering invites premature engineering. The one-year horizon may need
50k–200k nodes, and wgpu is tempting.

## Decision

MAP (M11) renders with Sigma.js v3 (WebGL) over Graphology. A custom renderer
is deferred until Sigma is measurably the bottleneck — there is no rendering
problem below fifty thousand visible nodes. Layout is computed in the Rust
core regardless of renderer (ADR-0001).

## Alternatives

- **wgpu from the start** — rejected: months of renderer work before knowing
  whether anyone uses MAP; Gate 4 may cut MAP investment entirely.
- **SVG/Canvas (d3)** — rejected: known ceiling well below 10k nodes at 60fps.

## Consequences

10k nodes @ 60 FPS must be verified on Sigma at M11 (it is the documented
target, not a measured fact). If the 200k-node future arrives, the renderer
swap is contained because the frontend only draws `{id, x, y, cluster}`.
