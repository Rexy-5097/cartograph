//! Structural diff of two independently analysed graphs.
//!
//! # What corresponds to what
//!
//! Two branches are two separate analyses, so nothing about a handle survives
//! between them: `NodeId` restarts at zero in each graph and names different
//! artefacts on each side. Correspondence is therefore by [`NodeIdentity`] —
//! the stable `(kind, name, file)` value from M13's identity slice — and never
//! by id.
//!
//! An edge corresponds to an edge when **source identity, target identity and
//! kind** all match. That triple is the edge's identity here; everything else
//! about an edge is a fact *about* the relationship rather than a statement of
//! which relationship it is.
//!
//! # Added, removed, changed — and why the boundary sits there
//!
//! - **Added** — the triple exists on the right and not on the left.
//! - **Removed** — the triple exists on the left and not on the right.
//! - **Changed** — the triple exists on both, and the evidence differs
//!   meaningfully: confidence, provenance, or the file the claim was observed
//!   in.
//!
//! A different endpoint or a different kind is **not** a change. It is a
//! removal and an addition, because the claim itself is a different claim:
//! "`a` calls `b`" becoming "`a` calls `c`" is not the first claim with new
//! evidence, and reporting it as one would tell a reader an edge was adjusted
//! when it was replaced.
//!
//! # Location is compared by file, not by line
//!
//! This is a judgement call and it is the one worth arguing with. Comparing
//! the full location would mark an edge changed whenever the line it was
//! observed at moved, so inserting one import at the top of a file would
//! report every relationship below it as changed — the exact churn that
//! excluding the line from *node* identity exists to prevent. A line shift is
//! not a meaningful difference in evidence; it is the same observation at a
//! new offset.
//!
//! The line is still reported, on both sides, so a reader can see where each
//! observation sits. It simply does not, alone, make an edge changed. The test
//! `a_line_shift_alone_is_not_a_change` pins this.
//!
//! # What is deliberately not compared
//!
//! `EdgeId` (graph-local, meaningless across analyses), `created_at` (the
//! moment the analysis ran, which differs on every run), and `commit` (names
//! the revision, so including it would mark *every* edge changed between two
//! revisions and say nothing).
//!
//! # Inherited limitation: graph construction merges some artefacts
//!
//! `ArchitectureGraph::node_for` deduplicates nodes on `(kind, name, file)`,
//! so two same-named methods in different classes of one file are already a
//! single node before any of this runs, as are nested definitions sharing a
//! name with an enclosing scope. This diff inherits that: it cannot distinguish
//! artefacts the graph never distinguished. Changing it would mean giving nodes
//! a scope or signature and altering `node_for`, which would move M10's
//! differential oracle and M12's blast results. Recorded here rather than
//! quietly worked around.
//!
//! # Parallel edges
//!
//! A graph may hold more than one edge sharing a triple. They are matched
//! within the triple: identical facts pair off as unchanged first, then the
//! remainder pair in sorted order as changed, and any excess on one side is an
//! addition or a removal. Sorting first is what keeps the pairing deterministic
//! rather than dependent on iteration order.

use std::collections::BTreeMap;

use cartograph_core::{Edge, EdgeKind, NodeIdentity, Provenance, identity_of};

use crate::ArchitectureGraph;

/// Which relationship an edge asserts, independent of the graph holding it.
///
/// Ordered, so a diff can be emitted in a stable sequence: by source, then
/// target, then kind.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct EdgeIdentity {
    /// The identity of the node the relationship starts at.
    pub source: NodeIdentity,
    /// The identity of the node it points to.
    pub target: NodeIdentity,
    /// The relationship kind, as its stable token.
    pub kind: &'static str,
}

/// What an analysis observed about a relationship.
///
/// The evidence a reported edge carries. Every field here comes from the edge
/// itself, so a reported difference can always be traced to an observation
/// rather than to the diff's bookkeeping.
#[derive(Debug, Clone, PartialEq)]
pub struct EdgeFacts {
    /// The uncalibrated prior the analysis assigned. Not a probability.
    pub confidence: f32,
    /// Which analysis produced the edge.
    pub provenance: Provenance,
    /// Repository-relative file the claim was observed in.
    pub file: String,
    /// One-based line, reported but not compared. See the module docs.
    pub line: u32,
    /// The observation supporting the edge.
    pub evidence: String,
}

