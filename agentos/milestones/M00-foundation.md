# M00 — Engineering foundation

Target: day 2 · Branch: `feature/m00-foundation` · Tag on acceptance: `cartograph-m00`

## Scope

Workspace on the external SSD; GitHub repository and governance; AgentOS
vendored and adapted; six crates (`core`, `graph`, `parser`, `resolver`, `cli`,
`testkit`); foundational domain types with evidence/provenance/confidence
enforced by construction; test + benchmark infrastructure; CI; quality gates;
milestone/checkpoint/ADR systems.

Explicitly out of scope: parsing, resolution, LSP, storage, incremental
analysis, desktop, MCP, AI.

## Acceptance criteria

- [x] Repository on external SSD, Git initialised, GitHub remote working
- [x] `cargo check/fmt --check/clippy -D warnings/test --workspace` all pass
- [x] Six crates compile; domain types (`Node`, `Edge`, `Confidence`,
      `Provenance`, `Evidence`, `SourceLocation`, `CommitId`) with tests
- [x] `Edge` cannot be constructed without confidence + provenance + evidence
- [x] AgentOS validator PASS; gates QG-001…008 defined and runnable
- [x] No secrets, no machine-specific paths in tracked files
- [x] No prohibited V1 dependency introduced
- [x] CI workflow, PR template, issue templates, CODEOWNERS
- [x] Documentation set complete (README, RULES, ARCHITECTURE, ROADMAP,
      CHECKPOINTS, SECURITY, CONTRIBUTING, AGENTS, AGENTOS, CHANGELOG, 9 ADRs)
- [ ] PR open, CI green, human owner acceptance → tag `cartograph-m00`

## Gate

`make check && make gates` — all PASS. CI mirrors both.
