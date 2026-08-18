# AgentOS v1.0.0 Release Notes

Welcome to the official stable release of **AgentOS v1.0.0**! This framework coordinates reviews, checks, standards, and metrics under a common quality runtime.

---

## 1. Release Highlights

- **Shared Runtime Kernel:** Consolidates common scheduler, state manager, policy parser, and logger modules to isolate policies from orchestration.
- **Harness Planning & Routing:** Routes developer tasks programmatically based on active filename pattern overrides.
- **Loop Engine Iterations:** Evaluates developer task outputs, analyzes trend deltas, categorizes root cause failures, and terminates early when improvements saturate.
- **Bootstrapping Layer:** Installs and bootstraps project configurations in under 5 seconds.
- **Executable Validation Suite:** Sweeps 21 validation scenarios to verify subsystem integrations.
- **Distribution Readiness:** Documented with team quickstarts, compatibility upgrades, and golden path examples.

---

## 2. Technical Scope & Architecture

- **Specialist Agents:** 14 reviewer configurations.
- **Subsystems Covered:** 15 active modules.
- **Maturity Scorecard:** 100/100 readiness.

---

## 3. Known Limitations

- Overriding patterns are basename matching rules. Subdirectory prefix logic defaults to filename scopes.
- Context file budgets are set to a max constraint of 4000 tokens.

---

## 4. Upgrade & Migration

Upgrade paths are documented under [COMPATIBILITY.md](COMPATIBILITY.md). Core principles are locked under [ENGINEERING_PRINCIPLES.md](ENGINEERING_PRINCIPLES.md).
