# Example: Backend Service Project

> **Profile:** `backend` | **Standards:** security, api_design, testing, code_quality
>
> This example shows how to use AgentOS to build a production backend API service.

---

## Project Summary

**Goal:** Build a RESTful API service with authentication, database persistence, and observability.
**Tech Stack:** Python + FastAPI + PostgreSQL + Redis
**Profile:** `backend`

---

## Step 1: Bootstrap

```bash
python3 tools/scripts/bootstrap_project.py --profile backend --defaults
# OR
make profile P=backend
```

Generated files:
- `PROJECT_CONFIG.yaml` — with backend feature flags active
- `context/vision.md` — pre-populated with backend template
- `context/state.md` — initialized to ACTIVE

---

## Step 2: Initialize AI Assistant

Open your AI assistant (Claude Code, Antigravity, Gemini, or Codex).

Say: **"Initialize this repository using AgentOS."**

The AI will:
1. Read `AGENTOS.md`
2. Load `context/state.md` + `PROJECT_CONFIG.yaml`
3. Report: "AgentOS initialized. Backend profile active. Standards: security, api_design, testing, code_quality."

---

## Step 3: Assign First Task

Say: **"Implement JWT authentication for the /login and /refresh endpoints."**

**Expected AgentOS execution flow:**

```
Task: Implement JWT authentication
        │
        ▼
Harness classifies: security + backend domain
        │
        ▼
Routes to: security-reviewer + ai-reviewer
        │
        ▼
Planner scopes:
  - Milestone 1: Token generation (access + refresh)
  - Milestone 2: Middleware validation
  - Milestone 3: Logout and token revocation
        │
        ▼
Loop Iteration 1:
  security-reviewer flags: missing token expiry
  ai-reviewer flags: no rate limiting on /login
        │
Loop Iteration 2:
  security-reviewer: token expiry added → PASS
  ai-reviewer: rate limiting added → PASS
        │
        ▼
Quality Gates:
  QG-005 (security_review.md) → PASS ✅
  QG-001 (feature_completion.md) → PASS ✅
  QG-002 (pull_request.md) → PASS ✅
        │
        ▼
Artifact generated:
  artifacts/decisions/ADR-0060-jwt-auth-approach.md
        │
        ▼
Validator: 100/100 ✅
```

---

## Step 4: Expected Artifacts

After completing the JWT feature, your repository should contain:

```
artifacts/
├── decisions/
│   └── ADR-0060-jwt-authentication-approach.md
└── reviews/
    └── jwt-security-review.md

context/
└── state.md  ← updated: JWT auth COMPLETE
```

---

## Step 5: Release

When ready to release:

```bash
python3 tools/scripts/harness_engine.py \
  --task "Prepare v1.0.0 backend service release" \
  --loop-mode Balanced
```

Expected quality gates: QG-006 (deployment), QG-008 (release)

---

## PROJECT_CONFIG.yaml for this example

```yaml
project:
  name: "Backend API Service"
  version: "0.1.0"
  profile: "backend"
  framework: "FastAPI"
  languages: ["Python"]
  owner: "lead-engineer"
  goals: "Build production-grade REST API with auth, persistence, and observability."
  deadline: "2026-12-31"
  status: "INITIALIZED"

enabled_features:
  standards:
    - code_quality
    - testing
    - documentation
    - security
    - api_design
  agents:
    - orchestrator
    - chief-architect
    - planner
    - security-reviewer
    - ai-reviewer
    - qa-reviewer
    - docs-reviewer
    - release-reviewer
  quality_gates:
    - QG-001
    - QG-002
    - QG-003
    - QG-005
    - QG-006
    - QG-008
  metrics:
    - quality
    - performance
```
