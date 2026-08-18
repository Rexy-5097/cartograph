# AgentOS integration

Cartograph's development process is governed by **AgentOS v1.0.0**, vendored
under [`agentos/`](agentos/).

AgentOS is a deterministic engineering operating system: workflows (SOPs),
agents (decisions) and tools (execution) in separate layers, with quality gates,
artefacts and architecture decision records. It governs *how this project is
built*. It is not part of the product and ships no runtime code.

## Why vendored rather than referenced

Upstream lives at [`Rexy-5097/raptors-way`](https://github.com/Rexy-5097/raptors-way).
It is vendored so that checking out any Cartograph commit reproduces the exact
governance that was in force when that commit was made — a checkpoint that
cannot reproduce its own rules is not a checkpoint. See
[ADR-0009](docs/adr/ADR-0009-vendored-agentos.md).

## Layout

```
agentos/
  AGENTOS.md                     Framework initialisation protocol
  PROJECT_CONFIG.yaml            Cartograph's profile and enabled features
  VERSION                        Framework version (1.0.0) — not Cartograph's
  agents/                        11 reviewer/authority agent contracts
  workflows/                     Standard operating procedures
  standards/                     What quality means, per domain
  checklists/                    Framework gate checklists
  context/                       Cartograph's project layer (vision, state, …)
  gates/                         Cartograph's quality gates QG-001 … QG-008
  milestones/                    M00 … M17 definitions and acceptance criteria
  artifacts/
    project-state.yaml           Machine-readable milestone state
    decisions/                   Upstream AgentOS ADRs (framework history)
  tools/scripts/
    validate_agentos.py          Framework health validator
    run_gates.py                 Cartograph's executable quality gates
  runtime/, validation/          Framework kernel and validation suite
```

## Two ADR namespaces, deliberately

| Location | Contains |
|---|---|
| [`docs/adr/`](docs/adr/) | **Cartograph's** architecture decisions |
| `agentos/artifacts/decisions/` | Upstream AgentOS's own 56 framework ADRs |

Cartograph's decisions do not belong in a vendored dependency's history, and
the framework's history is worth keeping rather than deleting.

## Quality gates

AgentOS ships eight generic checklist gates. Cartograph defines eight
**project-specific** gates with the same numbering, in
[`agentos/gates/`](agentos/gates/), because a Rust static-analysis engine needs
`cargo clippy -D warnings` rather than a generic "code reviewed" checkbox.

| Gate | Cartograph meaning | Enforced by |
|---|---|---|
| QG-001 | Repository integrity | `run_gates.py` |
| QG-002 | Formatting | `cargo fmt --check` |
| QG-003 | Lints | `cargo clippy -D warnings` |
| QG-004 | Tests | `cargo test --workspace` |
| QG-005 | Security and secrets | `run_gates.py` |
| QG-006 | Architecture compliance | `run_gates.py` |
| QG-007 | Documentation and change tracking | `run_gates.py` |
| QG-008 | Milestone acceptance criteria | `run_gates.py` |

The framework's own checklist mapping is preserved in `agentos/AGENTOS.md` and
still applies to framework validation. Where the two disagree for Cartograph's
purposes, `agentos/gates/` wins.

Later milestones add: benchmark correctness, cross-stack precision and recall,
calibration, performance, incremental latency, rendering performance, MCP
correctness, privacy verification, release verification.

## Running it

```bash
make validate      # AgentOS framework health
make gates         # Cartograph quality gates QG-001 … QG-008
make check         # fmt + clippy + test + validate
```

## Modification to upstream

Exactly one, recorded in [ADR-0009](docs/adr/ADR-0009-vendored-agentos.md):
`tools/scripts/validate_agentos.py` had its repository root hardcoded to the
framework author's machine. It now resolves relative to the script, overridable
with `AGENTOS_ROOT`.

Two upstream files are deliberately **not** vendored:

| Omitted | Reason |
|---|---|
| `.devcontainer/devcontainer.json` | Docker in local development is rejected for v1 |
| `.pre-commit-config.yaml` | A Python pre-commit hook chain; Cartograph gates through `make` and CI |

The framework validator counts both as missing, which is why its Distribution
category scores 90 rather than 100. That is an accurate report of a deliberate
choice, and is preferred to adding Docker configuration to raise a score.

## Current health

```
Overall Grade  : 99/100
Final Status   : PASS
```

Reproduce with `make validate`.
