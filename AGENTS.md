# Instructions for AI coding assistants

Read this before changing anything in this repository.

## Before you start

1. Read [PROJECT_RULES.md](PROJECT_RULES.md) — 25 binding rules.
2. Read [`agentos/artifacts/project-state.yaml`](agentos/artifacts/project-state.yaml) — which milestone is active and which is permitted next.
3. Read the milestone definition in [`agentos/milestones/`](agentos/milestones/).
4. Read [ARCHITECTURE.md](ARCHITECTURE.md) if the change touches structure.

## The rules that get broken most often

**Do not implement the next milestone.** Execute the current one only, then
stop and report (RULE 025). `project-state.yaml` names the next permitted
milestone; anything beyond it is out of scope even if it seems small.

**Do not let a language model construct graph structure** (RULE 007). Not as a
fallback, not as a "hint", not behind a flag. Every edge is computed by static
analysis. A model may explain a finished subgraph (M16) and nothing else.

**Do not guess.** An unresolved URL segment is `{unknown}`. An unknown commit is
`None`. A route that did not match produces no edge. A plausible-looking guess
is worse than a gap, because it is indistinguishable from a derived fact
(RULES 009, 010).

**Do not add a dependency speculatively.** Every one must answer "what concrete
Cartograph problem does this solve right now?" The frozen stack naming a crate
is permission to add it *at the milestone that needs it*.

**Do not publish an unmeasured number.** No accuracy or performance claim
without a benchmark run on this codebase.

## Working agreement

- Work on `feature/mNN-slug`, branched from the latest `main`. Never commit to
  `main` (RULES 020, 021).
- Conventional commits: `feat:`, `fix:`, `refactor:`, `test:`, `docs:`, `perf:`,
  `build:`, `ci:`, `chore:`. Atomic and meaningful — never "changes", "work",
  "update", "stuff".
- One pull request per coherent milestone, not per commit.
- Run `make check` before pushing. It runs what CI runs.
- Do not force-push. Do not rewrite `main` history. Never delete or move a
  milestone tag.

## Roles

Claude Code may perform several of these; AgentOS preserves the separation of
responsibility.

| Role | Owns |
|---|---|
| Architect | Architecture, ADRs, dependency decisions, domain boundaries |
| Core engineer | Rust core, graph, storage, performance |
| Parser engineer | tree-sitter, AST extraction, language-specific analysis |
| Resolution engineer | LSP, HTTP matching, cross-language and ORM resolution |
| Research engineer | Dynamic string analysis, confidence calibration, benchmarking, layout |
| Desktop engineer | Tauri, React, Sigma, interaction |
| Integration engineer | Git, GitHub, PR integration, MCP |
| QA engineer | Tests, golden fixtures, regression detection, benchmarks |
| Security engineer | Privacy, credential handling, secret redaction, MCP access |
| Release engineer | CI, releases, packaging, Homebrew, npm wrapper, crates.io |

Reviewer agent contracts: [`agentos/agents/`](agentos/agents/).

## Definition of done

A milestone is done when its quality gate passes — not when the code compiles.

```bash
make check          # fmt, clippy, test, AgentOS validator
make gates          # QG-001 … QG-008
```

Then: commit, push the branch, open a pull request, wait for CI, self-review the
diff, update `CHECKPOINTS.md` and `project-state.yaml`, report to the human
owner, and **stop**.

Self-review is required and is not the final authority (RULE 024). The human
project owner decides whether to advance.
