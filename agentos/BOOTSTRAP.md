# Bootstrap Reference Guide

> **Purpose:** Complete reference for bootstrapping AgentOS in a new project.
>
> This document is for AI assistants and human developers. Read `AGENTOS.md` first.

---

## What Bootstrap Does

Running `bootstrap_project.py` (or `make bootstrap`) performs 7 steps:

1. **Verifies** all required repository directories exist
2. **Loads** the selected project profile (`profiles/{name}.yaml`)
3. **Generates** `PROJECT_CONFIG.yaml` with enabled features
4. **Creates** `context/vision.md` if missing
5. **Creates** `context/state.md` if missing
6. **Runs** `validate_agentos.py`
7. **Reports** initialization status

---

## Bootstrap Modes

### Interactive (Recommended for first-time setup)

```bash
python3 tools/scripts/bootstrap_project.py --interactive
# OR
make bootstrap
```

Prompts for: Project Name, Profile, Goals, Framework, Languages, Deadline.

### Defaults (Non-interactive, fast)

```bash
python3 tools/scripts/bootstrap_project.py --defaults
# OR
make bootstrap-quick
```

Uses `ai_project` profile with sensible defaults. Good for CI or quick starts.

### Profile Override

```bash
python3 tools/scripts/bootstrap_project.py --profile backend --defaults
# OR
make profile P=backend
```

Available profiles: `ai_project` | `backend` | `frontend` | `ml` | `research` | `isro` | `hackathon` | `flagship`

### Config File

```bash
python3 tools/scripts/bootstrap_project.py --config my_project.yaml
```

Supply a YAML file with `project:` block matching `PROJECT_CONFIG.example.yaml`.

### Resume Interrupted Setup

```bash
python3 tools/scripts/bootstrap_project.py --resume
```

Re-runs setup without overwriting existing `context/` files.

### Self-Test

```bash
python3 tools/scripts/bootstrap_project.py --self-test
# OR
make self-test
```

Verifies all 8 profile YAML files exist and bootstrap runs cleanly. Use this to confirm a fresh clone is healthy.

---

## Generated Files

| File | Description |
|------|-------------|
| `PROJECT_CONFIG.yaml` | Active project configuration and feature flags |
| `context/vision.md` | Project goals and success criteria template |
| `context/state.md` | Live project status (updated every session) |

---

## Profile Summary

| Profile | Standards | Agents | Gates |
|---------|-----------|--------|-------|
| `ai_project` | ai_ml, security, code_quality | orchestrator, ai-reviewer, security-reviewer | QG-001,002,004,005 |
| `backend` | security, api_design, testing, code_quality | orchestrator, security-reviewer, performance-reviewer | QG-001,002,003,005,006,008 |
| `frontend` | ui_ux, security, testing, code_quality | orchestrator, ui-reviewer, security-reviewer | QG-001,002,003,005,008 |
| `ml` | ai_ml, research, data_engineering, security | orchestrator, ai-reviewer, science-reviewer | QG-001,002,003,004,005,008 |
| `research` | research, documentation, testing | orchestrator, science-reviewer, docs-reviewer | QG-001,002,004 |
| `isro` | security, research, documentation | orchestrator, security-reviewer, science-reviewer | QG-001,002,004,005,006,008 |
| `hackathon` | code_quality, documentation | orchestrator, docs-reviewer | QG-001,002 |
| `flagship` | All 9 | All 11 | All 8 (QG-001 to QG-008) |

---

## Context Loading Policy

AgentOS uses minimal context loading to preserve token budgets.

| Load Priority | Files | Condition |
|--------------|-------|-----------|
| **Always** | `AGENTOS.md`, `context/state.md`, `PROJECT_CONFIG.yaml` | Every session |
| **On demand** | `context/vision.md`, `context/architecture.md`, `context/decisions.md` | When relevant |
| **Per task** | `agents/{agent}.md`, `standards/{domain}.md`, `checklists/{gate}.md` | When routing |
| **Never eagerly** | `artifacts/decisions/`, `runtime/` internals, all scenarios | Too expensive |

---

## Verification Rules

Bootstrap is considered complete when:

1. `PROJECT_CONFIG.yaml` exists and is valid YAML
2. `context/vision.md` and `context/state.md` exist
3. `validate_agentos.py` reports `100/100 PASS`

If any of these fail, do not proceed to development.

---

## Troubleshooting

| Problem | Fix |
|---------|-----|
| Missing profile YAML | Run `make self-test` — it will identify which profiles are missing |
| `pyyaml` not found | `pip install pyyaml` |
| Validator fails after bootstrap | Read warning output; usually a missing required file |
| Interactive mode hangs | Press Ctrl+C and use `--defaults` instead |

---

## See Also

- [AGENTOS.md](./AGENTOS.md) — Full AI initialization protocol
- [PROJECT_CONFIG.example.yaml](./PROJECT_CONFIG.example.yaml) — Annotated config example
- [ARCHITECTURE.md](./ARCHITECTURE.md) — Bootstrap Flow diagram (Section 8)
- [TEAM_QUICKSTART.md](./TEAM_QUICKSTART.md) — Zero-knowledge 5-minute guide
