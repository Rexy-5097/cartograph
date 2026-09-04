//! An evidence bundle: the already-derived evidence behind one subgraph.
//!
//! # What this is for
//!
//! ASK (M16) explains a subgraph. It may only ever *interpret* evidence that
//! deterministic analysis already produced, and it may never propose, modify or
//! fill a gap in graph structure (RULE 007, ADR-0007). This module is the
//! artefact that boundary is drawn around: everything ASK is allowed to reason
//! about goes into a bundle, and nothing else does.
//!
//! A bundle copies claims, never source. Every field here is read straight off
//! an [`Edge`] the analysis produced — nothing computes, rounds or reformats.
//!
//! # Why a bundle carries a scope
//!
//! [`EdgeId`] is graph-local and restarts at zero for every analysis
//! (ADR-0011). That makes a citation that outlives its analysis genuinely
//! dangerous rather than merely stale: edge `42` almost certainly *exists* in
//! the next graph — as a completely different relationship — so a naive lookup
//! would succeed and quote confident, well-formed, wrong evidence. The desktop
//! evidence panel already defends against exactly this with a generation token.
//!
//! A bundle does the same, and derives its token from its own contents rather
//! than from a counter. A counter would make two constructions of the same
//! subgraph differ, which determinism forbids; a content-derived scope makes
//! them agree, and still separates two bundles whose contents differ. So
//! [`BundleScope`] answers "is this citation about the evidence I am holding?"
//! and a mismatch is refused rather than answered.
//!
//! The scope is meaningful within a run, not across runs — the same property
//! the desktop's generation token has, and for the same reason: it exists to
//! detect a mismatch, not to name a bundle forever.
//!
//! # Ordering
//!
//! Deterministic by semantic identity, never by `EdgeId` and never by discovery
//! order — the rule `blast` already follows. Two graphs describing one
//! architecture in different insertion orders produce the same bundle.

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use cartograph_core::{Confidence, Edge, EdgeId, EdgeKind, NodeIdentity, Provenance, identity_of};
use thiserror::Error;

use crate::ArchitectureGraph;
use crate::blast::BlastRadius;
use crate::trace::{Trace, TraceStep};

/// Identifies the contents of one bundle, so a citation cannot outlive them.
///
/// Derived from the bundle's own edges, so two constructions of the same
/// subgraph agree and two different subgraphs do not. It is not persisted and
/// means nothing across runs — it exists only to make "is this citation about
/// the evidence you are holding?" answerable.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct BundleScope(u64);

impl BundleScope {
    /// The scope as an opaque number, for diagnostics and transport.
    ///
    /// Callers must compare scopes rather than interpret them: the encoding is
    /// an implementation detail and carries no meaning of its own.
    #[must_use]
    pub const fn as_u64(self) -> u64 {
        self.0
    }
}

/// One edge's already-derived evidence, as a bundle carries it.
///
/// `from` and `to` are semantic identities rather than [`NodeId`]s
/// (`cartograph_core::identity_of`): a `NodeId` is graph-local like an
/// `EdgeId`, and a reader needs to know *what* the relationship connects, not
/// which slot it occupied.
///
/// [`NodeId`]: cartograph_core::NodeId
#[derive(Debug, Clone, PartialEq)]
pub struct BundledEdge {
    /// The edge, as this bundle's scope names it.
    pub edge: EdgeId,
    /// The artefact the relationship starts at.
    pub from: NodeIdentity,
    /// The artefact it reaches.
    pub to: NodeIdentity,
    /// What kind of relationship it is.
    pub kind: EdgeKind,
    /// The evidence text, byte for byte as the analysis produced it.
    ///
    /// This is the only text a citation may quote, and it is a *claim about*
    /// the source rather than the source itself (`cartograph_core::Evidence`).
    pub evidence: String,
    /// How the relationship was derived.
    pub provenance: Provenance,
    /// The uncalibrated prior the analysis attached. Not a probability.
    pub confidence: Confidence,
    /// Where the relationship was observed, repository-relative.
    ///
    /// `SourceLocation` refuses absolute paths at construction, so a machine
    /// layout cannot enter a bundle through this field.
    pub location: cartograph_core::SourceLocation,
}

