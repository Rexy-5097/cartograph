# Example: Research Project

> **Profile:** `research` | **Standards:** research, documentation, testing, code_quality
>
> This example shows how to use AgentOS for scientific research and computational experiments.

---

## Project Summary

**Goal:** Conduct reproducible scientific experiments with validated statistical methods.
**Tech Stack:** Python + NumPy + Pandas + Jupyter
**Profile:** `research`

---

## Step 1: Bootstrap

```bash
python3 tools/scripts/bootstrap_project.py --profile research --defaults
# OR
make profile P=research
```

---

## Step 2: Initialize AI Assistant

Say: **"Initialize this repository using AgentOS."**

Research profile activates: `research`, `documentation`, `testing`, `code_quality` standards.
Agents enabled: `science-reviewer`, `docs-reviewer`, `qa-reviewer`.

---

## Step 3: Typical Task Flow

**Task:** "Design and run calibration validation experiment for sensor array."

```
Harness classifies: research + scientific domain
        │
Routes to: science-reviewer + docs-reviewer
        │
Loop (Balanced — 3 iterations):
  Iteration 1: science-reviewer flags: missing confidence intervals
  Iteration 2: docs-reviewer flags: methodology not documented
  Iteration 3: Both PASS
        │
Gates:
  QG-004 (research_validation) → PASS ✅  ← research-specific gate
  QG-001 (feature_completion) → PASS ✅
```

---

## Step 4: Expected Artifacts

```
artifacts/experiments/calibration-experiment-v1.md
artifacts/decisions/ADR-XXXX-statistical-method-selection.md
context/state.md  ← updated
```

---

## Key Research-Specific Rules

- Every experiment must be logged in `artifacts/experiments/`
- Statistical methods must be documented before implementation
- Reproducibility is mandatory: seed all random operations
- Science reviewer must PASS before any experiment is declared complete
