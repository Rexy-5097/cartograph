# State — Cartograph

> Layer: project · **Update at the end of every session** · Machine-readable
> twin: [../artifacts/project-state.yaml](../artifacts/project-state.yaml)

## Current position

| Field | Value |
|---|---|
| Active milestone | **M00 — Engineering foundation** |
| Status | Complete pending human review (PR open) |
| Branch | `feature/m00-foundation` |
| Next permitted milestone | M01 (after M00 PR is accepted by the human owner) |
| Last checkpoint | none yet — M00 is the first |
| Spec | Frozen V3, August 2026 |

## What exists

- Rust workspace, six crates. Domain model (evidence/provenance/confidence
  enforced by construction), architecture graph over petgraph, CLI `version`
  command, testkit with the spec's worked cross-stack chain as fixtures.
- 50 tests passing; fmt/clippy(-D warnings)/check clean; Criterion harness
  compiles (no published numbers).
- AgentOS v1.0.0 vendored under `agentos/`, validator PASS 99/100.
- Governance: PROJECT_RULES (25 rules), ARCHITECTURE, ROADMAP, CHECKPOINTS,
  SECURITY, CONTRIBUTING, AGENTS, AGENTOS docs; CI; PR/issue templates;
  CODEOWNERS; quality gates QG-001…008; milestone definitions M00…M17;
  9 Cartograph ADRs.

## What does NOT exist (deliberately)

No parsing, no resolution, no LSP, no storage, no incremental engine, no
desktop app, no MCP, no AI integration. `cartograph-parser` and
`cartograph-resolver` are empty crates reserved for M01+.

## Open risks

- exFAT working volume: no hard links (cargo falls back to copying), AppleDouble
  `._*` sidecar files; mitigated via .gitignore + `make clean-sidecars`; see
  docs/development/external-storage.md.
- Confidence priors are uncalibrated until M08.
- Node identity is graph-local until M10.

## Session log

| Date | Session outcome |
|---|---|
| 2026-08-18 | M00 executed end-to-end: workspace, domain model, graph, CLI, testkit, AgentOS vendored + adapted, governance docs, gates, CI, PR opened. Stopped per RULE 025. |
