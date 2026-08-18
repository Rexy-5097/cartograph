# Example: Flagship Production Project

> **Profile:** `flagship` | **Standards:** ALL 9 standards active | **Agents:** ALL 11 agents active
>
> This example shows how to use AgentOS for a production-scale flagship engineering project.
> All quality gates are mandatory. No exceptions.

---

## Project Summary

**Goal:** Build a production-scale multi-service engineering platform with full observability.
**Tech Stack:** Python + TypeScript + FastAPI + Next.js + PostgreSQL + Redis + Kafka
**Profile:** `flagship`

---

## Step 1: Bootstrap

```bash
python3 tools/scripts/bootstrap_project.py --profile flagship --defaults
# OR
make profile P=flagship
```

This activates:
- All 9 engineering standards
- All 11 specialist agents
- All 8 quality gates (QG-001 through QG-008)
- All 3 metrics modules

---

## Step 2: Initialize AI Assistant

Say: **"Initialize this repository using AgentOS."**

AI responds: "AgentOS initialized. Flagship profile active. All standards, agents, and gates enabled."

---

## Step 3: Full Execution Flow

Flagship projects use the complete AgentOS lifecycle:

```
Feature Request
        │
        ▼
Planner → defines scope + milestones
        │
        ▼
Harness → classifies all domains affected
        │
        ├──→ ai-reviewer (if AI components)
        ├──→ security-reviewer (always)
        ├──→ performance-reviewer (if critical path)
        ├──→ ui-reviewer (if frontend)
        └──→ qa-reviewer (always)
        │
        ▼
Loop (Exhaustive mode for flagship)
        │
        ▼
Quality Gates (ALL 8 must pass):
  QG-001 Feature Completion ✅
  QG-002 Pull Request ✅
  QG-003 QA ✅
  QG-004 Research Validation ✅ (if AI/research components)
  QG-005 Security Review ✅
  QG-006 Deployment ✅
  QG-007 Bug Fix ✅ (for any bugs discovered)
  QG-008 Release ✅
        │
        ▼
Artifacts:
  ADR → decisions/
  Release Notes → releases/
  Architecture Review → reviews/
        │
        ▼
Validator: 100/100 required
```

---

## Expected Artifacts at Flagship Scale

```
artifacts/
├── decisions/        ← 50+ ADRs expected
├── experiments/      ← Research logs
├── incidents/        ← Post-mortems
├── releases/         ← Release history
└── reviews/          ← Architecture reviews

production_certification/
├── SYSTEM_CERTIFICATION.md
├── PERFORMANCE_REPORT.md
└── TEAM_READINESS.md
```

---

## Flagship-Specific Rules

1. **Architecture freeze protocol:** Any architectural change requires a Chief Architect ADR
2. **No untested code:** QG-003 (QA gate) is mandatory before every merge
3. **Security by default:** QG-005 runs on every feature that touches data or APIs
4. **Deployment gate:** QG-006 must PASS before any production deployment
5. **Post-mortem required:** Every production incident generates an artifact in `artifacts/incidents/`
