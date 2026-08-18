//! Cross-language resolution.
//!
//! # Status: reserved, not implemented
//!
//! This crate is empty on purpose. M00 establishes the workspace; resolution
//! begins at M03.
//!
//! This is the subsystem the product exists for. A React component, an HTTP
//! route, a Python handler and a database table are four artefacts in three
//! languages with no shared symbol table. Connecting them is not a graph
//! problem — it is three compiler problems:
//!
//! 1. **Partial evaluation of constructed URLs.** `` `${BASE}/orders/${id}` ``
//!    must become `/api/v1/orders/{*}` through constant propagation and
//!    abstract interpretation over string values. Unknown segments stay
//!    explicitly unknown; the resolver never guesses a value (RULE 010).
//! 2. **Normalisation across route dialects.** `/orders/:id`,
//!    `/orders/{id}`, `/orders/<int:id>` and `` /orders/${id} `` denote one
//!    route and must collapse to one canonical form.
//! 3. **ORM model resolution.** Handler to service to repository to model to
//!    table.
//!
//! Where an `OpenAPI` document exists it is ground truth and inference is
//! skipped.
//!
//! Planned scope, in milestone order:
//!
//! | Milestone | Scope                                                       |
//! |-----------|-------------------------------------------------------------|
//! | M03       | Canonical route normalisation                               |
//! | M04       | Cross-language resolver: TypeScript call site to Python handler |
//! | M05       | Template evaluator for dynamically constructed URLs         |
//! | M06       | ORM resolution: handler to model to table                   |
//!
//! # One rule that will not move
//!
//! No language model participates in any of this. Every edge this crate
//! eventually produces is computed by static analysis and carries the evidence
//! for its own claim. A model may explain a subgraph after the fact (M16); it
//! may never propose an edge. See RULE 007 and
//! `docs/adr/ADR-0007-no-llm-graph-construction.md`.