/// Why a subgraph could not be bundled.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum BundleError {
    /// An edge named by the subgraph is not in the graph.
    ///
    /// A bundle that silently dropped it would under-report the evidence
    /// behind an answer, which is the one thing an evidence bundle may not do.
    #[error("edge {edge} is not in the graph")]
    MissingEdge {
        /// The edge that could not be resolved.
        edge: EdgeId,
    },
    /// An edge's evidence carries something that must never leave the machine.
    ///
    /// Refused rather than scrubbed: evidence is a claim the analysis made, and
    /// quietly rewriting it would make the bundle disagree with the graph it
    /// came from. The edge is named; **the offending text is not**, because an
    /// error message is a log line and RULE 015 applies to it too.
    #[error("evidence for edge {edge} carries {what}, which may not leave the machine")]
    UnsafeEvidence {
        /// The edge whose evidence was refused.
        edge: EdgeId,
        /// What kind of thing was found, never the thing itself.
        what: Unsafe,
    },
}

/// What was found in evidence that may not leave the machine.
///
/// Deliberately a category rather than the text: naming the category is enough
/// to debug, and reproducing the value would put it in the log the check exists
/// to keep it out of.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum Unsafe {
    /// A path inside somebody's home directory, or a drive-rooted path.
    MachinePath,
    /// A credential-shaped string.
    Secret,
}

impl std::fmt::Display for Unsafe {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::MachinePath => "a machine-specific path",
            Self::Secret => "a credential-shaped string",
        })
    }
}

/// Why a citation was refused.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum CitationError {
    /// The citation belongs to a different bundle.
    ///
    /// This is the stale-`EdgeId` defence. Refused rather than answered,
    /// because the edge the citation names probably *does* exist here — as
    /// something else entirely.
    #[error("citation belongs to another bundle")]
    ForeignBundle,
    /// The bundle does not contain the cited edge.
    #[error("edge {edge} is not in this bundle")]
    UnknownEdge {
        /// The edge the citation named.
        edge: EdgeId,
    },
    /// The quoted text is not part of that edge's evidence.
    ///
    /// This is the acceptance criterion made mechanical: "cite edge evidence
    /// verbatim" means an exact, byte-for-byte substring, and anything else —
    /// a paraphrase, a reflowed line, an invented clause — is refused.
    #[error("quoted text is not a verbatim substring of the evidence for edge {edge}")]
    NotVerbatim {
        /// The edge the citation named.
        edge: EdgeId,
    },
    /// The citation quotes nothing.
    ///
    /// An empty quotation would satisfy a substring test against any evidence
    /// at all, so it is not a citation.
    #[error("a citation must quote something")]
    Empty,
}

/// One quotation from one edge's evidence.
///
/// Constructed only through [`EvidenceBundle::cite`] or checked through
/// [`EvidenceBundle::verify`], so a citation that exists has been proved
/// against the bundle it names rather than merely asserted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Citation {
    scope: BundleScope,
    edge: EdgeId,
    text: String,
}

impl Citation {
    /// The bundle this citation was proved against.
    #[must_use]
    pub const fn scope(&self) -> BundleScope {
        self.scope
    }

    /// The edge whose evidence is quoted.
    #[must_use]
    pub const fn edge(&self) -> EdgeId {
        self.edge
    }

    /// The quoted text, byte for byte as it appears in the evidence.
    #[must_use]
    pub fn text(&self) -> &str {
        &self.text
    }

    /// Rebuilds a citation received from outside, without trusting it.
    ///
    /// Nothing about the value proves it belongs to any bundle; that is what
    /// [`EvidenceBundle::verify`] is for. This exists so a later layer can
    /// carry a citation across a boundary and then hand it back for checking.
    #[must_use]
    pub fn claimed(scope: BundleScope, edge: EdgeId, text: impl Into<String>) -> Self {
        Self {
            scope,
            edge,
            text: text.into(),
        }
    }
}

/// The already-derived evidence behind one subgraph.
///
/// Everything ASK may reason about, and nothing else.
#[derive(Debug, Clone, PartialEq)]
pub struct EvidenceBundle {
    scope: BundleScope,
    edges: Vec<BundledEdge>,
}

