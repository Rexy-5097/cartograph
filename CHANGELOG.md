# Changelog

All notable changes to Cartograph are recorded here.

Format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).
Versioning follows [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

Cartograph is pre-release. No version has shipped; the first public release is
v0.1.0 at milestone M09.

## [Unreleased]

### Added — M00, engineering foundation

**Rust workspace** with six crates:

- `cartograph-core` — the domain model. Node and edge kinds from the frozen
  specification, `Confidence`, `Provenance`, `Evidence`, `SourceLocation`,
  `CommitId`. Edges cannot be constructed without provenance, confidence and
  evidence; the invariant is enforced by the type system rather than by review.
- `cartograph-graph` — the architecture graph over `petgraph::StableDiGraph`,
  with traversal in both directions. No petgraph type appears in the public API.
- `cartograph-cli` — the `cartograph` binary. `cartograph version`, with
  `--json`.
- `cartograph-testkit` — fixtures and builders, including the specification's
  cross-stack chain as fixed test data.
- `cartograph-parser`, `cartograph-resolver` — reserved and deliberately empty.
  They arrive at M01–M02 and M03–M06 respectively.

**Validation**

- 50 tests across unit and integration levels.
- Criterion benchmark harness with graph-construction and traversal benchmarks.
  No result is published; the harness exists so M10 has a baseline to compare
  against.
- Workspace lints: `unsafe_code` forbidden, `missing_docs` warned, clippy
  `all` + `pedantic`.

**Governance**

- AgentOS v1.0.0 vendored under `agentos/` and adapted for Cartograph.
- Quality gates QG-001 – QG-008 with an executable runner.
- Milestone definitions M00 – M17, machine-readable project state, checkpoint
  system.
- Nine architecture decision records.
- GitHub Actions CI, pull request template, issue templates, CODEOWNERS.

### Deliberately not included

No parsing, no resolution, no LSP, no storage, no desktop application, no MCP,
no AI, no incremental analysis. Those belong to later milestones and adding them
early would break the gate model that makes rollback possible.

### Known limitations

- Node identity is graph-local and not content-addressed. Handles are not
  portable between graphs. Content-addressed identity arrives at M10.
- Confidence values are the specification's uncalibrated starting priors. No
  accuracy claim rests on them until M08 fits them against a labelled benchmark.
- `Evidence` is a validated string. Structured evidence arrives at M04, when
  there is a real resolver to describe.
- The repository's default location is an exFAT volume, which has no hard links;
  Cargo falls back to copying its incremental cache. See
  `docs/development/external-storage.md`.
