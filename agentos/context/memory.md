# Memory — Cartograph

> Layer: project · Running context buffer for agents. Newest first. Prune when stale.

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