impl EdgeFacts {
    /// The parts that decide whether an edge changed.
    ///
    /// Deliberately excludes the line and the evidence summary: both move when
    /// code shifts without the relationship differing, and the summary embeds a
    /// location of its own.
    fn comparable(&self) -> (u32, Provenance, &str) {
        // Confidence is compared through its bit pattern rather than with `==`
        // so the comparison is total and reflexive; the values come from the
        // same analyser on both sides, so equal claims produce equal bits.
        (
            self.confidence.to_bits(),
            self.provenance,
            self.file.as_str(),
        )
    }

    /// Total ordering key, for pairing parallel edges deterministically.
    fn ordering_key(&self) -> (u32, String, u32, String) {
        (
            self.confidence.to_bits(),
            format!("{:?}", self.provenance),
            self.line,
            self.evidence.clone(),
        )
    }
}

/// Which field of an edge's evidence differed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ChangedField {
    /// The confidence the analysis assigned.
    Confidence,
    /// Which analysis produced the edge.
    Provenance,
    /// The file the claim was observed in.
    File,
}

impl ChangedField {
    /// The stable token for the wire.
    #[must_use]
    pub fn token(self) -> &'static str {
        match self {
            Self::Confidence => "confidence",
            Self::Provenance => "provenance",
            Self::File => "file",
        }
    }
}

/// An edge present on one side only.
#[derive(Debug, Clone, PartialEq)]
pub struct EdgeAppearance {
    /// The relationship asserted.
    pub identity: EdgeIdentity,
    /// What the analysis observed about it.
    pub facts: EdgeFacts,
}

/// An edge present on both sides whose evidence differs.
#[derive(Debug, Clone, PartialEq)]
pub struct ChangedEdge {
    /// The relationship asserted, identical on both sides by definition.
    pub identity: EdgeIdentity,
    /// What the earlier analysis observed.
    pub before: EdgeFacts,
    /// What the later analysis observed.
    pub after: EdgeFacts,
    /// Which fields differ, ascending and deduplicated.
    pub fields: Vec<ChangedField>,
}

/// The structural difference between two analyses.
#[derive(Debug, Clone, PartialEq)]
pub struct GraphDiff {
    /// Relationships the later analysis asserts and the earlier did not.
    pub added: Vec<EdgeAppearance>,
    /// Relationships the earlier analysis asserted and the later does not.
    pub removed: Vec<EdgeAppearance>,
    /// Relationships both assert, with differing evidence.
    pub changed: Vec<ChangedEdge>,
    /// Relationships both assert identically. Counted, not listed: a reader
    /// wants what moved, and a real repository has thousands that did not.
    pub unchanged: usize,
}

impl GraphDiff {
    /// Whether the two analyses assert exactly the same relationships.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.added.is_empty() && self.removed.is_empty() && self.changed.is_empty()
    }

    /// How many differences are reported.
    #[must_use]
    pub fn len(&self) -> usize {
        self.added.len() + self.removed.len() + self.changed.len()
    }
}

/// The stable token for an edge kind.
///
/// Spelled out rather than taken from `Debug`, for the same reason node kinds
/// are: it is a wire value once a diff quotes it. `EdgeKind` is
/// `#[non_exhaustive]`, so the wildcard is required; it names the kind as
/// unknown rather than guessing, and an unknown kind still diffs correctly
/// because the token participates only in identity and ordering.
#[must_use]
pub fn edge_kind_token(kind: EdgeKind) -> &'static str {
    match kind {
        EdgeKind::Import => "import",
        EdgeKind::Call => "call",
        EdgeKind::Inherits => "inherits",
        EdgeKind::Implements => "implements",
        EdgeKind::References => "references",
        EdgeKind::HttpCall => "http-call",
        EdgeKind::OrmAccess => "orm-access",
        EdgeKind::Queries => "queries",
        _ => "unknown",
    }
}

/// Describes one edge of `graph`, or `None` if an endpoint is missing.
///
/// A missing endpoint would be a corrupt graph. It is skipped rather than
/// rendered as a placeholder, because inventing an identity for it would let
/// the corruption match something on the other side.
fn describe(graph: &ArchitectureGraph, edge: &Edge) -> Option<(EdgeIdentity, EdgeFacts)> {
    let source = graph.node(edge.source())?;
    let target = graph.node(edge.target())?;

    Some((
        EdgeIdentity {
            source: identity_of(source),
            target: identity_of(target),
            kind: edge_kind_token(edge.kind()),
        },
        EdgeFacts {
            confidence: edge.confidence().get(),
            provenance: edge.provenance(),
            file: edge.location().file().to_owned(),
            line: edge.location().line(),
            evidence: edge.evidence().as_str().to_owned(),
        },
    ))
}

