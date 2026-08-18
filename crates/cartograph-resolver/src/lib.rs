//! Cross-language resolution.
//!
//! # Status: canonicalisation only (M03)
//!
//! This is the subsystem the product exists for, and it is built in stages.
//! What exists today is **M03: canonical route normalisation** — one
//! observation in, one canonical form out.
//!
//! ```text
//! RouteObservation   ─┐
//!                     ├─► normalize ─► CanonicalRoute
//! HttpCallObservation ─┘
//! ```
//!
//! # The boundary this crate is currently defending
//!
//! ```text
//! M03 (here):    raw observation ─► canonical representation
//! M04 (not yet): client call + backend route ─► MATCH ─► evidenced edge
//! ```
//!
//! Canonicalisation is a **per-observation** transformation. Nothing here
//! compares a client call with a route declaration, decides they describe the
//! same endpoint, attaches confidence, or produces a graph edge. The
//! structural helpers this crate exposes — [`CanonicalRoute::same_shape`] and
//! [`CanonicalRoute::compatible_method`] — answer "could these be
//! comparable?", a question about two shapes, never a claim about two systems.
//!
//! Everything downstream depends on that separation holding: a canonical form
//! that quietly assumed a match would make every later confidence number
//! meaningless.
//!
//! # What normalisation will not do
//!
//! - **Infer parameters from values.** `/orders/123` keeps `123` as a static
//!   segment. Only route syntax declares a parameter.
//! - **Evaluate templates.** `` `${BASE}/orders` `` yields an unknown leading
//!   segment, not a resolved prefix. Evaluation is M05.
//! - **Interpret regular expressions.** A Django `re_path` pattern is
//!   [`NormalizationStatus::Unsupported`] with its raw form retained. "Cannot
//!   safely normalise" is a usable fact; "probably this route" is not.
//! - **Default a missing method.** [`Methods::Unknown`] is a state, not a gap
//!   to fill with `GET`.
//! - **Collapse trailing slashes.** `/orders` and `/orders/` stay distinct.
//!
//! Still to come: the cross-language join and LSP-backed symbol resolution
//! (M04), template evaluation (M05), ORM resolution (M06). No language model
//! participates in any of it — see RULE 007 and
//! `docs/adr/ADR-0007-no-llm-graph-construction.md`.

pub mod canonical;
pub mod normalize;

pub use canonical::{
    CanonicalPath, CanonicalRoute, Methods, NormalizationNote, NormalizationStatus,
    ObservationKind, RouteProvenance, Segment,
};
pub use normalize::{normalize_client_call, normalize_method, normalize_route_declaration};
