# Cartograph — Roadmap

Seventeen milestones. The first seven occupy fourteen days, because the
resolver either works quickly or reveals early that it does not.

**Build order is resolver → graph → CLI → benchmark → app → blast → diff → MCP
→ ask, and never the reverse.** Do not build the visual map before the resolver
produces useful data. Do not build a chatbot before the graph is trustworthy.

Every milestone has: scope, acceptance criteria, tests, a quality gate, a pull
request, a checkpoint tag, a rollback state and a report. A milestone is
complete when its **gate passes**, not when its code exists (RULE 017).

Every milestone is also built under the
[Continuous Verification Protocol](docs/development/continuous-verification.md)
(RULE 026, gate QG-009): verified slices rather than one late check, real
repositories for resolver work, and a Verification Findings section in the
report. The high-risk milestones — M04–M08, M10, M12, M13 — require adversarial
real-repository validation.

---

## Status

| Milestone | Deliverable | Target | Status |
|---|---|---|---|
| **M00** | Workspace, six crates, CI, clippy, criterion, tracing | Day 2 | **Accepted** (`cartograph-m00`) |
| **M01** | TypeScript extraction — symbols, imports, calls, strings | Day 4 | **Accepted** (`cartograph-m01`) |
| **M02** | Python extraction — FastAPI, Flask, Django routes | Day 5 | **Accepted** (`cartograph-m02`) |
| **M03** | Path normalisation to canonical form | Day 6 | **Accepted** (`cartograph-m03`) |
| **M04** | Cross-language resolver — TS call to Python handler | Day 8 | **Accepted** (`cartograph-m04`) |
| **M05** | Template evaluator for dynamic URLs | Day 11 | **Accepted** (`cartograph-m05`) |
| **M06** | ORM resolution — handler to model to table | Day 13 | **Accepted** (`cartograph-m06`) |
| M07 | Full verified chain on a real repository | Day 14 | **Accepted** (`cartograph-m07`) |
| M08 | Golden fixtures, CrossStack-Bench, calibration | Week 4 | **Accepted** (`cartograph-m08`) |
| M09 | CLI polish, trace, JSON output, v0.1.0 public | Week 6 | **Accepted** (`cartograph-m09`) |
| M10 | Incremental engine | Week 8 | **Accepted** (`cartograph-m10`) — Stage C only, see [ADR-0014](docs/adr/ADR-0014-m10-scope-reconciliation.md) |
| M11 | Desktop app — Tauri, Sigma.js, MAP | Week 11 | **Accepted** (`cartograph-m11`) |
| M12 | Blast radius across language boundaries | Week 13 | **Accepted** (`cartograph-m12`) |
| M13 | Structural diff, branch against branch | Week 16 | **Accepted** (`cartograph-m13`) |
| M14 | GitHub Action posting architecture review on PRs | Week 19 | **Accepted** (`cartograph-m14`) |
| M15 | MCP server | Week 21 | **Accepted** (`cartograph-m15`) |
| M16 | ASK — explanation over subgraph evidence | Week 24 | **Unlocked, not started** |
| M17 | Public launch and technical writeup | Week 25 | Not started |

Statuses here are the human-readable view; the authoritative record is
[`agentos/artifacts/project-state.yaml`](agentos/artifacts/project-state.yaml),
and accepted milestones are pinned by immutable tags (see
[CHECKPOINTS.md](CHECKPOINTS.md)).

Machine-readable state: [`agentos/artifacts/project-state.yaml`](agentos/artifacts/project-state.yaml).
Milestone definitions: [`agentos/milestones/`](agentos/milestones/).

---

## Release ladder

Every version must be independently usable. Ship ugly at v0.1; do not wait for
v1.0.

| Version | Brings | Milestone |
|---|---|---|
| 0.1 | CLI | M09 |
| 0.2 | Local graph database | M10 — **deferred**, see [ADR-0014](docs/adr/ADR-0014-m10-scope-reconciliation.md) |
| 0.3 | Incremental analysis | M10 — Stage C only; see ADR-0014 |
| **0.4** | **MAP** — the desktop application | M11 — **delivered** |
| 0.5 | Blast radius | M12 |
| **0.6** | **DIFF** — the retention surface | M13 — **delivered** |
| **0.7** | **GitHub PR integration** | M14 — **delivered** |
| **0.8** | **MCP** — the agent surface | M15 — **delivered** |
| 0.9 | ASK | M16 |
| **1.0** | Stable | M17 |

v0.1 proves the engine. v0.6 creates retention. v1.0 is a product.

---

## Stopping gates

Milestones without stopping rules are how six-month projects become
eighteen-month projects. Each of these is a decision point, not a
disappointment.

### Gate 1 — M07 slips past day 21

The resolver exceeds two-person capacity. **Narrow to TypeScript → FastAPI
only.** Drop Flask, Django and Express. Do not expand language coverage for
feature count.

### Gate 2 — M08 cross-stack F1 below 0.70

Cross-stack resolution may not be reliably solvable by static analysis. Do not
pretend the resolver is solved. **Publish CrossStack-Bench as a standalone
contribution, publish the failures, and stop.** A benchmark that proves a
problem is hard is a real contribution.

### Gate 3 — M09 + 30 days, fewer than 100 real users

Positioning is wrong, not the product. **Rewrite the pitch before building the
desktop application.**

### Gate 4 — M13 + 30 days, under 15% seven-day return

MAP is a poster, not a tool. **Cut MAP investment and commit entirely to DIFF.**

### Gate 5

Never continue indefinitely without evaluating these criteria.

---

## Metrics that matter

Not stars. Repositories analysed · analysis failure rate · cross-language
precision and recall · median analysis time · incremental update latency ·
weekly active users · seven-day retention · MCP queries served · pull requests
analysed.

First meaningful target: one hundred real users, twenty weekly active, ten
returning, five using it inside real projects.

---

## Performance targets

Targets, **not measurements**. Nothing here has been benchmarked; publishing an
unmeasured number would violate RULE 025's spirit and the publishing standard.

| Scenario | v1 target | Later target |
|---|---|---|
| 10k files, initial analysis | under 60 s | under 10 s warm |
| Normal file change, incremental | under 250 ms | under 100 ms |
| Rendering, 10k nodes | 60 FPS | benchmark 50k / 100k / 200k |

---

## Distribution

| Channel | Mechanism | Milestone |
|---|---|---|
| `cargo install` | crates.io | M09 |
| `npx cartograph` | Thin npm wrapper downloading a prebuilt platform binary | M09+ |
| `brew install` | Homebrew tap | M09+ |
| GitHub Action | Runs the binary in the user's own CI — no server, no hosting cost | M14 |
| MCP | Claude Code and Cursor query the graph directly | M15 |

Running analysis in the user's CI rather than on hosted infrastructure holds
operating cost at zero through launch. Do not undo that without a reason.
