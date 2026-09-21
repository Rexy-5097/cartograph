//! The ASK provider boundary: a question, derived evidence, and an answer that
//! cannot exist unless every claim in it cites evidence that was actually sent.
//!
//! # What this crate is for
//!
//! M16's acceptance sentence has two halves. `cartograph_desktop::ask` delivers
//! the second — *"with AI disabled the feature degrades to showing the raw
//! evidence"* — and this crate is the first: *"ASK answers cite edge evidence
//! verbatim"*.
//!
//! It owns the **provider abstraction** and the **citation check**, and nothing
//! else. It does not own authorization, repository identity, filesystem
//! discovery or graph construction, and it cannot reach any of them: its only
//! dependencies are `cartograph-core` and `cartograph-graph`.
//!
//! # The dependency list is the security boundary
//!
//! [ADR-0021](../../../docs/adr/ADR-0021-ask-boundary-and-citation-contract.md)
//! Amendment 3, decision C3, puts the AI provider here rather than in
//! `cartograph-pipeline` for one reason: capability that lives in a shared
//! crate is capability every client acquires. The HTTP client arrives in this
//! crate in a later step and in **no other**, so `cartograph-cli` and
//! `cartograph-mcp` remain incapable of network egress by the shape of the
//! dependency graph rather than by anyone remembering.
//!
//! What that buys, structurally rather than by promise:
//!
//! - `cartograph_pipeline::authorization::RepositoryIdentity` is **not
//!   nameable** here. This crate cannot put one in a request because it cannot
//!   refer to one.
//! - The `ArchitectureGraph` never crosses the boundary. A provider is handed
//!   an [`EvidenceBundle`], which is a copy of already-derived claims.
//! - A [`SourceLocation`] refuses an absolute path at construction, so a
//!   machine layout cannot enter a bundle and therefore cannot leave in one.
//!
//! # What a model may produce
//!
//! Explanation text and structured citations. Nothing else. It cannot create a
//! node, create an edge, alter [`Evidence`] or have any output re-enter graph
//! construction, because nothing in this crate can do those things — ADR-0007
//! is enforced by the dependency graph, not by a prompt.
//!
//! # Validation is the gate, not the schema
//!
//! A provider may be asked for structured output, and it may comply perfectly
//! while quoting text no edge contains. Shape is not truth. So every citation
//! is re-checked **locally, against the bundle that was actually sent**, by
//! [`EvidenceBundle::cite`] — the machinery M16 Slice 1 already built and
//! tested.
//!
//! One invalid citation rejects **the whole answer**, not the item. A partially
//! valid answer is the most dangerous output this feature can produce, because
//! it looks checked. [`Answer`] has private fields and no public constructor:
//! the only way to obtain one is [`validate`], so "render an unvalidated
//! answer" is not an expressible program.
//!
//! # What is deliberately absent
//!
//! No HTTP client, no TLS and no credential store. [`GroqProvider`] is a
//! complete provider that runs entirely over the [`transport`] seam, and that
//! seam contains no client: nothing in this crate can open a socket.
//! Everything here runs offline, and its only dependencies outside the
//! workspace are `serde` and `serde_json` — both already workspace
//! dependencies — which build the Groq request bytes in [`groq`]. `ureq` and
//! `rustls` arrive in a later step, behind [`Transport`], and cannot change any
//! rule stated here.
//!
//! [`Evidence`]: cartograph_core::Evidence
//! [`SourceLocation`]: cartograph_core::SourceLocation
//! [`EvidenceBundle`]: cartograph_graph::bundle::EvidenceBundle
//! [`EvidenceBundle::cite`]: cartograph_graph::bundle::EvidenceBundle::cite

pub mod answer;
pub mod error;
pub mod groq;
pub mod provider;
pub mod question;
pub mod testing;
pub mod transport;

pub use answer::{
    Answer, ClaimedAnswer, ClaimedCitation, ClaimedItem, ExplanationItem, revalidate, validate,
};
pub use error::{ProviderError, ResponseFault};
pub use groq::GroqProvider;
pub use provider::Provider;
pub use question::{Question, QuestionError};
pub use transport::{Header, HttpResponse, Transport, TransportError};
