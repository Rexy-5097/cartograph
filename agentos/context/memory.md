# Memory — Cartograph

> Layer: project · Running context buffer for agents. Newest first. Prune when stale.

- **Verify continuously, not at the end (RULE 026, QG-009).** M04's fixtures
  were green while the matcher produced 138 false edges on Flask's own
  repository. Synthetic tests prove a subsystem does what you imagined; only
  real input proves what you imagined was right. Build in slices, state
  invariants first, inspect real output by hand, and record before/after counts
  for every defect. See docs/development/continuous-verification.md.
- **The toolchain is pinned** (rust-toolchain.toml, 1.97.1). Local had drifted
  eight minor versions behind CI's `stable`, which is why `collapsible_match`
  fired only in CI at both M01 and M02. Raising the pin means fixing the newer
  lints in the same commit.
- **exFAT quirks are real, not theoretical.** The T7 Shield volume materialises
  AppleDouble `._*` files (including inside `.git`, causing "non-monotonic
  index" errors) and has no hard links (cargo copies instead of linking its
  incremental cache — harmless but slower). `.gitignore` covers `._*`;
  `make clean-sidecars` removes them; docs/development/external-storage.md
  explains. Git itself operates correctly.
- **The validator caught a real bug at M00**: `Provenance::OpenApiSpec`
  serde-renamed to `open-api-spec` while Display said `openapi-spec`. A test
  asserting Display/serde agreement caught it. Keep writing invariant tests —
  they pay for themselves immediately.
- **NodeIds are graph-local** until M10. Never pass a handle from one graph
  into another; it silently addresses the wrong node rather than failing.
- **Upstream AgentOS quirk**: validator hardcoded the author's absolute repo
  path (fixed, see ADR-0009) and DOCUMENTATION_INDEX.md used absolute file://
  links (rewritten to relative).
- **Confidence priors are uncalibrated** — never quote them as accuracy. M08
  replaces them with fitted values plus a reliability diagram and ECE.
