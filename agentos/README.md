# AgentOS v1.0.0 — Engineering Operating System

> **Version:** 1.0.0 | **Status:** Stable Release | **License:** MIT
>
> An AI-first engineering operating system for professional engineering teams.

---

## ⚡️ Quick Start (One Prompt, Zero Setup)

### The Bootstrap Prompt

```text
You are starting a new engineering project using AgentOS v1.0.0.

Execute the following steps in order:
1. Read AGENTOS.md completely (this is the system specification).
2. Read context/state.md (current project state).
3. Read PROJECT_CONFIG.yaml (active profile and enabled features).
4. Confirm the active profile and which standards, agents, and quality gates are enabled.
5. Run the validator: python3 tools/scripts/validate_agentos.py
6. Confirm the score is 100/100.
7. Report to the engineer: "AgentOS initialized. Profile: [X]. Standards: [list]. Ready."
8. Await task assignment. Do not begin implementation until initialization confirms.
```

1. Open your AI coding assistant.
2. Paste the bootstrap prompt above.
3. The assistant will clone, bootstrap, validate, and initialize AgentOS automatically (subject to the capabilities of the AI tool you're using).

---

## Why AgentOS Exists

### The Problem

Every software team reinvents the same infrastructure: how to structure reviews, how to enforce standards, how to route tasks to the right person, how to loop on quality until it's good enough. Most teams do this informally — through Slack messages, PR comments, and tribal knowledge that disappears when engineers leave.

AI coding assistants have made this worse, not better. A developer opens Claude or Gemini and says "build a feature." The AI has no idea of the project's standards, previous decisions, or review requirements. It guesses. The result is inconsistent.

### The Solution

AgentOS is the missing operating system layer between your team and your AI assistant.

It gives your AI:
- A defined review pipeline (who reviews what, in what order)
- Engineering standards (what quality means in your domain)
- Quality gates (how to verify quality before advancing)
- Architectural memory (ADRs and decisions persisted across sessions)
- A structured execution loop (plan → implement → review → validate → release)

### Design Goals

- **Deterministic** — Same task, same process, every time
- **Vendor-neutral** — Works with Claude, Gemini, Codex, Antigravity, or any future AI
- **Token-efficient** — Minimal context loading; agents load only what they need
- **Human-controlled** — AI assists; humans decide
- **Self-validating** — The repository validates itself continuously

### Non-Goals

- AgentOS is NOT a code generator
- AgentOS is NOT a prompt template collection
- AgentOS is NOT a CI/CD system (though it integrates with one)
- AgentOS is NOT tied to any specific technology stack or language
- AgentOS does NOT make architectural decisions — it records them

---

## Architecture

```
╔══════════════════════════════════════════════════════════════════════╗
║                       AgentOS Architecture v1.0.0                    ║
╠══════════════════════════════════════════════════════════════════════╣
║                                                                      ║
║  ┌─────────────────────────────────────────────────────────────┐    ║
║  │                    Human Engineer                            │    ║
║  └──────────────────────────┬──────────────────────────────────┘    ║
║                             │                                        ║
║                             ▼                                        ║
║  ┌─────────────────────────────────────────────────────────────┐    ║
║  │                  Harness Runtime                             │    ║
║  │  Classifier → Context Optimizer → Agent Router → Planner    │    ║
║  └──────────────────────────┬──────────────────────────────────┘    ║
║                             │                                        ║
║                             ▼                                        ║
║  ┌─────────────────────────────────────────────────────────────┐    ║
║  │                   Loop Runtime                               │    ║
║  │  Iteration Controller → Reflection → Improvement Planner    │    ║
║  │  → Execution Monitor → Quality Evaluator → Termination      │    ║
║  └──────────────────────────┬──────────────────────────────────┘    ║
║                             │                                        ║
║             ┌───────────────┼───────────────┐                        ║
║             ▼               ▼               ▼                        ║
║  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐              ║
║  │ Orchestrator │  │Chief Architect│  │   Planner    │              ║
║  └──────┬───────┘  └──────┬───────┘  └──────────────┘              ║
║         │                 │                                           ║
║         ▼                 ▼ (escalation)                             ║
║  ┌─────────────────────────────────────────────────────────────┐    ║
║  │                  Specialist Reviewers                        │    ║
║  │  ai • science • security • performance • ui • qa • docs     │    ║
║  └──────────────────────────┬──────────────────────────────────┘    ║
║                             │                                        ║
║  ┌──────────────────────────▼──────────────────────────────────┐    ║
║  │                    WAT Foundation                            │    ║
║  │  workflows/    agents/    tools/    standards/   checklists/ │    ║
║  │  (SOPs)        (decisions) (exec)   (quality)    (gates)     │    ║
║  └──────────────────────────┬──────────────────────────────────┘    ║
║                             │                                        ║
║  ┌──────────────────────────▼──────────────────────────────────┐    ║
║  │              Runtime Kernel                                  │    ║
║  │  Scheduler · State Manager · Event Bus · Policy Loader      │    ║
║  │  Logger · Health Monitor                                     │    ║
║  └─────────────────────────────────────────────────────────────┘    ║
╚══════════════════════════════════════════════════════════════════════╝
```

### Execution Flow Diagram

```
Task Arrives (feature, bug, research, release)
        │
        ▼
  Bootstrap / Initialize (AGENTOS.md protocol)
        │
        ▼
  Harness Runtime (classify → route → plan)
        │
        ▼
  Loop Runtime (iterate until quality threshold)
        │
        ├──→ Agents (orchestrator → reviewer(s))
        │
        ├──→ Standards (verify quality definition)
        │
        ├──→ Checklists (milestone gate checks)
        │
        ▼
  Artifacts (ADRs, reports, release notes)
        │
        ▼
  Validator (100/100 required)
        │
        ▼
  DONE
```

---

## End-to-End Example: Building JWT Authentication

This walkthrough shows exactly how AgentOS handles a real engineering task from start to finish.

**Task:** "Build JWT Authentication"

### Step 1 — Initialize
```bash
# AI reads AGENTOS.md, loads context/state.md and PROJECT_CONFIG.yaml
python3 tools/scripts/bootstrap_project.py --profile backend --defaults
```
Files loaded: `AGENTOS.md` → `context/state.md` → `PROJECT_CONFIG.yaml`

### Step 2 — Harness Dispatches
```bash
python3 tools/scripts/harness_engine.py \
  --task "Implement JWT authentication" \
  --files "auth/jwt.py,auth/middleware.py,auth/models.py" \
  --loop-mode Balanced
```
Harness classifies: `security` domain → routes to `security-reviewer` + `ai-reviewer`

### Step 3 — Planner Scopes Work
`agents/planner.md` invoked → produces scope:
- Milestone 1: JWT token generation
- Milestone 2: Middleware validation
- Milestone 3: Refresh token flow

### Step 4 — Loop Runs (3 iterations)
```
Iteration 1: Security reviewer flags: missing token expiry validation
Iteration 2: AI reviewer flags: no rate limiting on /login endpoint
Iteration 3: Both reviewers PASS — quality threshold met
```

### Step 5 — Quality Gates Applied
```
QG-005 (security_review.md)  → PASS ✅
QG-001 (feature_completion.md) → PASS ✅
QG-002 (pull_request.md)     → PASS ✅
```

### Step 6 — Artifacts Generated
```
artifacts/decisions/ADR-0060-jwt-authentication-approach.md
artifacts/reviews/jwt-security-review.md
```

### Step 7 — Validate
```bash
python3 tools/scripts/validate_agentos.py
# Overall Grade: 100/100 | Final Status: PASS
```

### Step 8 — Done
`context/state.md` updated with JWT auth as COMPLETE.

**Total iterations:** 3 | **Quality score:** 100 | **Gate status:** ALL PASS

This is the AgentOS workflow for every engineering task.

---

## Quick Start

### Option 1: AI-Guided (Recommended)
```bash
git clone <your-agentos-repo>
cd agentos-template
python3 tools/scripts/bootstrap_project.py --interactive
# Then open your AI assistant and say:
# "Initialize this repository using AgentOS."
```

### Option 2: Make Commands
```bash
make bootstrap          # Interactive setup
make validate           # Run health check
make health             # Print health report
make test               # Run synthetic validation suite
make examples           # Run example scenarios
```

### Option 3: Defaults
```bash
python3 tools/scripts/bootstrap_project.py --defaults
python3 tools/scripts/validate_agentos.py
```

---

## Project Profiles

Select the profile that matches your project type:

| Profile | Best For | Key Standards |
|---------|----------|---------------|
| `ai_project` | ML systems, AI products | ai_ml, security, code_quality |
| `backend` | APIs, services, databases | security, api_design, testing |
| `frontend` | Web apps, UI systems | ui_ux, code_quality, testing |
| `ml` | Model training, MLOps | ai_ml, research, data_engineering |
| `research` | Scientific computation | research, documentation, testing |
| `isro` | Mission-critical systems | security, research, documentation |
| `hackathon` | Rapid prototyping | code_quality, documentation |
| `flagship` | Production flagship | All standards active |

---

## AI Workflow

```
Open AI assistant in this repository
         │
         ▼
AI reads AGENTOS.md automatically
         │
         ▼
AI loads context/state.md + PROJECT_CONFIG.yaml
         │
         ▼
AI reports: "AgentOS initialized. Ready."
         │
         ▼
You assign a task: "Build X" or "Fix Y" or "Review Z"
         │
         ▼
AI routes through Harness → Loop → Reviewers
         │
         ▼
AI validates: python3 tools/scripts/validate_agentos.py
         │
         ▼
AI reports: "Task complete. Quality: 100/100. Gate: PASS."
```

---

## Human Workflow

```
1. Clone repository
2. Run: python3 tools/scripts/bootstrap_project.py --interactive
3. Fill in context/vision.md (project goals)
4. Open AI assistant
5. Assign tasks using natural language
6. Review AI-generated artifacts and ADRs
7. Approve quality gate results
8. Merge changes
9. Update context/state.md at session end
```

---

## Repository Structure

```
agentos-template/
│
├── AGENTOS.md                  ← AI entrypoint (read first)
├── README.md                   ← This file
├── BOOTSTRAP.md                ← Bootstrap reference guide
├── START_PROJECT.md            ← New project checklist
├── ENGINEERING_PRINCIPLES.md   ← Design philosophy (frozen)
├── VERSION_POLICY.md           ← Semantic versioning rules
├── COMPATIBILITY.md            ← Upgrade paths
├── CONTRIBUTING.md             ← Contribution guide
├── CODE_OF_CONDUCT.md          ← Community standards
├── SECURITY.md                 ← Security policy
├── SUPPORTED_VERSIONS.md       ← Version support matrix
├── INSTALL.md                  ← Setup instructions
├── CHANGELOG.md                ← Version history
├── LICENSE                     ← MIT
├── VERSION                     ← 1.0.0
├── PROJECT_CONFIG.yaml         ← Active project config
├── PROJECT_CONFIG.example.yaml ← Annotated config example
├── Makefile                    ← Developer shortcuts
├── .editorconfig               ← Editor formatting rules
├── .pre-commit-config.yaml     ← Pre-commit hooks
│
├── .agentos/                   ← Runtime configuration
│   ├── config.yml              ← WAT config and trigger map
│   └── manifest.yml            ← File inventory and ownership
│
├── .devcontainer/              ← Dev container for VS Code / Cursor
│   └── devcontainer.json
│
├── .github/                    ← GitHub integration
│   ├── ISSUE_TEMPLATE/         ← Issue templates
│   ├── workflows/              ← GitHub Actions CI
│   ├── PULL_REQUEST_TEMPLATE.md
│   └── CODEOWNERS
│
├── context/                    ← Live project context
│   ├── vision.md               ← Goals and success criteria
│   ├── state.md                ← Current status (update every session)
│   ├── architecture.md         ← System design
│   ├── tech_stack.md           ← Technology choices
│   ├── decisions.md            ← ADR index
│   ├── workflow.md             ← Active master workflow
│   └── memory.md               ← Agent context buffer
│
├── workflows/                  ← Standard operating procedures
├── agents/                     ← Specialist agent contracts
├── tools/scripts/              ← CLI tooling
├── standards/                  ← Quality definitions
├── checklists/                 ← Quality gate checklists
├── metrics/                    ← Engineering targets
├── templates/                  ← Document templates
├── profiles/                   ← Project profiles
├── artifacts/                  ← Generated artifacts (never delete)
├── validation/                 ← Synthetic validation suite
├── runtime/                    ← Harness, Loop, and Kernel modules
├── integrations/               ← AI vendor adapters
├── examples/                   ← Project starter examples
├── production_certification/   ← v1.0.0 certification documents
└── lessons/                    ← Institutional memory
```

---

## Agent System

Agents are **explicitly invoked** — never automatic. One task → one primary reviewer.

| Agent | Role | Invoke When |
|-------|------|-------------|
| `orchestrator` | Routes, bundles, merges | Starting any non-trivial task |
| `chief-architect` | Decisions and conflicts | Architectural decisions, reviewer disagreements |
| `planner` | Scope and milestones | New feature, research planning |
| `ai-reviewer` | AI/ML code quality | Model code, AI pipelines, embeddings |
| `science-reviewer` | Scientific validity | Experiments, calibration, research methods |
| `security-reviewer` | Security and safety | Auth, data handling, API exposure |
| `performance-reviewer` | Speed and efficiency | Critical paths, high-load components |
| `ui-reviewer` | Interface quality | Frontend, interaction design, accessibility |
| `qa-reviewer` | Test quality | Feature completion, pre-release verification |
| `docs-reviewer` | Documentation | Docs changes, onboarding materials |
| `release-reviewer` | Release readiness | Release preparation, changelog, versioning |

---

## Standards

| Standard | Domain | Tier |
|----------|--------|------|
| `code_quality` | Code style, complexity, maintainability | 1 — Critical |
| `testing` | Coverage, design, regression | 1 — Critical |
| `documentation` | Completeness, accuracy, accessibility | 1 — Critical |
| `security` | Threats, auth, data safety | 1 — Critical |
| `ai_ml` | Model quality, bias, inference safety | 2 — Domain |
| `research` | Reproducibility, statistical validity | 2 — Domain |
| `api_design` | REST/GraphQL design, versioning | 2 — Domain |
| `data_engineering` | Pipeline quality, data validity | 2 — Domain |
| `ui_ux` | Accessibility, usability, consistency | 2 — Domain |

---

## Documentation

| Document | Purpose |
|----------|---------|
| [AGENTOS.md](./AGENTOS.md) | AI initialization protocol |
| [ARCHITECTURE.md](./ARCHITECTURE.md) | **21 Mermaid diagrams** — every system visualized |
| [BOOTSTRAP.md](./BOOTSTRAP.md) | Bootstrap reference |
| [START_PROJECT.md](./START_PROJECT.md) | New project checklist |
| [INSTALL.md](./INSTALL.md) | Setup instructions |
| [TEAM_QUICKSTART.md](./TEAM_QUICKSTART.md) | 10-minute zero-knowledge guide |
| [SUPPORT.md](./SUPPORT.md) | Support channels and troubleshooting |
| [DOCUMENTATION_INDEX.md](./DOCUMENTATION_INDEX.md) | Full documentation registry |
| [ENGINEERING_PRINCIPLES.md](./ENGINEERING_PRINCIPLES.md) | Design philosophy |
| [CONTRIBUTING.md](./CONTRIBUTING.md) | How to contribute |
| [CHANGELOG.md](./CHANGELOG.md) | Version history |
| [COMPATIBILITY.md](./COMPATIBILITY.md) | Upgrade paths |

---

## FAQ

**Q: Which AI assistants does AgentOS support?**
A: Claude Code, Antigravity, Gemini, Codex, and any future AI assistant that can read markdown files. All vendor-specific adapters are in `integrations/`.

**Q: Does AgentOS require a specific programming language?**
A: No. AgentOS is language and framework agnostic. The runtime is Python but your project can use any language.

**Q: What happens if the validator score drops below 100?**
A: Surface the warnings to the human engineer before proceeding. Do not declare any task DONE with a failing validator.

**Q: How do I add a new agent?**
A: See [CONTRIBUTING.md](./CONTRIBUTING.md). In short: create `agents/{role}.md`, add to `.agentos/config.yml`, update `agents/README.md`, log an ADR, bump the MINOR version.

**Q: Can I customize standards and checklists?**
A: Yes. Standards and checklists are project-adaptable. Update them in your project copy. Never modify the AgentOS template defaults without bumping the version.

**Q: How do I update AgentOS itself?**
A: See [COMPATIBILITY.md](./COMPATIBILITY.md) for upgrade paths. v1.x.y releases are backward compatible. Breaking changes only occur in major versions.

---

## Troubleshooting

| Problem | Solution |
|---------|----------|
| Validator score < 100 | Read warning details at the bottom of the report |
| `bootstrap_project.py` fails | Run `--self-test` first; check that all `profiles/` YAML files exist |
| Agent not routing correctly | Check `.agentos/config.yml` trigger map |
| Loop exits too early | Switch `--loop-mode` to `Exhaustive` |
| Broken link in docs | Run the validator — cross-reference check will surface it |
| Profile YAML missing | Check `profiles/` directory; run `make validate` |

---

## Versioning

**Current version:** `1.0.0` — Stable Release

AgentOS follows [Semantic Versioning](https://semver.org):
- `MAJOR` — Breaking changes to agent contracts, workflow schemas, or runtime APIs
- `MINOR` — New agents, new profiles, new standards (backward compatible)
- `PATCH` — Bug fixes, documentation updates, validator improvements

See [VERSION_POLICY.md](./VERSION_POLICY.md) for the full versioning policy.

---

## Known Limitations

- The Harness and Loop runtimes are **simulation engines** for engineering process orchestration. They do not call external AI APIs — they model the process locally.
- The validator performs **static analysis** only. It does not execute agent code.
- Project profiles activate feature flags but do not enforce them programmatically — enforcement is through the validator and review process.
- The synthetic validation suite runs deterministic simulations, not live AI evaluations.

---

## Future Releases

AgentOS v1.1.0 (planned — not scheduled):
- Live AI provider integration in Harness
- Real-time validation webhooks
- Multi-agent parallel review execution
- Project dashboard UI
- AgentOS CLI (`agentos init`, `agentos run`, `agentos report`)

See `production_certification/FUTURE_ROADMAP.md` for the full roadmap.

---

## Contributing

See [CONTRIBUTING.md](./CONTRIBUTING.md). All contributors must follow [CODE_OF_CONDUCT.md](./CODE_OF_CONDUCT.md).

---

## License

MIT. See [LICENSE](./LICENSE).

---

*AgentOS v1.0.0 — Engineering infrastructure built for decades, not deadlines.*
