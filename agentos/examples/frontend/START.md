# Example: Frontend Web Application

> **Profile:** `frontend` | **Standards:** ui_ux, code_quality, testing, security
>
> This example shows how to use AgentOS to build a production frontend web application.

---

## Project Summary

**Goal:** Build a modern web application with great UX, accessibility, and performance.
**Tech Stack:** TypeScript + React + Next.js + Tailwind CSS
**Profile:** `frontend`

---

## Step 1: Bootstrap

```bash
python3 tools/scripts/bootstrap_project.py --profile frontend --defaults
# OR
make profile P=frontend
```

This activates: `ui_ux`, `code_quality`, `testing`, `security`, `documentation` standards.
Reviewers enabled: `ui-reviewer`, `qa-reviewer`, `security-reviewer`, `docs-reviewer`.
Gates enabled: QG-001, QG-002, QG-003, QG-005, QG-008.

---

## Step 2: Initialize AI Assistant

Open your AI assistant and say: **"Initialize this repository using AgentOS."**

AI confirms: "AgentOS initialized. Frontend profile active. Standards: ui_ux, code_quality, testing, security."

---

## Step 3: Typical Task Flow

**Task:** "Build an accessible authentication form with email/password and Google OAuth."

```
Harness classifies: ui + security domain
        │
Routes to: ui-reviewer + security-reviewer
        │
Loop (Balanced — 3 iterations):
  Iteration 1:
    ui-reviewer: Missing aria-labels on inputs → add
    security-reviewer: CSRF protection needed → add
  Iteration 2:
    ui-reviewer: Color contrast ratio < 4.5:1 → fix
    security-reviewer: OAuth state parameter missing → add
  Iteration 3:
    ui-reviewer: PASS ✅
    security-reviewer: PASS ✅
        │
Quality Gates:
  QG-001 (feature_completion) → PASS ✅
  QG-002 (pull_request) → PASS ✅
  QG-003 (qa) → PASS ✅
  QG-005 (security_review) → PASS ✅
```

---

## Step 4: Frontend-Specific Standards Applied

| Standard | What It Checks |
|----------|---------------|
| `ui_ux` | Accessibility (ARIA, contrast, keyboard nav), usability, mobile responsiveness |
| `code_quality` | Component structure, TypeScript types, prop validation |
| `testing` | Component tests, E2E tests, visual regression |
| `security` | XSS prevention, CSRF, OAuth security, CSP headers |

---

## Step 5: Expected Artifacts

```
artifacts/
├── decisions/
│   └── ADR-XXXX-auth-ui-component-architecture.md
└── reviews/
    └── auth-form-ui-security-review.md

context/
└── state.md  ← updated: Auth form COMPLETE
```

---

## Step 6: Performance and Accessibility Checklist

Before QG-008 (release), run the ui-reviewer against:

```
□ Lighthouse score > 90 (Performance, Accessibility, Best Practices, SEO)
□ All interactive elements keyboard-navigable
□ Color contrast ratio ≥ 4.5:1 (WCAG AA)
□ Core Web Vitals: LCP < 2.5s, FID < 100ms, CLS < 0.1
□ Mobile responsiveness verified
□ No console errors in production build
```

---

## PROJECT_CONFIG.yaml for this example

```yaml
project:
  name: "Frontend Web Application"
  version: "0.1.0"
  profile: "frontend"
  framework: "Next.js + React"
  languages:
    - "TypeScript"
    - "CSS"
  owner: "lead-engineer"
  goals: "Build accessible, performant production web application."
  deadline: "2026-12-31"
  status: "INITIALIZED"

enabled_features:
  standards:
    - code_quality
    - testing
    - documentation
    - security
    - ui_ux
  agents:
    - orchestrator
    - chief-architect
    - planner
    - ui-reviewer
    - security-reviewer
    - qa-reviewer
    - docs-reviewer
  quality_gates:
    - QG-001
    - QG-002
    - QG-003
    - QG-005
    - QG-008
  metrics:
    - quality
    - performance
```