impl EvidenceBundle {
    /// Bundles the evidence for a set of edges.
    ///
    /// Duplicates collapse: a subgraph may reach one edge by several routes,
    /// and the evidence behind it is the same evidence either way.
    ///
    /// # Errors
    ///
    /// [`BundleError::MissingEdge`] if an edge is not in the graph, and
    /// [`BundleError::UnsafeEvidence`] if evidence carries a machine path or a
    /// credential-shaped string.
    pub fn from_edges(
        graph: &ArchitectureGraph,
        edges: impl IntoIterator<Item = EdgeId>,
    ) -> Result<Self, BundleError> {
        let mut bundled: Vec<BundledEdge> = Vec::new();
        let mut seen: Vec<EdgeId> = Vec::new();

        for id in edges {
            if seen.contains(&id) {
                continue;
            }
            seen.push(id);

            let edge = graph
                .edge(id)
                .ok_or(BundleError::MissingEdge { edge: id })?;
            if let Some(what) = unsafe_in(edge.evidence().as_str()) {
                return Err(BundleError::UnsafeEvidence { edge: id, what });
            }
            bundled.push(bundle_one(graph, edge));
        }

        // Deterministic by semantic identity, never by `EdgeId` and never by
        // discovery order — the rule `blast` follows. `EdgeId` breaks a full
        // tie only so that parallel edges carrying identical claims still have
        // a total order.
        bundled.sort_by(|a, b| {
            a.from
                .as_str()
                .cmp(b.from.as_str())
                .then_with(|| a.to.as_str().cmp(b.to.as_str()))
                .then_with(|| (a.kind as u8).cmp(&(b.kind as u8)))
                .then_with(|| a.evidence.cmp(&b.evidence))
                .then_with(|| a.edge.as_u64().cmp(&b.edge.as_u64()))
        });

        let scope = scope_of(&bundled);
        Ok(Self {
            scope,
            edges: bundled,
        })
    }

    /// Bundles the evidence behind a completed [`Trace`].
    ///
    /// # Errors
    ///
    /// As [`EvidenceBundle::from_edges`].
    pub fn from_trace(graph: &ArchitectureGraph, trace: &Trace) -> Result<Self, BundleError> {
        let mut ids = Vec::new();
        collect_steps(&trace.steps, &mut ids);
        Self::from_edges(graph, ids)
    }

    /// Bundles the evidence behind a [`BlastRadius`].
    ///
    /// One edge per dependent: the `via` edge is the one the reported
    /// confidence was taken from, so it is the edge an explanation must cite.
    ///
    /// # Errors
    ///
    /// As [`EvidenceBundle::from_edges`].
    pub fn from_blast(graph: &ArchitectureGraph, blast: &BlastRadius) -> Result<Self, BundleError> {
        Self::from_edges(graph, blast.reached.iter().map(|r| r.via))
    }

    /// The scope a citation must present.
    #[must_use]
    pub const fn scope(&self) -> BundleScope {
        self.scope
    }

    /// The bundled edges, in deterministic order.
    #[must_use]
    pub fn edges(&self) -> &[BundledEdge] {
        &self.edges
    }

    /// How many edges the bundle carries.
    #[must_use]
    pub fn len(&self) -> usize {
        self.edges.len()
    }

    /// Whether the bundle carries no evidence at all.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.edges.is_empty()
    }

    /// The bundled edge with this id, if the bundle carries it.
    #[must_use]
    pub fn get(&self, edge: EdgeId) -> Option<&BundledEdge> {
        self.edges.iter().find(|e| e.edge == edge)
    }

    /// Quotes part of one edge's evidence, proving the quotation as it goes.
    ///
    /// # Errors
    ///
    /// [`CitationError::UnknownEdge`] if the bundle does not carry the edge,
    /// [`CitationError::Empty`] for an empty quotation, and
    /// [`CitationError::NotVerbatim`] if the text is not an exact substring of
    /// that edge's evidence.
    pub fn cite(&self, edge: EdgeId, text: &str) -> Result<Citation, CitationError> {
        let citation = Citation::claimed(self.scope, edge, text);
        self.verify(&citation)?;
        Ok(citation)
    }

    /// Checks a citation that arrived from somewhere else.
    ///
    /// The four properties the citation contract requires, in the order that
    /// fails most cheaply: it belongs to this bundle, it names an edge this
    /// bundle carries, it quotes something, and what it quotes is a
    /// byte-for-byte substring of that edge's evidence.
    ///
    /// # Errors
    ///
    /// [`CitationError::ForeignBundle`], [`CitationError::UnknownEdge`],
    /// [`CitationError::Empty`] or [`CitationError::NotVerbatim`].
    pub fn verify(&self, citation: &Citation) -> Result<(), CitationError> {
        if citation.scope != self.scope {
            return Err(CitationError::ForeignBundle);
        }
        let edge = self.get(citation.edge).ok_or(CitationError::UnknownEdge {
            edge: citation.edge,
        })?;
        if citation.text.is_empty() {
            return Err(CitationError::Empty);
        }
        if !edge.evidence.contains(citation.text.as_str()) {
            return Err(CitationError::NotVerbatim {
                edge: citation.edge,
            });
        }
        Ok(())
    }
}

