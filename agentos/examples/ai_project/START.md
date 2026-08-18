# Example: AI/ML Project

> **Profile:** `ai_project` | **Standards:** ai_ml, security, code_quality, testing
>
> This example shows how to use AgentOS to build an AI-powered product or ML pipeline.

---

## Project Summary

**Goal:** Build an AI-powered document analysis system with LLM integration.
**Tech Stack:** Python + LangChain + Gemini API + FastAPI
**Profile:** `ai_project`

---

## Step 1: Bootstrap

```bash
python3 tools/scripts/bootstrap_project.py --profile ai_project --defaults
# OR
make profile P=ai_project
```

---

## Step 2: Initialize AI Assistant

Open your AI assistant and say: **"Initialize this repository using AgentOS."**

The AI reads `AGENTOS.md` and activates the `ai_project` profile.

---

## Step 3: Typical Task Flow

**Task:** "Build a document summarization pipeline using Gemini."

```
Harness classifies: ai_ml + security domain
        │
Routes to: ai-reviewer + security-reviewer
        │
Loop (Balanced — 3 iterations):
  Iteration 1: ai-reviewer flags hallucination risk — add validation
  Iteration 2: security-reviewer flags PII exposure — add redaction
  Iteration 3: Both PASS
        │
Gates:
  QG-001 (feature_completion) → PASS ✅
  QG-004 (research_validation) → PASS ✅  ← unique to ai/research profiles
  QG-005 (security_review) → PASS ✅
```

---

## Step 4: Expected Artifacts

```
artifacts/decisions/ADR-XXXX-llm-provider-selection.md
artifacts/experiments/doc-summarization-eval-v1.md
context/state.md  ← updated
```

---

## PROJECT_CONFIG.yaml for this example

```yaml
project:
  name: "AI Document Analysis System"
  version: "0.1.0"
  profile: "ai_project"
  framework: "LangChain + FastAPI"
  languages: ["Python"]
  owner: "lead-engineer"
  goals: "Build production-grade LLM document analysis pipeline."
  deadline: "2026-12-31"
  status: "INITIALIZED"
```
