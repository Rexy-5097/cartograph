# Cartograph — Project Rules

> **Status:** Binding. **Specification:** Frozen Engineering Specification V3, August 2026.

These rules govern every change to this repository. They are not style
preferences. Each one exists because violating it would either break the
product's central claim — that its edges are derived and provable — or destroy
the project's ability to recover from a bad milestone.

A change that violates a rule needs an [ADR](docs/adr/) that supersedes the
rule, not an exception.

---

## Product architecture

| # | Rule |
|---|------|
| **001** | The Rust core is the product. |
| **002** | CLI, desktop and MCP are clients of the same core graph API. None of them holds analysis logic. |
| **003** | The Cartograph domain graph is owned by Cartograph. `petgraph` provides algorithms and never appears in a public API. |

## Evidence

| # | Rule |
|---|------|
| **004** | Every edge contains provenance. |
| **005** | Every edge contains confidence. |
| **006** | Every meaningful inferred relationship contains evidence. |

These three are enforced by the type system, not by review: `Edge` has no
`Default`, no public fields and one constructor that requires all three. See
[`crates/cartograph-core/src/edge.rs`](crates/cartograph-core/src/edge.rs).

## The model

| # | Rule |
|---|------|
| **007** | Language models never construct graph structure. |
| **008** | Language models may explain graph evidence, and only after deterministic analysis has produced it. |

RULE 007 is the product. If a model proposes an edge, every claim Cartograph
makes about provenance becomes false at once. See
[ADR-0007](docs/adr/ADR-0007-no-llm-graph-construction.md).

## Honesty about what is known

| # | Rule |
|---|------|
| **009** | Unknown values remain unknown. An unresolved segment is `{unknown}`, an unknown commit is absent — never a plausible-looking guess. |
| **010** | The resolver never hallucinates a URL, symbol, route, table or dependency. |

## Privacy

| # | Rule |
|---|------|
| **011** | No source code leaves the machine by default. |
| **012** | AI is off by default, opt-in per repository. |
| **013** | The application is fully useful with zero API keys and no network. |
| **014** | Credentials live in the OS keychain, never in source files or ordinary configuration. |
| **015** | Secrets, environment variable values, source contents and tokens are never written to logs. |

See [SECURITY.md](SECURITY.md) and [docs/security/](docs/security/).

## Milestones

| # | Rule |
|---|------|
| **016** | Every milestone has a stopping gate. |
| **017** | A failed gate prevents silent advancement. A milestone is not complete because code exists; it is complete when its gate passes. |
| **018** | Every completed milestone creates a reproducible checkpoint. |
| **019** | Every significant architecture change requires an ADR. |

## Git and review

| # | Rule |
|---|------|
| **020** | No direct development on `main`. |
| **021** | Every feature is developed on a dedicated branch (`feature/mNN-slug`). |
| **022** | Every milestone ends in a GitHub pull request. |
| **023** | Every milestone pull request requires automated validation. |
| **024** | Claude Code may self-review. Self-review is not the final authority. |
| **025** | Stop after each milestone and report status to the human project owner. |
| **026** | Every non-trivial implementation step passes a local verification loop before the next dependent step begins. |

RULE 026 is the [Continuous Verification Protocol](docs/development/continuous-verification.md),
enforced by [QG-009](agentos/gates/QG-009-continuous-verification.md). In short:

- Implement the **smallest slice**, then compile, test, inspect the output, run
  negative tests and check for regressions — before starting the next slice.
  Never accumulate a large unverified change set.
- **State the invariants first** and test them: an ambiguous match never
  creates an edge, an unknown method never becomes GET, an unknown value never
  becomes a concrete one, confidence is never presented as calibrated while it
  is not.
- **Inspect real output by hand** — a positive, a boundary, an ambiguous, a
  negative and a malformed case. Passing tests do not prove the output is right.
- **Validate against real repositories** for resolver and static-analysis work,
  and stop to classify anything unexpected rather than ignoring it.
- **Record before/after counts** when a defect is found, and add a regression
  test naming its origin.
- If verification finds a defect, **stop**: reproduce, isolate, root-cause,
  regression-test, fix, rerun, inspect. Do not patch around it.

The protocol exists because M04's fixtures were green while the matcher was
producing 138 false edges on a real repository. Optimise for finishing a
milestone with evidence that it is correct, not for finishing it quickly.

---

## Dependency policy

Every dependency must answer: **what concrete Cartograph problem does this
solve right now?**

"We might need it later" is not an answer. The frozen stack names crates that
Cartograph will eventually use; naming a crate there is permission to add it at
the milestone that needs it, not an instruction to add it now.

### Rejected for v1

Neo4j · PostgreSQL · Redis · S3 · Kafka · Kubernetes · vector databases · RAG ·
LangChain · Python in the core · custom GPU renderer · mobile · accounts ·
billing · cloud processing of source · LLM graph construction · Docker in local
development

This list is enforced mechanically by quality gate
[QG-006](agentos/gates/QG-006-architecture-compliance.md).

---

## Non-goals

Each of the following is a way to feel productive while avoiding the resolver:

1. A mobile application.
2. Team collaboration, accounts, authentication or billing.
3. More than three languages before M09.
4. Complete LSP coverage.
5. A settings panel, a dashboard, or a marketing site with testimonials.
6. A custom GPU renderer before Sigma.js is measurably the bottleneck.
7. Any cloud component before a user asks for one and offers to pay.
8. A language model anywhere inside graph construction.

---

## Publishing standard

Never publish a performance or accuracy number that has not been measured on
this codebase. Benchmark failures are published alongside successes. See
[docs/benchmarks/](docs/benchmarks/).
