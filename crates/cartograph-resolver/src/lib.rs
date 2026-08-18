//! Cross-language resolution.
//!
//! # Status: canonicalisation and matching (M03–M04)
//!
//! This is the subsystem the product exists for, and it is built in stages.
//! The pipeline now runs end to end for the HTTP boundary:
//!
//! ```text
//! RouteObservation   ─┐                        ┌─► RouteIndex ─┐
//!                     ├─► normalize ─► Canonical│               ├─► match ─► edge
//! HttpCallObservation ─┘                        └─► client ─────┘
//! ```
//!
//! - **M03, [`normalize`]** — one observation in, one canonical form out. A
//!   per-observation transformation that relates nothing to anything.
//! - **M04, [`matching`] and [`edge`]** — the first semantic join. A client
//!   observation is compared against indexed backend routes, classified, and
//!   only then, only if accepted, turned into an `HttpCall` edge carrying
//!   confidence, provenance and evidence.
//!
//! # What keeps the join honest
//!
//! Static segments compare exactly. A server's declared parameter accepts a
//! client's concrete value, but a client's *unknown* value against a server
//! literal is possible-not-verified and can never be exact. An undeclared
//! method is never read as GET. **Ambiguity produces no edge** — several
//! plausible routes is an unanswered question, and answering it arbitrarily
//! would poison every confidence number downstream. An unresolved template
//! prefix is refused outright until M05 evaluates the constant.
//!
//! Confidence values are the specification's **uncalibrated priors**; M08
//! replaces them with values fitted against a labelled benchmark. No accuracy
//! claim rests on them before then.
//!
//! # What normalisation will not do
//!
//! - **Infer parameters from values.** `/orders/123` keeps `123` as a static
//!   segment. Only route syntax declares a parameter.
//! - **Evaluate templates.** `` `${BASE}/orders` `` yields an unknown leading
//!   segment, not a resolved prefix, and a client whose prefix is unknown is
//!   refused for matching rather than aligned by guesswork. Evaluation is M05.
//! - **Interpret regular expressions.** A Django `re_path` pattern is
//!   [`NormalizationStatus::Unsupported`] with its raw form retained. "Cannot
//!   safely normalise" is a usable fact; "probably this route" is not.
//! - **Default a missing method.** [`Methods::Unknown`] is a state, not a gap
//!   to fill with `GET`.
//! - **Collapse trailing slashes.** `/orders` and `/orders/` stay distinct.
//!
//! Still to come: template evaluation (M05), ORM resolution (M06), and
//! LSP-backed symbol resolution when a milestone genuinely needs it — M04 did
//! not, because route matching joins observations rather than symbols. No
//! language model participates in any of it, ever: see RULE 007 and
//! `docs/adr/ADR-0007-no-llm-graph-construction.md`.

pub mod canonical;
pub mod edge;
pub mod matching;
pub mod normalize;

pub use canonical::{
    CanonicalPath, CanonicalRoute, Methods, NormalizationNote, NormalizationStatus,
    ObservationKind, RouteProvenance, Segment,
};
pub use edge::add_edge_for_match;
pub use matching::{
    Candidate, MatchReason, MatchResult, MatchStatus, MethodCompatibility, PathCompatibility,
    RouteIndex, UnsupportedReason, match_client, priors,
};
pub use normalize::{normalize_client_call, normalize_method, normalize_route_declaration};
