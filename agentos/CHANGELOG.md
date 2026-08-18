# CHANGELOG

All notable changes to this template are documented here.

Format follows [Keep a Changelog](https://keepachangelog.com/en/1.0.0/).
Versioning follows [Semantic Versioning](https://semver.org/spec/v2.0.0.html) per `VERSION_POLICY.md`.

---

## [Unreleased]

### In Progress
- Post-v1 Roadmap enhancements

---

## [1.0.0-final] — 2026-07-02 — Final Phase: AI-First Distribution & Repository Packaging

### Added
- `AGENTOS.md` — Complete rewrite as comprehensive AI initialization protocol (14 sections, all required protocol areas)
- `PROJECT_CONFIG.example.yaml` — Annotated example configuration with all available options
- `CODE_OF_CONDUCT.md` — Contributor Covenant community standards
- `SECURITY.md` — Security policy with vulnerability reporting and design principles
- `SUPPORTED_VERSIONS.md` — Version support matrix with stability guarantees
- `.editorconfig` — Consistent formatting across all editors
- `.pre-commit-config.yaml` — Pre-commit hooks (Python, YAML, Markdown, whitespace)
- `Makefile` — 8 developer shortcuts (bootstrap, validate, health, test, examples, self-test, profile, clean)
- `.devcontainer/devcontainer.json` — VS Code / Cursor dev container with extensions and postCreateCommand
- `.github/ISSUE_TEMPLATE/bug_report.md` — Bug report template
- `.github/ISSUE_TEMPLATE/feature_request.md` — Feature request template
- `.github/ISSUE_TEMPLATE/question.md` — Question template
- `.github/PULL_REQUEST_TEMPLATE.md` — Pull request template with AgentOS checklist
- `.github/workflows/validate.yml` — CI: validator + synthetic suite + bootstrap self-test
- `.github/workflows/lint.yml` — CI: Python (ruff), YAML, Markdown, internal link checker
- `.github/workflows/release.yml` — CI: automated GitHub release on version tags
- `.github/CODEOWNERS` — Code ownership for critical files
- `.github/LABELS.md` — Repository label definitions
- `profiles/frontend.yaml` — Frontend/web app project profile
- `profiles/ml.yaml` — Machine learning / MLOps project profile
- `profiles/flagship.yaml` — Production flagship profile (all 9 standards, all 11 agents, all 8 gates)
- `examples/backend/START.md` + `PROJECT_CONFIG.yaml` — Complete backend example with JWT auth walkthrough
- `examples/ai_project/START.md` — AI/ML product example
- `examples/research/START.md` — Scientific research example
- `examples/flagship/START.md` — Production flagship example
- `integrations/claude/README.md` — Claude Code integration guide
- `integrations/gemini/README.md` — Google Gemini integration guide
- `integrations/codex/README.md` — OpenAI Codex/Copilot integration guide
- `integrations/antigravity/README.md` — Google Antigravity integration guide (native)
- `production_certification/DISTRIBUTION_READINESS_REPORT.md` — Full distribution readiness audit
- `production_certification/AI_COMPATIBILITY_REPORT.md` — AI vendor compatibility matrix

### Modified
- `README.md` — Full improvement: added Why AgentOS Exists, architecture flow diagram, end-to-end JWT example, all missing sections, v1.0.0 versioning
- `DOCUMENTATION_INDEX.md` — Synchronized with all 50+ repository documents across 13 categories
- `tools/scripts/bootstrap_project.py` — Added `--profile` flag, expanded self-test to verify all 8 profiles, updated interactive prompt
- `tools/scripts/validate_agentos.py` — Distribution validator expanded to check GitHub templates, developer tooling, community files, all 8 profiles, integration READMEs, and stale version references

### Validator
- All 18 categories: 100/100 | 0 warnings | Distribution: PASS
- Synthetic suite: 21/21 PASS
- Bootstrap self-test: All 8 profiles verified — PASS

---

## [1.0.0] — 2026-07-02 — Phase 13 Production Certification & v1.0.0 Final Release

### Added
- Completed system-wide audit of all 15 subsystems under `production_certification/SYSTEM_CERTIFICATION.md`.
- Created `production_certification/STABILITY_REPORT.md` compiling code metrics, file count checks, and ADR summaries.
- Created `production_certification/PERFORMANCE_REPORT.md` documenting Harness and Loop telemetry benchmark runs.
- Created `production_certification/TEAM_READINESS.md` detailing new teammate developer onboarding ramp-up scores.
- Created `production_certification/RELEASE_CHECKLIST.md` verifying release compliance.
- Created `production_certification/FUTURE_ROADMAP.md` listing post-v1 upgrades (v1.1, v1.2, v2.0).
- Created `production_certification/MIGRATION_REPORT.md` certifying portability on fresh clones.
- Created `production_certification/REPOSITORY_STATISTICS.md` dashboard consolidating structural counts.
- Created `COMPATIBILITY.md` documenting upgrade paths and deprecation rules.
- Created `examples/` subdirectories housing START.md golden path onboarding guides for Backend, AI, Research, and Flagship projects.
- Upgraded validator script `validate_agentos.py` to audit v1.0.0 stable deliverables and output the **v1.0.0 Production Certification Report**.

---

## [1.0.0-rc1] — 2026-07-02 — Phase 12 Release Candidate Validation

### Added
- Configured a strict repository architecture freeze (no new agents or runtime engines).
- Created `production_validation/` directory structure containing benchmarks configurations, README, and assessment files.
- Published `production_validation/productivity_metrics.csv` defining developer adoption metrics (boot times, repair speeds, loop counts).
- Created `production_validation/rc1_validation_report.md` logging run telemetry for Backend (Project A) and Research (Project B) controlled benchmarks.
- Published `production_validation/readiness_assessment.md` detailing RC1 readiness checks.
- Upgraded static validator `validate_agentos.py` to assert `1.0.0-rc1` version compliance and output the **Production RC1 Health Report**.
- Logged 5 new validation ADRs (ADR-0052 through ADR-0056) and indexed them.

---

## [0.11.0] — 2026-07-02 — Phase 11 Synthetic Validation Suite

### Added
- Created the executable Synthetic Validation Suite under `validation/` comprising 20 isolated scenario directories (`VS-001` through `VS-020`) containing `scenario.md`, request payloads (`input/request.json`), and assertion scorecards (`assertions.yaml`).
- Implemented the test suite orchestrator `validation/runner/execute_suite.py`, scenario run parser `execute_scenario.py`, and dashboard markdown report generator `report_generator.py`.
- Mapped target subsystem tracing in `validation/manifest.yaml` to ensure complete coverage audit runs.
- Upgraded the static validator `validate_agentos.py` to check validation manifest, runners, scenario folders, and output the **Validation Suite Health Report**.
- Logged 5 new ADRs (ADR-0052 through ADR-0056) under `artifacts/decisions/` and indexed them.

---

## [0.10.0] — 2026-07-02 — Phase 10 Loop Engine

### Added
- Created the shared Runtime Kernel under `runtime/kernel/` containing the core platform tools (`scheduler.py`, `state_manager.py`, `event_bus.py`, `policy_loader.py`, `logger.py`, `health.py`).
- Implemented the modular Loop Engine kernel under `runtime/loop/` separating tracking (`iteration_controller.py`), diagnostic runs (`reflection.py`), improvement planning (`improvement_planner.py`), safety blocks (`execution_monitor.py`), quality scoring (`quality_evaluator.py`), stopping checks (`termination.py`), state transition (`state_machine.py`), and execution runs (`runtime.py`).
- Configured YAML policy configurations under `runtime/policies/` (`loop.yaml`, `reflection.yaml`, `termination.yaml`, `retry_limits.yaml`, `quality_thresholds.yaml`).
- Integrated Harness CLI `tools/scripts/harness_engine.py` to trigger loop execution after the planning phase, printing the combined **Loop Execution Report**.
- Upgraded the static validator `validate_agentos.py` to check loop kernel files, policies, safety triggers, and output the **Loop Health Report**.
- Logged 7 new ADRs (ADR-0045 through ADR-0051) under `artifacts/decisions/` and indexed them.

---

## [0.9.0] — 2026-07-02 — Phase 9 Harness Engine

### Added
- Implemented the modular Harness Engine kernel under `runtime/harness/` separating classification (`classifier.py`), context caching (`context_optimizer.py`), routing (`agent_router.py`, `tool_router.py`), planner (`execution_planner.py`), budgets (`cost_optimizer.py`), retries (`failure_recovery.py`), and state tracking (`state_machine.py`, `runtime.py`).
- Configured policies in YAML files under `runtime/policies/` (`routing.yaml`, `context.yaml`, `quality.yaml`, `tools.yaml`, `retry.yaml`).
- Created `tools/scripts/harness_engine.py` as the CLI interface wrapper to ingest Request objects, invoke the kernel, monitor runs, and print Harness reports.
- Published `tools/scripts/harness_specification.md` specifying Harness contracts, Request schema, and Plan schema.
- Upgraded the static validator `validate_agentos.py` to check policy files, duplicate routes, SM states, and output the **Harness Health Report**.
- Logged 7 new ADRs (ADR-0038 through ADR-0044) in `artifacts/decisions/` and indexed them.

---

## [0.8.0] — 2026-07-02 — Phase 8 Bootstrap & One-Prompt Initialization

### Added
- Created `BOOTSTRAP.md` root guide explaining the repository sequence and selective loading policies.
- Published `START_PROJECT.md` specifying AI entry interfaces, supported assistants, and expected behavior.
- Added 4 vendor integration profiles under `integrations/` (`claude/bootstrap.md`, `antigravity/bootstrap.md`, `gemini/bootstrap.md`, `codex/bootstrap.md`).
- Added 5 configuration profiles under `profiles/` (`ai_project.yaml`, `backend.yaml`, `research.yaml`, `isro.yaml`, `hackathon.yaml`).
- Implemented the interactive CLI setup tool `tools/scripts/bootstrap_project.py` supporting CLI flags (`--interactive`, `--config`, `--defaults`, `--self-test`, `--resume`) and merge recovery.
- Upgraded the static validator `validate_agentos.py` to parse `PROJECT_CONFIG.yaml` and output the **Bootstrap Health Report**.
- Logged 5 new ADRs (ADR-0033 through ADR-0037) in `artifacts/decisions/` and indexed them.

---

## [0.7.0] — 2026-07-02 — Phase 7 Artifact System

### Added
- Created the master schema definition `templates/SCHEMA.md` specifying metadata keys and validation rules.
- Converted all templates under `templates/` into machine-verifiable contracts.
- Added 3 new artifact categories: Sprint Plan (`sprint_plan.md`), Risk Register (`risk_register.md`), and Incident Report (`incident_report.md`).
- Upgraded the static validator `tools/scripts/validate_agentos.py` to parse document frontmatter, audit mandatory keys, verify trace links, and output the **Artifact Health Report**.
- Logged 5 new ADRs (ADR-0028 through ADR-0032) in `artifacts/decisions/` and indexed them.

---

## [0.6.0] — 2026-07-02 — Phase 6 Quality Gate System

### Added
- Completed all 8 Quality Gate checklists under `checklists/` (`feature_completion.md`, `pull_request.md`, `qa.md`, `research_validation.md`, `security_review.md`, `deployment.md`, `bug_fix.md`, `release.md`).
- Assigned unique IDs (`QG-001` through `QG-008`) to all checklists.
- Added metadata parameters (version, runtime, severity, automation level, retry policy) to all checklist files.
- Published a full Traceability Matrix mapping Gates to Standards, Metrics, Reviewers, and ADRs in `checklists/README.md`.
- Implemented the Pass/Warning/Fail decision model with evidence-based criteria.
- Added post-release Monitoring to the development lifecycle.
- Logged 5 new ADRs (ADR-0023 through ADR-0027) in `artifacts/decisions/` and indexed them.

---

## [0.5.0] — 2026-07-02 — Phase 5 Validation Suite

### Added
- Created the static validator `tools/scripts/validate_agentos.py` checking structural formats, links, budgets, standard/metrics consumers, and circular dependencies.
- Programmed a detailed health report showing file counts, broken links, circular routes, token budgets, and scoring grades.
- Built a dry-run runtime simulator `tools/scripts/simulate_agent_runtime.py` verifying triggers against `agents/TESTS.md` routing.

---

## [0.4.0] — 2026-07-02 — Phase 4 Agent Runtime

### Added
- Populated all 11 agent contracts under `agents/` with explicit input/output schemas and lifecycle state transitions.
- Enforced context budgets (< 4000) and de-escalation hierarchies.
- Published `agents/CAPABILITIES.md` (orchestrator routing matrix) and `agents/TESTS.md` (routing test fixtures).
- Logged 7 new ADRs (ADR-0016 through ADR-0022).

---

## [0.3.0] — 2026-07-02 — Phase 3 Engineering Constitution

### Added
- Completed all 9 engineering standards in `standards/` (`code_quality.md`, `testing.md`, `documentation.md`, `security.md`, `ai_ml.md`, `research.md`, `api_design.md`, `data_engineering.md`, `ui_ux.md`).
- Completed all 3 metric files in `metrics/` (`performance.md`, `quality.md`, `benchmarks.md`).
- Established four-tier dependency standards hierarchy (Foundation -> Cross-cutting -> Domain -> Component).
- Implemented four-level quality model (Minimum Acceptable, Recommended, Production Grade, Flagship Grade).
- Implemented binary Reviewer Question Framework (10 YES/NO questions) for reviewer agents.
- Documented 5 new ADRs (ADR-0011 through ADR-0015) in `artifacts/decisions/` covering standards hierarchy, quality levels, metrics philosophy, reviewer questions, and cross-reference inheritance.

---

## [0.2.0] — 2026-07-02 — Phase 2 Project Intelligence Layer

### Added
- Completed all 7 context files in `context/` (`vision.md`, `state.md`, `memory.md`, `architecture.md`, `tech_stack.md`, `workflow.md`, `decisions.md`).
- Completed `workflows/master.md` as the main operational brain of AgentOS.
- Documented 5 new ADRs (ADR-0006 through ADR-0010) covering context design, token compression, master workflow design, state heartbeat, and workflow-context separation.
- Established strict size budgets and selective loading protocols for agent context windows.

---

## [0.1.0] — 2026-07-02 — Phase 1 Foundation

### Added

**Repository foundation:**
- `README.md` — Full repository overview with architecture diagram, all sections
- `ENGINEERING_PRINCIPLES.md` — 13 governing principles; philosophical foundation of the entire system
- `VERSION_POLICY.md` — Semantic versioning contract (MAJOR/MINOR/PATCH definitions)
- `CONTRIBUTING.md` — Contribution guidelines, standards, and PR process
- `INSTALL.md` — Setup guide (placeholder, completed in Phase 7)
- `CHANGELOG.md` — This file
- `LICENSE` — MIT License
- `VERSION` — Semantic version file
- `.gitignore` — Comprehensive patterns for Python, Node, ML/AI, OS, secrets, infrastructure

**Runtime configuration:**
- `.agentos/config.yml` — WAT config, agent roster, trigger map, artifact naming
- `.agentos/manifest.yml` — Complete file inventory and ownership map

**Context layer (project-owned placeholders):**
- `context/vision.md`, `context/workflow.md`, `context/state.md`
- `context/architecture.md`, `context/tech_stack.md`
- `context/decisions.md` — Active ADR index (populated with Phase 1 decisions)
- `context/memory.md`

**Workflow layer (infrastructure placeholders):**
- `workflows/master.md`, `workflows/feature_development.md`, `workflows/bug_fix.md`
- `workflows/research.md`, `workflows/release.md`, `workflows/incident_response.md`

**Agent layer (infrastructure placeholders):**
- `agents/orchestrator.md` — NEW: routing and coordination layer
- `agents/chief-architect.md`, `agents/planner.md`
- `agents/ai-reviewer.md`, `agents/science-reviewer.md`, `agents/security-reviewer.md`
- `agents/performance-reviewer.md`, `agents/ui-reviewer.md`
- `agents/qa-reviewer.md`, `agents/docs-reviewer.md`, `agents/release-reviewer.md`

**Tools layer:**
- `tools/mcp/template.yml` — MCP integration template
- `tools/mcp/README.md`, `tools/scripts/README.md`

**Standards (infrastructure placeholders):**
- `standards/code_quality.md`, `standards/testing.md`, `standards/documentation.md`
- `standards/security.md`, `standards/ai_ml.md`, `standards/research.md`
- `standards/api_design.md`, `standards/data_engineering.md`, `standards/ui_ux.md`

**Metrics layer (new):**
- `metrics/README.md`, `metrics/performance.md`, `metrics/quality.md`, `metrics/benchmarks.md`

**Checklists (infrastructure placeholders):**
- `checklists/feature_completion.md`, `checklists/pull_request.md`, `checklists/qa.md`
- `checklists/research_validation.md`, `checklists/release.md`, `checklists/deployment.md`
- `checklists/bug_fix.md`, `checklists/security_review.md`

**Templates (infrastructure placeholders):**
- `templates/experiment_log.md`, `templates/decision_record.md`, `templates/architecture_review.md`
- `templates/bug_report.md`, `templates/feature_proposal.md`, `templates/meeting_notes.md`
- `templates/release_notes.md`

**Artifacts directory:**
- `artifacts/decisions/` — with 5 initial ADRs (ADR-0001 through ADR-0005)
- `artifacts/experiments/`, `artifacts/incidents/`, `artifacts/releases/`

**Lessons layer (new):**
- `lessons/README.md`

**Integrations layer (new):**
- `integrations/README.md`, `integrations/claude/`, `integrations/gemini/`, `integrations/codex/`

### Architectural Decisions (see artifacts/decisions/)

| ADR | Decision |
|-----|---------|
| ADR-0001 | Renamed `knowledge/` → `context/` for precise semantics |
| ADR-0002 | Renamed `records/` → `artifacts/` for clarity |
| ADR-0003 | Added `orchestrator` agent — separates routing from decision authority |
| ADR-0004 | Kebab-case agent naming with role-appropriate suffixes |
| ADR-0005 | Vendor-neutral design — all vendor config isolated in `integrations/` |

### Design Principles Established

- WAT (Workflows → Agents → Tools) as the core organizing philosophy
- No implementation agent — main coding session implements
- Explicit agent invocation — never automatic
- Metrics layer — measurable targets instead of subjective reviews
- Lessons layer — institutional memory that grows with the project
- Vendor isolation — `integrations/` boundary separates AgentOS from AI providers
- ADR-first architecture — every significant decision documented

---

*AgentOS Template Changelog*
