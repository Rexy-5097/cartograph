//! Cross-language resolution.
//!
//! The subsystem the product exists for. It turns the parser's per-file
//! observations into evidenced edges across the stack: a client call to the
//! handler that serves it (`HttpCall`), a component to the client function that
//! issues its request (`Call`), a handler to the model it uses (`OrmAccess`),
//! and a model to its table (`Queries`).
//!
//! # The pipeline
//!
//! ```text
//! TypeScript ─► HTTP call observations ─► symbolic URL values ─┐
//!                                                              ├─► canonical route ─► match ─► HttpCall
//! Python ────► route declarations ─► served path ──────────────┘
//! Python ────► model classes ─► table names ─► model uses ─► OrmAccess, Queries
//! TypeScript ─► component ─► hook ─► client function ─► Call
//! ```
//!
//! - [`canonical`] and [`normalize`] — one observation in, one canonical form
//!   out. A per-observation transformation that relates nothing to anything.
//! - [`symbolic`], [`evaluator`] and [`dynamic`] — a client URL evaluated into
//!   a [`SymbolicValue`], with constants propagated through module scope and
//!   across files. Whatever cannot be determined stays unknown.
//! - [`routers`] — the path a route is actually served on, composed from router
//!   prefixes and `include_router` mountings.
//! - [`matching`] and [`edge`] — the first semantic join. A client observation
//!   is compared against indexed backend routes, classified, and only then,
//!   only if accepted, turned into an `HttpCall` edge carrying confidence,
//!   provenance and evidence.
//! - [`imports`] — a name at a use site resolved through its own import
//!   statement to the declaration it refers to, following re-exports for a
//!   bounded number of hops.
//! - [`client_trace`] — the client wrapper kept on the path. An `HttpCall` edge
//!   is sourced at the function that issues the request, and the components and
//!   hooks above it reach that function through `Call` edges resolved by import,
//!   for a bounded number of hops.
//! - [`orm`] — `SQLAlchemy` and Django model classes to their tables, and
//!   handlers to the models they use.
//! - [`access_cache`] and [`dependencies`] — the per-file cache for ORM access
//!   resolution, keyed on what each resolution actually read.
//!
//! Every edge this crate creates carries one of three provenances: route
//! matching for `HttpCall`, static import resolution for `Call`, and ORM
//! resolution for `OrmAccess` and `Queries`.
//!
//! # What keeps the join honest
//!
//! Static segments compare exactly. A server's declared parameter accepts a
//! client's concrete value, but a client's *unknown* value against a server
//! literal is possible-not-verified and can never be exact. An undeclared
//! method is never read as GET. **Ambiguity produces no edge** — several
//! plausible routes is an unanswered question, and answering it arbitrarily
//! would poison every confidence number downstream. A client whose leading
//! prefix is still unknown after evaluation is refused outright rather than
//! aligned by guesswork.
//!
//! Confidence values are the specification's **uncalibrated priors**, chosen
//! by evidence class ([`priors`]). M08 measured them against independently
//! labelled edges and did not replace them: where they could be measured they
//! understate accuracy, and none may be read as a probability. See
//! `docs/writeup/technical-writeup.md`.
//!
//! # What normalisation will not do
//!
//! - **Infer parameters from values.** `/orders/123` keeps `123` as a static
//!   segment. Only route syntax declares a parameter.
//! - **Evaluate templates.** Evaluation happens before normalisation, in
//!   [`dynamic`]. A segment evaluation could not determine stays unknown — so
//!   `` `${BASE}/orders` `` with an undetermined `BASE` keeps an unknown leading
//!   segment rather than a resolved prefix.
//! - **Interpret regular expressions.** A Django `re_path` pattern is
//!   [`NormalizationStatus::Unsupported`] with its raw form retained. "Cannot
//!   safely normalise" is a usable fact; "probably this route" is not.
//! - **Default a missing method.** [`Methods::Unknown`] is a state, not a gap
//!   to fill with `GET`.
//! - **Collapse trailing slashes.** `/orders` and `/orders/` stay distinct.
//!
//! # What this crate does not do
//!
//! - **Resolve JavaScript backend routes.** Routes come from Python
//!   declarations. The parser recognises an Express- or Hono-style registration
//!   only so that it is not mistaken for a client request; it is never a route.
//! - **Confirm a match against an `OpenAPI` specification.** The domain model
//!   reserves a provenance and a prior for that; nothing here produces either.
//! - **Follow an ORM access through intermediate layers.** Only direct
//!   `Model(...)` and `Model.objects.…` accesses inside a handler are resolved.
//!   A service or repository layer between a handler and a model is not
//!   followed, `session.query(Model)` and `select(Model)` are not recognised,
//!   and raw SQL is not linked to models.
//! - **Compose every prefix.** An `include_router` whose child is an attribute
//!   rather than a name, or a prefix applied by a project-local helper, does not
//!   compose.
//! - **Resolve symbols through a language server.** Resolution is syntactic and
//!   import-based. Route matching joins observations rather than symbols, so no
//!   milestone has yet needed LSP-backed resolution.
//!
//! No language model participates in any of it, ever: see RULE 007 and
//! `docs/adr/ADR-0007-no-llm-graph-construction.md`.

pub mod access_cache;
pub mod canonical;
pub mod client_trace;
pub mod dependencies;
pub mod dynamic;
pub mod edge;
pub mod evaluator;
pub mod imports;
pub mod matching;
pub mod normalize;
pub mod orm;
pub mod routers;
pub mod symbolic;
pub mod urlconf;

pub use access_cache::{
    AccessCache, AccessCacheError, AccessCacheStats, GlobalFingerprints, RecomputeHook,
};
pub use canonical::{
    CanonicalPath, CanonicalRoute, Methods, NormalizationNote, NormalizationStatus,
    ObservationKind, RouteProvenance, Segment,
};
pub use client_trace::{ClientFunction, add_client_call_edges, client_functions};
pub use dependencies::{
    AliasSetFingerprint, PathSetFingerprint, PerFileDependencyIdentity, ResolutionContext,
};
pub use dynamic::{
    ExportedConstants, ResolvedUrl, collect_exported_constants, resolve_url, scope_for_file,
    with_resolved_url,
};
pub use edge::add_edge_for_match;
pub use evaluator::{Scope, evaluate_expression, evaluate_string_fact};
pub use imports::{ModuleIndex, Resolution};
pub use matching::{
    Candidate, MatchReason, MatchResult, MatchStatus, MethodCompatibility, PathCompatibility,
    RouteIndex, UnsupportedReason, match_client, priors,
};
pub use normalize::{normalize_client_call, normalize_method, normalize_route_declaration};
pub use orm::{
    AccessKind, ModelSetFingerprint, OrmAccessSite, OrmAnalysis, OrmFlavor, OrmModel,
    PerFileAccessAnalysis, TableName, add_orm_edges, canonical_model_set, discover_accesses,
    discover_accesses_tracked, discover_models,
};
pub use routers::{PrefixResolution, RouterIndex, with_composed_prefix};
pub use symbolic::SymbolicValue;
pub use urlconf::UrlConfIndex;
