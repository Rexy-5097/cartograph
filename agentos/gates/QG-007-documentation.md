# QG-007 — Documentation & change tracking

- CHANGELOG has an entry for the change.
- CHECKPOINTS.md and `artifacts/project-state.yaml` reflect reality — the state
  file may not claim PASS for a gate that did not run (Part 16 rule).
- Docs describe implemented behavior only; future behavior is explicitly
  labelled with its milestone.
- New/changed architecture has its ADR, indexed in `context/decisions.md`.
