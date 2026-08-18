# ADR-0002 — Two-tier analysis: tree-sitter + LSP

**Status:** Accepted · 2026-08-18

## Context

Cartograph needs both syntactic facts (what does this file say?) and semantic
facts (what does this name refer to?). Writing type checkers for TypeScript and
Python is out of scope for any small team.

## Decision

Tier 1: tree-sitter parses every file — incremental, error-tolerant, wide
grammar coverage; a file mid-edit still yields facts. Tier 2: language servers
(tsserver for TypeScript, Pyright for Python) via async-lsp answer symbol
resolution queries exactly. The parser crate owns tier 1; the resolver crate
owns tier 2.

## Alternatives

- **Compiler APIs directly** (tsc API, Python `ast` + typeshed) — rejected:
  per-language runtimes in the core (Node, Python), violating "no Python in
  the core" and complicating distribution.
- **tree-sitter only** — rejected: cannot resolve types or cross-file symbols;
  cross-stack claims would be guesses.
- **LSP only** — rejected: no error tolerance, no fast whole-repo extraction,
  and LSP servers do not expose string literals/call sites uniformly.

## Consequences

Two analysis passes to coordinate. LSP servers are external processes to
manage (M04). Complete LSP coverage is explicitly a non-goal — LSP is used for
targeted resolution queries, not bulk extraction.