/// Copies one edge's claims into the bundle's shape.
fn bundle_one(graph: &ArchitectureGraph, edge: &Edge) -> BundledEdge {
    BundledEdge {
        edge: edge.id(),
        from: identity_for(graph, edge.source()),
        to: identity_for(graph, edge.target()),
        kind: edge.kind(),
        evidence: edge.evidence().as_str().to_owned(),
        provenance: edge.provenance(),
        confidence: edge.confidence(),
        location: edge.location().clone(),
    }
}

/// A node's stable identity, or an explicit placeholder when it is absent.
///
/// An edge cannot exist without its endpoints, so the placeholder is
/// unreachable in a well-formed graph; it exists so that a malformed one
/// degrades to a visible gap rather than to a panic.
fn identity_for(graph: &ArchitectureGraph, node: cartograph_core::NodeId) -> NodeIdentity {
    graph.node(node).map_or_else(
        || {
            cartograph_core::identity_parts(
                cartograph_core::NodeKind::ExternalService,
                "{unknown}",
                None,
            )
        },
        identity_of,
    )
}

/// Walks a trace's hops, depth first, collecting every edge it traversed.
fn collect_steps(steps: &[TraceStep], out: &mut Vec<EdgeId>) {
    for step in steps {
        out.push(step.edge);
        collect_steps(&step.next, out);
    }
}

/// Derives the scope from the bundle's ordered contents.
fn scope_of(edges: &[BundledEdge]) -> BundleScope {
    let mut hasher = DefaultHasher::new();
    edges.len().hash(&mut hasher);
    for edge in edges {
        edge.edge.as_u64().hash(&mut hasher);
        edge.from.as_str().hash(&mut hasher);
        edge.to.as_str().hash(&mut hasher);
        (edge.kind as u8).hash(&mut hasher);
        edge.evidence.hash(&mut hasher);
        format!("{:?}", edge.provenance).hash(&mut hasher);
        edge.confidence.get().to_bits().hash(&mut hasher);
        edge.location.file().hash(&mut hasher);
        edge.location.line().hash(&mut hasher);
    }
    BundleScope(hasher.finish())
}

/// Whether evidence carries something that may not leave the machine.
///
/// The shapes are QG-005's, restated here because the gate scans tracked files
/// and this scans a value at runtime — the same rule enforced at a second
/// place, not a new rule. A generic "starts with `/`" test is deliberately
/// absent: route evidence legitimately reads `GET /api/orders`, and a check
/// that refused it would be a check nobody could keep.
fn unsafe_in(text: &str) -> Option<Unsafe> {
    if has_home_path(text) || has_drive_path(text) {
        return Some(Unsafe::MachinePath);
    }
    if has_secret(text) {
        return Some(Unsafe::Secret);
    }
    None
}

/// A path inside somebody's home directory: `/Users/<name>/`, `/home/<name>/`.
fn has_home_path(text: &str) -> bool {
    ["/Users/", "/home/"].iter().any(|prefix| {
        text.match_indices(prefix).any(|(at, _)| {
            let rest = &text[at + prefix.len()..];
            // A named component that is itself followed by a separator: the
            // shape QG-005 matches, and the one that identifies a real account
            // rather than the bare directory.
            rest.split_once('/').is_some_and(|(name, _)| {
                !name.is_empty()
                    && name
                        .chars()
                        .all(|c| c.is_ascii_alphanumeric() || "_.-".contains(c))
            })
        })
    })
}

/// A drive-rooted Windows path: `C:\...` or `C:/...`.
fn has_drive_path(text: &str) -> bool {
    text.as_bytes().windows(3).any(|window| {
        window[0].is_ascii_alphabetic()
            && window[1] == b':'
            && (window[2] == b'\\' || window[2] == b'/')
    })
}

