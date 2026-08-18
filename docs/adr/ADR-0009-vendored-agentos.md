# ADR-0009 — AgentOS vendored under `agentos/` with minimal modification

**Status:** Accepted · 2026-08-18

## Context

Development is governed by AgentOS. Two installations exist on this machine:
`~/WorkFlow/agentos-template` (upstream v1.0.0, complete: profiles, CI
templates, certification/validation suites, published as
github.com/Rexy-5097/raptors-way) and the copy vendored into the Aaroh project
(v1.0.0 minus distribution files). The template is the authoritative source;
Aaroh's copy is itself a vendor of it.

## Decision

Vendor upstream v1.0.0 into `agentos/` rather than referencing it, so any
Cartograph commit reproduces the exact governance in force when it was made — a
checkpoint that cannot reproduce its own rules is not a checkpoint.

Modifications to upstream (kept deliberately minimal):

1. `tools/scripts/validate_agentos.py` — repository root was hardcoded to the
   author's absolute home path; now resolved relative to the script,
   overridable via `AGENTOS_ROOT`. Its DOCUMENTATION_INDEX link check was
   updated to match the rewritten relative links.
2. Absolute `file:///Users/...` links in 7 docs rewritten repository-relative.
3. `profiles/adityanet.yaml` removed (another project's profile, leaked into
   the template).
4. Not vendored: `.devcontainer/` (Docker is rejected for v1 local dev),
   `.pre-commit-config.yaml` (gating goes through make + CI), template `.git/`.

Cartograph-owned additions live beside the framework: `PROJECT_CONFIG.yaml`,
`context/*` content, `gates/`, `milestones/`, `artifacts/project-state.yaml`,
`tools/scripts/run_gates.py`.

## Alternatives

- **Git submodule** — rejected: submodules are routinely broken on clone and
  the framework needs project-local adaptation anyway.
- **Reimplement a lighter AgentOS** — rejected: explicitly prohibited
  (do not invent a second AgentOS).

## Consequences

Framework validator scores 99/100 (the two deliberate omissions cost
Distribution points) — an accurate report of a deliberate choice. Upstream
updates require a manual re-vendor with the four modifications reapplied.
