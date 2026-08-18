# ADR-0005: Vendor-Neutral Design with Isolated Integrations

> **Status:** Accepted | **Date:** 2026-07-02 | **Decider:** Principal Architect

---

## Context

AgentOS is designed to work with multiple AI coding agents: Claude Code, Antigravity, Gemini CLI, Codex, OpenHands, and future agents. If AgentOS was tightly coupled to any single vendor, it would become obsolete as tooling evolves.

## Decision

All content outside `integrations/` must be vendor-neutral.

Vendor-specific configuration (system prompts, tool configs, workflow adaptations) lives exclusively in `integrations/{vendor}/`.

## Structure

```
integrations/
  README.md          ← explains vendor isolation principle
  claude/            ← Claude-specific config
  gemini/            ← Gemini-specific config
  codex/             ← Codex-specific config
  {future-vendor}/   ← extensible
```

## Consequences

- Adding a new AI provider requires only adding a new subdirectory in `integrations/`
- No core AgentOS file needs to change when a new AI provider is added
- AgentOS workflows, standards, checklists, and agents remain usable regardless of which AI provider the project uses

## Trade-offs

Vendor-specific optimizations may be more complex to implement (they cannot reach into core files). This is an acceptable trade-off for long-term flexibility.