/// A credential-shaped string, in QG-005's vocabulary.
fn has_secret(text: &str) -> bool {
    const GITHUB: [&str; 5] = ["ghp_", "gho_", "ghu_", "ghs_", "ghr_"];

    if text.contains("-----BEGIN ") && text.contains("PRIVATE KEY") {
        return true;
    }
    if GITHUB.iter().any(|prefix| {
        text.match_indices(prefix).any(|(at, _)| {
            text[at + prefix.len()..]
                .chars()
                .take_while(char::is_ascii_alphanumeric)
                .count()
                >= 20
        })
    }) {
        return true;
    }
    if text.match_indices("AKIA").any(|(at, _)| {
        text[at + 4..]
            .chars()
            .take_while(|c| c.is_ascii_uppercase() || c.is_ascii_digit())
            .count()
            >= 16
    }) {
        return true;
    }
    ["xoxb-", "xoxa-", "xoxp-", "xoxr-", "xoxs-"]
        .iter()
        .any(|prefix| text.contains(prefix))
}

#[cfg(test)]
mod tests {
    use super::*;

    use cartograph_core::{Evidence, NodeId, NodeKind, SourceLocation};

    use crate::EdgeSpec;
    use crate::blast::blast_radius;
    use crate::trace::{DEFAULT_MAX_DEPTH, trace};

    fn node(graph: &mut ArchitectureGraph, name: &str) -> NodeId {
        graph
            .node_for(
                NodeKind::Function,
                name,
                Some(SourceLocation::new("app.py", 1).expect("location")),
            )
            .expect("node")
    }

    /// Adds one edge, with evidence the caller controls.
    fn edge_with(graph: &mut ArchitectureGraph, from: &str, to: &str, evidence: &str) -> EdgeId {
        let source = node(graph, from);
        let target = node(graph, to);
        graph
            .add_edge(EdgeSpec {
                source,
                target,
                kind: EdgeKind::Call,
                confidence: Confidence::new(0.9).expect("confidence"),
                provenance: Provenance::StaticImportResolution,
                evidence: Evidence::new(evidence).expect("evidence"),
                location: SourceLocation::new("app.py", 1).expect("location"),
                commit: None,
            })
            .expect("edge")
    }

    fn graph_of(edges: &[(&str, &str)]) -> ArchitectureGraph {
        let mut graph = ArchitectureGraph::new();
        for (from, to) in edges {
            edge_with(&mut graph, from, to, &format!("{from} calls {to}"));
        }
        graph
    }

    /// A credential-shaped string, assembled rather than written down.
    ///
    /// QG-005 scans tracked files for exactly this shape, and it is right
    /// to: a credential-shaped literal in a fixture is still one in the
    /// history. Prefix and body are joined at runtime, so the file carries
    /// neither and the test still exercises the real shape.
    fn fake_token() -> String {
        format!("{}{}", "ghp_", "abcdefghijklmnopqrstuvwxyz0123456789")
    }

    fn all_edges(graph: &ArchitectureGraph) -> Vec<EdgeId> {
        graph.edges().map(Edge::id).collect()
    }

    fn named(graph: &ArchitectureGraph, name: &str) -> cartograph_core::NodeId {
        graph.nodes().find(|n| n.name() == name).expect("node").id()
    }

    // -- construction ------------------------------------------------

    #[test]
    fn a_bundle_with_no_edges_is_empty_rather_than_an_error() {
        let graph = graph_of(&[]);
        let bundle = EvidenceBundle::from_edges(&graph, []).expect("bundle");

        assert!(bundle.is_empty());
        assert_eq!(bundle.len(), 0);
    }

    #[test]
    fn one_edge_carries_its_claims_unchanged() {
        let mut graph = ArchitectureGraph::new();
        let id = edge_with(&mut graph, "a", "b", "a calls b at app.py:1");
        let bundle = EvidenceBundle::from_edges(&graph, [id]).expect("bundle");

        assert_eq!(bundle.len(), 1);
        let bundled = bundle.get(id).expect("present");
        assert_eq!(bundled.evidence, "a calls b at app.py:1");
        assert_eq!(bundled.provenance, Provenance::StaticImportResolution);
        assert!((bundled.confidence.get() - 0.9).abs() < f32::EPSILON);
        assert_eq!(bundled.location.file(), "app.py");
    }

