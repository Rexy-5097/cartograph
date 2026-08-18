# Start a New Project with AgentOS

> **Purpose:** The canonical entry point for starting a new project inside this repository.
>
> Read `AGENTOS.md` first. Then follow this checklist.

---

## Zero-Knowledge Start

If you know nothing about AgentOS, this is all you need:

```bash
# 1. Clone
git clone <repo-url>
cd agentos-template

# 2. Bootstrap
make bootstrap

# 3. Open AI assistant and say:
# "Initialize this repository using AgentOS."

# 4. Verify
make validate
```

Done. Your AI assistant handles the rest.

---

## Initialization Prompt (Copy–Paste Ready)

Paste this into your AI assistant for a complete initialization:

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

---

## Project Initialization Checklist

Complete these steps for every new project:

### Phase 1 — Repository Setup
- [ ] Clone repository
- [ ] Run `make bootstrap` or `python3 tools/scripts/bootstrap_project.py --interactive`
- [ ] Confirm `PROJECT_CONFIG.yaml` is generated
- [ ] Run `make validate` → must show 100/100

### Phase 2 — Context Population
- [ ] Edit `context/vision.md` — describe your project goals and success criteria
- [ ] Edit `context/state.md` — set first sprint tasks
- [ ] Edit `context/tech_stack.md` — document your technology choices
- [ ] Edit `context/architecture.md` — sketch initial system design

### Phase 3 — AI Initialization
- [ ] Open AI assistant in this directory
- [ ] Paste initialization prompt above or say "Initialize this repository using AgentOS"
- [ ] AI confirms initialization (profile, standards, gates)
- [ ] AI confirms `100/100 PASS`

### Phase 4 — First Task
- [ ] Assign first engineering task to the AI
- [ ] Confirm Harness routes to correct reviewer(s)
- [ ] Loop executes and meets quality threshold
- [ ] Quality gate(s) PASS
- [ ] `context/state.md` updated
- [ ] Artifact created if architectural decision was made

---

## Supported AI Assistants

| Assistant | How to Initialize | Notes |
|-----------|------------------|-------|
| **Antigravity** | Auto-discovers `AGENTOS.md` | Native integration |
| **Claude Code** | Auto-reads `AGENTOS.md` | Paste init prompt if needed |
| **Gemini** | Provide `AGENTOS.md` as context | Paste init prompt |
| **Codex / Copilot** | Open `AGENTOS.md` in editor tab | Paste init prompt in Chat |

See `integrations/{assistant}/README.md` for assistant-specific guidance.

---

## Selecting a Project Profile

```bash
make profile P=<name>
```

| Profile | Use When |
|---------|---------|
| `ai_project` | Building AI/ML products |
| `backend` | APIs, services, databases |
| `frontend` | Web apps, UI systems |
| `ml` | Model training, MLOps |
| `research` | Scientific computation |
| `isro` | Mission-critical systems |
| `hackathon` | Rapid prototyping |
| `flagship` | Full-scale production (all standards active) |

---

## Looking at Examples

Before starting, review the starter example for your project type:

```bash
cat examples/backend/START.md     # Backend API service
cat examples/frontend/START.md    # Web application
cat examples/ai_project/START.md  # AI/ML product
cat examples/research/START.md    # Scientific research
cat examples/flagship/START.md    # Production flagship
```

Each example shows the expected execution flow, quality gates, and generated artifacts.

---

## Failure Recovery

| Problem | Recovery |
|---------|---------|
| Bootstrap interrupted | `python3 tools/scripts/bootstrap_project.py --resume` |
| Validator fails | Read warnings — do not proceed until 100/100 |
| AI doesn't load protocol | Ensure `AGENTOS.md` is at repository root |
| Missing profile | `make self-test` — identifies missing profiles |
| Wrong profile selected | Re-run `make bootstrap` and choose correct profile |

---

## See Also

- [AGENTOS.md](./AGENTOS.md) — Full AI protocol specification
- [BOOTSTRAP.md](./BOOTSTRAP.md) — Bootstrap reference
- [TEAM_QUICKSTART.md](./TEAM_QUICKSTART.md) — 5-minute guide
- [ARCHITECTURE.md](./ARCHITECTURE.md) — Visual system diagrams
- [examples/](./examples/) — Starter project templates
