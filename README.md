# Cartograph

**Compute the architecture. Prove the relationships.**

Cartograph is a local-first architectural intelligence engine. It derives a
symbol-level, cross-language graph of a software system from source code,
language semantics, HTTP boundaries, database schemas and Git history — and
shows the evidence behind every edge it draws.

> **Status: pre-alpha, milestone M00 of 17.** The engineering foundation exists.
> The resolver does not. `cartograph analyze` is not implemented yet. Nothing in
> this README describes behaviour the code does not have — see
> [What works today](#what-works-today).

---

## The problem

Every tool in this category draws a file-level import graph and calls it
architecture. Ask one what a checkout button touches and it answers with the
files that import each other. That is a lexical fact; the question was a causal
one, and the gap between them is where production incidents live.

## What is different

Cartograph resolves relationships **across** the stack, not merely within a
language. A React component, an HTTP route, a Python handler and a database
table are four artefacts in three languages with no shared symbol table:

```
CheckoutButton.tsx  onSubmit() line 34
        │  HTTP POST /api/orders          0.94  template-eval
        ▼
POST /api/orders/{*}  (canonical)
        │  route match                    0.98  route-matcher
        ▼
orders.py  create_order() line 12
        │  ORM Order(...)                 0.97  orm-resolve
        ▼
orders — table
```

Doing this requires partial evaluation of dynamically constructed URLs,
normalisation across four route-declaration dialects, and ORM model
resolution — three compiler problems, none of which a GitHub API call can
answer.

### Never `A → B`

Cartograph stores the claim *and how it was known*:

```json
{
  "source": "web/src/CheckoutButton.tsx:34",
  "target": "api/orders.py:12 create_order",
  "kind": "http-call",
  "confidence": 0.94,
  "provenance": "route-matcher",
  "evidence": "POST /api/orders",
  "commit": "a3f9c21",
  "created_at": "2026-08-17T09:14:22Z"
}
```

Click any edge and you get its record. Competitors assert relationships;
Cartograph produces evidence for them.

### The governing principle

**Deterministic analysis first. Evidence second. Inference third. The model
last, and never in the graph.**

No language model constructs graph structure — ever. A model may explain a
subgraph after static analysis has produced it (M16). It may never propose an
edge. This is an immutable architectural principle, not a current limitation:
see [ADR-0007](docs/adr/ADR-0007-no-llm-graph-construction.md).

---

## What works today

M00 delivered the engineering foundation. Concretely:

| Component | State |
|---|---|
| `cartograph-core` — domain model, evidence, provenance, confidence | Implemented, tested |
| `cartograph-graph` — architecture graph over `petgraph` | Implemented, tested |
| `cartograph-cli` — `cartograph version` | Implemented |
| `cartograph-parser` — tree-sitter extraction | **Empty. M01–M02.** |
| `cartograph-resolver` — cross-language resolution | **Empty. M03–M06.** |
| `cartograph-testkit` — fixtures | Implemented |

```console
$ cargo run -p cartograph-cli -- version
cartograph 0.0.0
specification V3
milestone M00
```

There are no accuracy numbers, no performance numbers and no benchmark results,
because none have been measured. See [docs/benchmarks/](docs/benchmarks/) for
what will be measured and when.

---

## The eventual product

Three surfaces, in build order:

| Surface | Question | Milestone |
|---|---|---|
| **MAP** | What is this system? | M11 |
| **BLAST** | What breaks if I change this? | M12 |
| **DIFF** | What did this change do to the architecture? | M13 |

The first finish line is one command on one real repository:
`cartograph analyze .` (M07). Everything else is downstream of it.

Full plan: [ROADMAP.md](ROADMAP.md).

---

## Privacy

- No source code leaves the machine. Default and unconditional.
- AI is opt-in, per repository, off by default. Cartograph is fully useful with
  no API key and no network.
- Credentials live in the OS keychain, never in configuration files.
- Source contents, tokens and environment variable values are never logged.

See [SECURITY.md](SECURITY.md).

---

## Building

Requires Rust 1.85 or later.

```bash
make check
```

That runs the same gates CI runs: formatting, clippy with warnings denied,
tests, and the AgentOS validator. Individual targets are listed by `make help`.

---

## Repository layout

```
crates/          The product. Six Rust crates.
docs/            Architecture, resolver design, benchmarks, security, ADRs.
agentos/         Vendored AgentOS v1.0.0 — development governance, not product code.
.github/         CI and pull request governance.
```

`agentos/` orchestrates how this project is built (milestones, gates,
checkpoints). It is deliberately separate from the implementation and ships no
runtime code. See [AGENTOS.md](AGENTOS.md).

---

## Governance

| Document | Purpose |
|---|---|
| [PROJECT_RULES.md](PROJECT_RULES.md) | The 25 binding rules |
| [ARCHITECTURE.md](ARCHITECTURE.md) | System architecture |
| [ROADMAP.md](ROADMAP.md) | 17 milestones and their gates |
| [CHECKPOINTS.md](CHECKPOINTS.md) | Reproducible rollback points |
| [AGENTS.md](AGENTS.md) | Instructions for AI coding assistants |
| [CONTRIBUTING.md](CONTRIBUTING.md) | How to contribute |
| [docs/adr/](docs/adr/) | Architecture decision records |

---

## Licence

[Apache-2.0](LICENSE).

The vendored AgentOS framework under `agentos/` is MIT-licensed and retains its
own [LICENSE](agentos/LICENSE).