    #[test]
    fn several_edges_are_all_carried() {
        let graph = graph_of(&[("a", "b"), ("b", "c"), ("c", "d")]);
        let bundle = EvidenceBundle::from_edges(&graph, all_edges(&graph)).expect("bundle");

        assert_eq!(bundle.len(), 3);
    }

    #[test]
    fn one_edge_reached_twice_is_bundled_once() {
        let mut graph = ArchitectureGraph::new();
        let id = edge_with(&mut graph, "a", "b", "a calls b");
        let bundle = EvidenceBundle::from_edges(&graph, [id, id, id]).expect("bundle");

        assert_eq!(bundle.len(), 1);
    }

    // -- derived from a traversal ------------------------------------

    #[test]
    fn a_trace_bundles_every_hop_it_walked() {
        let graph = graph_of(&[("a", "b"), ("b", "c")]);
        let walk = trace(&graph, named(&graph, "a"), DEFAULT_MAX_DEPTH);
        let bundle = EvidenceBundle::from_trace(&graph, &walk).expect("bundle");

        assert_eq!(bundle.len(), 2);
    }

    #[test]
    fn a_blast_radius_bundles_the_edge_each_confidence_came_from() {
        let graph = graph_of(&[("a", "b"), ("b", "c")]);
        let radius = blast_radius(&graph, named(&graph, "c"));
        let bundle = EvidenceBundle::from_blast(&graph, &radius).expect("bundle");

        assert!(!radius.reached.is_empty(), "fixture must reach something");
        assert_eq!(bundle.len(), radius.reached.len());
    }

    // -- determinism -------------------------------------------------

    #[test]
    fn the_same_graph_bundles_identically_every_time() {
        let graph = graph_of(&[("a", "b"), ("b", "c"), ("c", "d")]);

        let first = EvidenceBundle::from_edges(&graph, all_edges(&graph)).expect("bundle");
        let second = EvidenceBundle::from_edges(&graph, all_edges(&graph)).expect("bundle");

        assert_eq!(first, second);
        assert_eq!(first.scope(), second.scope());
    }

    #[test]
    fn insertion_order_does_not_change_the_bundle() {
        let forward = graph_of(&[("a", "b"), ("b", "c"), ("c", "d")]);
        let backward = graph_of(&[("c", "d"), ("b", "c"), ("a", "b")]);

        let one = EvidenceBundle::from_edges(&forward, all_edges(&forward)).expect("bundle");
        let other = EvidenceBundle::from_edges(&backward, all_edges(&backward)).expect("bundle");

        let claims = |b: &EvidenceBundle| -> Vec<String> {
            b.edges()
                .iter()
                .map(|e| format!("{}->{}|{}", e.from.as_str(), e.to.as_str(), e.evidence))
                .collect()
        };
        assert_eq!(claims(&one), claims(&other));
    }

    #[test]
    fn the_order_edges_are_offered_in_does_not_change_the_bundle() {
        let graph = graph_of(&[("a", "b"), ("b", "c"), ("c", "d")]);
        let mut reversed = all_edges(&graph);
        reversed.reverse();

        let forward = EvidenceBundle::from_edges(&graph, all_edges(&graph)).expect("bundle");
        let backward = EvidenceBundle::from_edges(&graph, reversed).expect("bundle");

        assert_eq!(forward, backward);
    }

    #[test]
    fn parallel_edges_between_one_pair_stay_in_a_stable_order() {
        let mut graph = ArchitectureGraph::new();
        let first = edge_with(&mut graph, "a", "b", "a calls b once");
        let second = edge_with(&mut graph, "a", "b", "a calls b twice");

        let one = EvidenceBundle::from_edges(&graph, [first, second]).expect("bundle");
        let other = EvidenceBundle::from_edges(&graph, [second, first]).expect("bundle");

        assert_eq!(one, other);
        assert_eq!(one.len(), 2);
        // Ordered by the claim, not by the order they were offered in.
        assert_eq!(one.edges()[0].evidence, "a calls b once");
        assert_eq!(one.edges()[1].evidence, "a calls b twice");
    }

    // -- scope -------------------------------------------------------

