## Milestone / scope

<!-- Which milestone (or fix/adr) is this? Link the definition in agentos/milestones/. -->

## What this delivers

<!-- Intent, not mechanics. -->

## Checklist

- [ ] Scope matches the active milestone — nothing from a future milestone smuggled in
- [ ] `make check` passes locally (fmt, clippy -D warnings, tests, AgentOS validator)
- [ ] `make gates` passes (QG-001…009)
- [ ] New dependencies: each one answers "what concrete problem does this solve **now**?" below
- [ ] No secrets, tokens, or machine-specific paths in the diff
- [ ] CHANGELOG.md updated; CHECKPOINTS.md and project-state.yaml reflect reality
- [ ] Docs describe implemented behavior only
- [ ] Architecture changes have an ADR (or none were made)
- [ ] Tests added/updated for every behavior change

## New dependencies (if any)

<!-- crate — concrete problem it solves right now -->

## Evidence

<!-- Paste the tail of `make check` / `make gates` output. -->
