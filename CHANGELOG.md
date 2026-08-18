# Changelog

All notable changes to Cartograph are recorded here.

Format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).
Versioning follows [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

Cartograph is pre-release. No version has shipped; the first public release is
v0.1.0 at milestone M09.

## [Unreleased]

### Added — M01, TypeScript extraction

**The first real analyzer.** `cartograph-parser` now extracts syntactic facts
from TypeScript and TSX using tree-sitter (the runtime is contained in one
private module; the public API is Cartograph's own language-neutral fact
model):

- **Imports** — default, namespace, named (+aliases), type-only, side-effect.
- **Symbols** — functions (+async), classes, methods, module-scope variables,
  interfaces, type aliases, enums; enclosing-symbol tracking; export status
  including `export {…}` clauses and anonymous `export default` expressions.
- **Call sites** — `foo()`, `a.b.c()`, `new Foo()`, argument counts.
  Computed/unreliable callees are skipped, not guessed.
- **Strings** — literals (escapes as written), template literals with full
  substitution structure preserved for M05's evaluator (never pre-resolved),
  flattened `+` concatenation chains.
- **HTTP observations** — `fetch`/`axios.*`/known-client verb calls recorded
  as *syntactic observations* with method hints (never defaulted) and
  literal/template/dynamic URL shapes. Observations are explicitly not
  `HttpCall` edges; the resolver creates edges at M04.
- **Diagnostics** — error-tolerant extraction: broken regions become
  structured, source-text-free diagnostics; the rest of the file (and the
  repository walk) continues. Non-UTF-8 and unreadable files fail softly.
- **CLI** — `cartograph parse <path> [--json]` walks a file or tree and
  reports facts and diagnostics. `analyze` remains absent until M09.
- **Tests** — 28 parser integration tests over a fixture corpus (all 22
  mandated syntax categories), 4 new CLI tests; 82 workspace tests total.
- **Benchmarks** — Criterion `ts_extraction` group (single small file,
  synthetic 50/500-function modules, 100-file synthetic project), fully
  deterministic in-memory inputs.

**Checkpoint policy repair (ADR-0010).** Provisional checkpoints
(`cartograph-mNN-rcK`, exact PR head) are now distinct from accepted
checkpoints (`cartograph-mNN`, exact merge commit, created only at human
acceptance). The premature `cartograph-m00` tag was retired after a safety
determination and replaced by `cartograph-m00-rc1`.

Dependencies added: `tree-sitter` 0.26, `tree-sitter-typescript` 0.23 —
required by this milestone's actual parsing work. No LSP, no Python grammar,
no future-milestone dependency.

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