    #[test]
    fn two_bundles_over_different_evidence_have_different_scopes() {
        let one = graph_of(&[("a", "b")]);
        let other = graph_of(&[("x", "y")]);

        let first = EvidenceBundle::from_edges(&one, all_edges(&one)).expect("bundle");
        let second = EvidenceBundle::from_edges(&other, all_edges(&other)).expect("bundle");

        assert_ne!(first.scope(), second.scope());
    }

    #[test]
    fn a_citation_from_another_bundle_is_refused_even_though_the_edge_id_exists_here() {
        // The stale-`EdgeId` hazard, made concrete: both graphs hold an edge
        // with the same id, describing entirely different relationships.
        let one = graph_of(&[("a", "b")]);
        let other = graph_of(&[("x", "y")]);

        let mine = EvidenceBundle::from_edges(&one, all_edges(&one)).expect("bundle");
        let theirs = EvidenceBundle::from_edges(&other, all_edges(&other)).expect("bundle");

        let id = theirs.edges()[0].edge;
        assert!(mine.get(id).is_some(), "the same id exists in this bundle");

        let foreign = Citation::claimed(theirs.scope(), id, "x calls y");
        assert_eq!(mine.verify(&foreign), Err(CitationError::ForeignBundle));
    }

    // -- citations ---------------------------------------------------

    #[test]
    fn a_verbatim_quotation_is_accepted() {
        let mut graph = ArchitectureGraph::new();
        let id = edge_with(
            &mut graph,
            "a",
            "b",
            "GET /api/orders matched GET /api/orders",
        );
        let bundle = EvidenceBundle::from_edges(&graph, [id]).expect("bundle");

        let citation = bundle
            .cite(id, "matched GET /api/orders")
            .expect("citation");
        assert_eq!(citation.text(), "matched GET /api/orders");
        assert_eq!(citation.edge(), id);
        assert_eq!(citation.scope(), bundle.scope());
        assert_eq!(bundle.verify(&citation), Ok(()));
    }

    #[test]
    fn the_whole_evidence_is_a_valid_quotation_of_itself() {
        let mut graph = ArchitectureGraph::new();
        let id = edge_with(&mut graph, "a", "b", "a calls b");
        let bundle = EvidenceBundle::from_edges(&graph, [id]).expect("bundle");

        assert!(bundle.cite(id, "a calls b").is_ok());
    }

    #[test]
    fn a_paraphrase_is_refused() {
        let mut graph = ArchitectureGraph::new();
        let id = edge_with(&mut graph, "a", "b", "a calls b");
        let bundle = EvidenceBundle::from_edges(&graph, [id]).expect("bundle");

        assert_eq!(
            bundle.cite(id, "a invokes b"),
            Err(CitationError::NotVerbatim { edge: id })
        );
    }

    #[test]
    fn text_absent_from_the_evidence_cannot_be_smuggled_into_a_citation() {
        let mut graph = ArchitectureGraph::new();
        let id = edge_with(&mut graph, "a", "b", "a calls b");
        let bundle = EvidenceBundle::from_edges(&graph, [id]).expect("bundle");

        // A superset of the evidence is not a substring of it.
        assert_eq!(
            bundle.cite(id, "a calls b and also writes to orders"),
            Err(CitationError::NotVerbatim { edge: id })
        );
    }

    #[test]
    fn a_citation_quoting_nothing_is_refused() {
        let mut graph = ArchitectureGraph::new();
        let id = edge_with(&mut graph, "a", "b", "a calls b");
        let bundle = EvidenceBundle::from_edges(&graph, [id]).expect("bundle");

        assert_eq!(bundle.cite(id, ""), Err(CitationError::Empty));
    }

    #[test]
    fn a_citation_naming_an_edge_outside_the_bundle_is_refused() {
        let mut graph = ArchitectureGraph::new();
        let inside = edge_with(&mut graph, "a", "b", "a calls b");
        let outside = edge_with(&mut graph, "c", "d", "c calls d");
        let bundle = EvidenceBundle::from_edges(&graph, [inside]).expect("bundle");

        assert_eq!(
            bundle.cite(outside, "c calls d"),
            Err(CitationError::UnknownEdge { edge: outside })
        );
    }

