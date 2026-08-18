# ADR-0012: Quality Level Model

> **Status:** Accepted | **Date:** 2026-07-02 | **Decider:** Principal Architect

---

## Context

Projects have different needs. A 24-hour hackathon or exploratory ML research project cannot afford the operational overhead of safety-critical, production-ready SaaS infrastructure. Conversely, production SaaS requires rigorous gates that would kill early-stage research velocity.

## Problem

How do we design a single set of standards that scales from lightweight hackathons to flagship, high-reliability production systems without creating separate documents for each speed?

## Decision

Define four explicit **Quality Levels** in every engineering standard and metrics file:
1. **Minimum Acceptable:** Functional system, zero syntax or critical compile errors. For student projects/early experiments.
2. **Recommended:** Good engineering habits (basic testing, named constants, documented setup). For hackathons and internal tools.
3. **Production Grade:** Rigorous software engineering standards (80% unit coverage, OpenAPI specs, core web vitals green, threat model documented). Deployed systems.
4. **Flagship Grade:** Safety-critical, research-publication grade, or ISRO-scale rigor (90%+ test coverage, formal audits, zero-flakiness, property fuzzing).

## Consequences

- The project configuration (`.agentos/config.yml`) specifies the target quality level.
- Reviewer agents evaluate work strictly against the columns matching the target level.
- Teams can easily "level up" a project by changing the configuration target and resolving the gaps identified by the reviewers.
