# ADR-0014: Reviewer Question Framework

> **Status:** Accepted | **Date:** 2026-07-02 | **Decider:** Principal Architect

---

## Context

Reviewer agents need to assess code changes and provide quick, unambiguous feedback. If a standard is a wall of narrative text, the agent must reason heavily to formulate its critique, leading to high token usage, slow turnaround, and inconsistent reviews.

## Problem

How do we format the criteria in our standards so that reviewer agents can evaluate them with minimal reasoning and maximum alignment?

## Decision

Embed a structured **Reviewer Question Framework** in every standard:
1. Provide a checklist of 10 binary (YES/NO) questions at the end of each standard.
2. The questions must be direct, specific, and directly map to the standard's best practices.
3. Reviewer agents must run this checklist explicitly in their output.
4. If a question is answered "NO," the agent must link it directly to a findings item (CRITICAL, MAJOR, or MINOR).

## Consequences

- Reviewer agents produce structured, highly consistent reviews.
- Token overhead for parsing rules is minimized because the questions act as pre-processed check-gates.
- Humans can audit the agent's work by simply verifying the checkbox answers.