    #[test]
    fn a_citation_may_not_quote_one_edge_and_name_another() {
        let mut graph = ArchitectureGraph::new();
        let first = edge_with(&mut graph, "a", "b", "a calls b");
        let second = edge_with(&mut graph, "c", "d", "c calls d");
        let bundle = EvidenceBundle::from_edges(&graph, [first, second]).expect("bundle");

        assert_eq!(
            bundle.cite(first, "c calls d"),
            Err(CitationError::NotVerbatim { edge: first })
        );
    }

    // -- redaction ---------------------------------------------------

    #[test]
    fn evidence_carrying_a_unix_home_path_is_refused() {
        let mut graph = ArchitectureGraph::new();
        let id = edge_with(
            &mut graph,
            "a",
            "b",
            "resolved from /home/username/src/app.py",
        );

        assert_eq!(
            EvidenceBundle::from_edges(&graph, [id]),
            Err(BundleError::UnsafeEvidence {
                edge: id,
                what: Unsafe::MachinePath
            })
        );
    }

    #[test]
    fn evidence_carrying_a_macos_home_path_is_refused() {
        let mut graph = ArchitectureGraph::new();
        let id = edge_with(
            &mut graph,
            "a",
            "b",
            "resolved from /Users/username/src/app.py",
        );

        assert!(EvidenceBundle::from_edges(&graph, [id]).is_err());
    }

    #[test]
    fn evidence_carrying_a_drive_rooted_path_is_refused() {
        let mut graph = ArchitectureGraph::new();
        let id = edge_with(
            &mut graph,
            "a",
            "b",
            r"resolved from C:\Users\username\app.py",
        );

        assert_eq!(
            EvidenceBundle::from_edges(&graph, [id]),
            Err(BundleError::UnsafeEvidence {
                edge: id,
                what: Unsafe::MachinePath
            })
        );
    }

    #[test]
    fn evidence_carrying_a_token_is_refused() {
        let mut graph = ArchitectureGraph::new();
        let id = edge_with(
            &mut graph,
            "a",
            "b",
            &format!("authenticated with {}", fake_token()),
        );

        assert_eq!(
            EvidenceBundle::from_edges(&graph, [id]),
            Err(BundleError::UnsafeEvidence {
                edge: id,
                what: Unsafe::Secret
            })
        );
    }

    #[test]
    fn evidence_carrying_a_private_key_block_is_refused() {
        let mut graph = ArchitectureGraph::new();
        let block = format!("-----BEGIN {} PRIVATE KEY-----", "RSA");
        let id = edge_with(&mut graph, "a", "b", &block);

        assert!(EvidenceBundle::from_edges(&graph, [id]).is_err());
    }

    #[test]
    fn a_route_path_is_not_mistaken_for_a_machine_path() {
        // The check must not refuse the evidence the resolver actually
        // produces: a leading `/` is a route, not somebody's home directory.
        let mut graph = ArchitectureGraph::new();
        let id = edge_with(
            &mut graph,
            "a",
            "b",
            "GET /api/orders matched GET /api/orders at web/client.ts:2",
        );

        assert!(EvidenceBundle::from_edges(&graph, [id]).is_ok());
    }

    #[test]
    fn the_refusal_never_repeats_the_thing_it_refused() {
        // An error message is a log line, and RULE 015 applies to it too.
        let mut graph = ArchitectureGraph::new();
        let secret = fake_token();
        let id = edge_with(&mut graph, "a", "b", &format!("token {secret}"));

        let error = EvidenceBundle::from_edges(&graph, [id]).expect_err("refused");
        let rendered = error.to_string();

        assert!(
            !rendered.contains(secret.as_str()),
            "refusal leaked the secret"
        );
        assert!(!rendered.contains("token "), "refusal quoted the evidence");
    }

    #[test]
    fn a_bundle_carries_no_absolute_path_in_any_field() {
        let graph = graph_of(&[("a", "b"), ("b", "c")]);
        let bundle = EvidenceBundle::from_edges(&graph, all_edges(&graph)).expect("bundle");

        for edge in bundle.edges() {
            let rendered = format!(
                "{}|{}|{}|{}",
                edge.from.as_str(),
                edge.to.as_str(),
                edge.evidence,
                edge.location.file()
            );
            assert!(unsafe_in(&rendered).is_none(), "unsafe content: {rendered}");
            assert!(
                !edge.location.file().starts_with('/'),
                "absolute location: {}",
                edge.location.file()
            );
        }
    }
}
