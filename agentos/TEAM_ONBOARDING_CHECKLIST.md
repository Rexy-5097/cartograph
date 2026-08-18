# Team Onboarding & Adoption Checklist

Use this checklist to track your onboarding progress into AgentOS.

---

## 1. Developer Setup
- [ ] **Repository Cloned:** Copy the stable AgentOS v1.0.0 layout into your local environment.
- [ ] **Bootstrap Executed:** Initialize configuration parameters using:
  ```bash
  python3 tools/scripts/bootstrap_project.py --defaults
  ```
- [ ] **Validator Executed:** Ensure the initial environment passes compliance validation check:
  ```bash
  python3 tools/scripts/validate_agentos.py
  ```
- [ ] **Verification Suite Run:** Execute the synthetic validation scenarios sweeps:
  ```bash
  python3 validation/runner/execute_suite.py
  ```

---

## 2. Platform Adoption
- [ ] **Read Onboarding Guides:** Review [TEAM_QUICKSTART.md](TEAM_QUICKSTART.md) and [AGENTOS.md](AGENTOS.md).
- [ ] **Select a Project Profile:** Customize `PROJECT_CONFIG.yaml` to match backend, research, or flagship scopes.
- [ ] **Understand Quality Gates:** Review checklist files under `checklists/` to see target exit conditions.
- [ ] **Run First Refinement Loop:** Build a feature task using `harness_engine.py` and inspect loop logs.
