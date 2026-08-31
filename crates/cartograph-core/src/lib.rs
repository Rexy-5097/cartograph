//! Cartograph domain model.
//!
//! This crate owns the vocabulary the whole product speaks: what a node is,
//! what an edge is, and — the part that distinguishes Cartograph from a
//! dependency-graph drawer — how a relationship was derived and how much it
//! should be trusted.
//!
//! # The invariant this crate exists to enforce
//!
//! Cartograph never stores `A -> B`. It stores the claim *and how it was
//! known*. [`Edge`] therefore has no `Default` and no partial constructor:
//! building one requires [`Confidence`], [`Provenance`], [`Evidence`] and a
//! [`SourceLocation`]. An edge that cannot answer "why do you think these two
//! things are connected?" cannot be constructed at all.
//!
//! # What is deliberately absent
//!
//! At M00 there is no parsing, no resolution and no storage. This crate is the
//! type foundation those milestones build on. Node identity is assigned by the
//! graph and is not yet content-addressed. M10 built its incremental engine
//! without it and deferred it again; see [`id::NodeId`] and ADR-0014, which
//! names the consumer (M13) and the gate the work must pass.

pub mod confidence;
pub mod edge;
pub mod error;
pub mod evidence;
pub mod id;
pub mod location;
pub mod node;
pub mod provenance;

pub use confidence::Confidence;
pub use edge::{Edge, EdgeKind};
pub use error::CoreError;
pub use evidence::Evidence;
pub use id::{CommitId, EdgeId, NodeId};
pub use location::SourceLocation;
pub use node::{Node, NodeKind};
pub use provenance::Provenance;

/// Version of the frozen engineering specification this domain model implements.
///
/// Bumped only when the specification itself is re-frozen, never on ordinary
/// releases.
pub const SPEC_VERSION: &str = "V3";
