# ADR-0005 — Local-first privacy model

**Status:** Accepted · 2026-08-18 · Enforces RULES 011–015

## Context

Cartograph is pointed at private source, including employers' monorepos. Trust
is a precondition for adoption, and privacy retrofits are never believed.

## Decision

From v1, as invariants: no source leaves the machine (default and
unconditional); AI opt-in per repository, off by default, product fully useful
with zero keys and no network; credentials in the OS keychain only; redaction
at the tracing layer — no source contents, tokens or environment values in
logs; telemetry opt-in only; minimum GitHub scopes; MCP exposes only the
session-authorized repository; temporary files cleaned on exit.

Enforced structurally where possible: `SourceLocation` rejects absolute paths
at construction; `EnvVar` nodes have no value field; QG-005 scans every PR.

## Alternatives

- **Cloud analysis for onboarding ease** — rejected: destroys the one property
  that lets the tool near private code, and adds hosting cost before revenue.

## Consequences

No server-side features in v1 (shared graphs wait for a team tier). Analysis
performance must come from the local machine, which the Rust core makes
feasible.