/// Groups a graph's edges by the relationship they assert.
fn group(graph: &ArchitectureGraph) -> BTreeMap<EdgeIdentity, Vec<EdgeFacts>> {
    let mut out: BTreeMap<EdgeIdentity, Vec<EdgeFacts>> = BTreeMap::new();
    for edge in graph.edges() {
        if let Some((identity, facts)) = describe(graph, edge) {
            out.entry(identity).or_default().push(facts);
        }
    }
    // Sorting inside each group is what makes the pairing of parallel edges a
    // property of their content rather than of iteration order.
    for facts in out.values_mut() {
        facts.sort_by_key(EdgeFacts::ordering_key);
    }
    out
}

/// Which fields of two fact sets differ.
fn changed_fields(before: &EdgeFacts, after: &EdgeFacts) -> Vec<ChangedField> {
    let mut fields = Vec::new();
    if before.confidence.to_bits() != after.confidence.to_bits() {
        fields.push(ChangedField::Confidence);
    }
    if before.provenance != after.provenance {
        fields.push(ChangedField::Provenance);
    }
    if before.file != after.file {
        fields.push(ChangedField::File);
    }
    fields
}

/// Compares two independently analysed graphs.
///
/// `before` and `after` are two separate analyses — typically two checked-out
/// branches. Neither graph's ids are used; correspondence is by identity
/// throughout.
///
/// The result is deterministic: groups are ordered by edge identity, and edges
/// within a group by their own facts, so the same pair of graphs always yields
/// the same sequence.
#[must_use]
pub fn diff(before: &ArchitectureGraph, after: &ArchitectureGraph) -> GraphDiff {
    let mut left = group(before);
    let right = group(after);

    let mut added = Vec::new();
    let mut removed = Vec::new();
    let mut changed = Vec::new();
    let mut unchanged = 0usize;

    for (identity, mut after_facts) in right {
        let Some(mut before_facts) = left.remove(&identity) else {
            // Nothing on the left asserts this relationship at all.
            added.extend(after_facts.into_iter().map(|facts| EdgeAppearance {
                identity: identity.clone(),
                facts,
            }));
            continue;
        };

        // Identical facts pair off first, so an unchanged edge is never
        // mistaken for a change against some other parallel edge.
        let mut kept_before = Vec::new();
        for facts in before_facts.drain(..) {
            if let Some(position) = after_facts
                .iter()
                .position(|other| other.comparable() == facts.comparable())
            {
                after_facts.remove(position);
                unchanged += 1;
            } else {
                kept_before.push(facts);
            }
        }

        // Whatever remains on both sides asserts the same relationship with
        // different evidence: paired in sorted order, so the pairing is stable.
        let paired = kept_before.len().min(after_facts.len());
        for (before_facts, after_facts) in kept_before
            .iter()
            .take(paired)
            .zip(after_facts.iter().take(paired))
        {
            changed.push(ChangedEdge {
                identity: identity.clone(),
                before: before_facts.clone(),
                after: after_facts.clone(),
                fields: changed_fields(before_facts, after_facts),
            });
        }

        for facts in kept_before.into_iter().skip(paired) {
            removed.push(EdgeAppearance {
                identity: identity.clone(),
                facts,
            });
        }
        for facts in after_facts.into_iter().skip(paired) {
            added.push(EdgeAppearance {
                identity: identity.clone(),
                facts,
            });
        }
    }

    // Anything left on the left was never seen on the right.
    for (identity, facts_list) in left {
        removed.extend(facts_list.into_iter().map(|facts| EdgeAppearance {
            identity: identity.clone(),
            facts,
        }));
    }

    added.sort_by(|a, b| {
        (&a.identity, a.facts.ordering_key()).cmp(&(&b.identity, b.facts.ordering_key()))
    });
    removed.sort_by(|a, b| {
        (&a.identity, a.facts.ordering_key()).cmp(&(&b.identity, b.facts.ordering_key()))
    });
    changed.sort_by(|a, b| {
        (&a.identity, a.before.ordering_key()).cmp(&(&b.identity, b.before.ordering_key()))
    });

    GraphDiff {
        added,
        removed,
        changed,
        unchanged,
    }
}
