# Backward Compatibility & Upgrade Specification

This document codifies the compatibility rules and upgrade policies of AgentOS after its stable **v1.0.0** release.

---

## 1. Compatibility Policy

- **Minor Version (v1.x):** Non-breaking additions are allowed. Core policies and checklist rules remain backward compatible.
- **Major Version (v2.0):** Breaking changes to directory layouts, standard keys, or runtime state transitions are permitted.

---

## 2. Upgrade Path

To upgrade an existing project's AgentOS modules from `1.x` to `1.y`:
1. Keep the custom `PROJECT_CONFIG.yaml` config intact.
2. Back up `context/state.md` and custom checklists.
3. Overwrite the `runtime/` and `tools/scripts/` folders.
4. Run `python3 tools/scripts/validate_agentos.py` to confirm compatibility setup.

---

## 3. Deprecated Components

No components are deprecated as of v1.0.0. All active layers are certification targets.
