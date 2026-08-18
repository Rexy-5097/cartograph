# Teammate Quick Start — AgentOS v1.0.0

> You need zero prior AgentOS knowledge. This guide gets you to your first feature in under 10 minutes.

---

## What Is AgentOS?

An AI-first engineering operating system. It gives your AI assistant a structured review pipeline, engineering standards, quality gates, and architectural memory — so development is consistent, high-quality, and repeatable.

**You don't need to read the whole framework.** The AI reads it for you.

---

## Step 1: Prerequisites

```bash
python3 --version    # 3.8 or higher required
git --version        # any version
pip install pyyaml   # only dependency
```

---

## Step 2: Clone & Bootstrap

```bash
git clone <your-agentos-repo-url>
cd agentos-template

# Option A — Guided (recommended for first-timers)
make bootstrap

# Option B — Quick defaults
make bootstrap-quick

# Option C — Specific project type
make profile P=backend       # or: ai_project | frontend | ml | research | flagship
```

> This generates `PROJECT_CONFIG.yaml`, `context/vision.md`, and `context/state.md`.

---

## Step 3: Verify Installation

```bash
make validate
```

Expected output: `Overall Grade: 100/100 | Final Status: PASS`

If you see anything other than 100/100, read the warning output and fix before proceeding.

---

## Step 4: Open Your AI Assistant

Open Claude Code, Antigravity, Gemini, Codex, or any AI coding assistant in this directory.

Then say exactly this:

> **"Initialize this repository using AgentOS."**

The AI will:
1. Discover and read `AGENTOS.md` automatically
2. Load `context/state.md` and `PROJECT_CONFIG.yaml`
3. Report: *"AgentOS initialized. Profile: [X]. Ready."*

You should receive a readiness confirmation before any development begins.

---

## Step 5: Assign Your First Task

Once the AI confirms initialization, assign a task in natural language:

> *"Build the user authentication feature."*
> *"Fix the database connection timeout bug."*
> *"Review the API design for the payments endpoint."*

The AI routes through the Harness, runs the Loop, invokes specialist reviewers, and reports quality gates.

---

## Step 6: End of Session

At the end of every working session:

```bash
make validate        # confirm 100/100
```

The AI should also update `context/state.md` with completed tasks and open items.

---

## Available Commands

```bash
make bootstrap        # Interactive setup
make bootstrap-quick  # Setup with defaults
make profile P=X      # Setup with specific profile
make validate         # Run full health check
make health           # Quick one-line health summary
make test             # Run all 21 validation scenarios
make self-test        # Verify all 8 profiles are healthy
make examples         # View example project flows
make clean            # Remove temp files
```

---

## Choosing a Profile

| Profile | Use When |
|---------|---------|
| `ai_project` | Building an AI product or ML pipeline |
| `backend` | Building APIs, services, or databases |
| `frontend` | Building web apps or UI systems |
| `ml` | Training models, MLOps, data pipelines |
| `research` | Scientific experiments or research |
| `isro` | Mission-critical or safety-critical systems |
| `hackathon` | Rapid prototyping, time-limited build |
| `flagship` | Full-scale production flagship project |

---

## Need More Help?

- [ARCHITECTURE.md](./ARCHITECTURE.md) — Visual diagrams of every system
- [BOOTSTRAP.md](./BOOTSTRAP.md) — Bootstrap reference
- [DOCUMENTATION_INDEX.md](./DOCUMENTATION_INDEX.md) — Full docs registry
- [SUPPORT.md](./SUPPORT.md) — Support channels and troubleshooting
- [examples/](./examples/) — Starter flows for backend, frontend, AI, research, flagship
