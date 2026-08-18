//! Language-specific extraction.
//!
//! # Status: reserved, not implemented
//!
//! This crate is empty on purpose. M00 establishes the workspace; extraction
//! begins at M01.
//!
//! When it is implemented, this crate's responsibility is the *first* tier of
//! the two-tier analysis described in `docs/adr/ADR-0002-two-tier-analysis.md`:
//! parse source with tree-sitter and extract the syntactic facts — declared
//! symbols, imports, call sites, string and template literals, route
//! declarations. It answers "what does this file say?".
//!
//! It does not resolve anything. Deciding which declaration a call site refers
//! to needs type information, which comes from a language server and belongs to
//! `cartograph-resolver`.
//!
//! Planned scope, in milestone order:
//!
//! | Milestone | Scope                                                |
//! |-----------|------------------------------------------------------|
//! | M01       | TypeScript: symbols, imports, calls, string literals  |
//! | M02       | Python: symbols, plus `FastAPI`, `Flask` and `Django` routes|
//!
//! Nothing is exported yet. An empty crate is more honest than a set of traits
//! invented before a single file has been parsed — the abstraction should come
//! from the extraction work, not precede it.
