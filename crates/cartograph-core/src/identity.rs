//! Stable semantic identity for a node, usable across independent analyses.
//!
//! # Why this exists, and what it is not
//!
//! [`NodeId`](crate::NodeId) is a *handle*: allocated per graph from zero, it
//! names different artefacts in two different graphs. Anything comparing two
//! analyses — a branch-vs-branch diff, a layout that preserves a reader's
//! mental map — needs the opposite: a value that names the same artefact in
//! both. This module supplies that value. It does not change `NodeId`, which
//! stays exactly what it was.
//!
//! Nothing here derives from insertion order, allocation order, traversal
//! order, hash-map iteration order, or any `NodeId`. Identity is a function of
//! the node's own semantics and nothing else, which is what makes two
//! independent analyses of one commit agree (ADR-0014, gate points 1 and 2).
//!
//! # The identity is `(kind, name, file)`
//!
//! Three fields, and the reason each is present:
//!
//! - **`kind`** — a class and a function may share a name in one file, and
//!   they are not the same artefact. Encoded through an explicit, exhaustive
//!   [`kind_token`] rather than `Debug`, so renaming a variant cannot silently
//!   change every identity in the repository.
//! - **`name`** — what the artefact is called. A rename is therefore a
//!   different identity, which is the deliberate policy below.
//! - **`file`** — the repository-relative path, already canonical and never
//!   absolute (`SourceLocation` enforces both). A `Connection` in
//!   `models/user.py` is not the one in `models/order.py`.
//!
//! **The line number is deliberately excluded.** Including it would make an
//! edit anywhere above an artefact rename it, so every function below a new
//! import would read as removed-plus-added. That is the "move" case of
//! ADR-0014 gate point 4, answered by construction rather than by a heuristic.
//!
//! A node may legitimately have no location — a `Repository` does not sit at a
//! line, and a `Table` discovered through an ORM model is named by the model.
//! Absent is encoded as its own tagged case, distinct from any present path, so
//! "unknown" cannot collide with a file named like a placeholder (RULE 009).
//!
//! # Why three fields are sufficient, and what that rests on
//!
//! `ArchitectureGraph::node_for` — the only constructor the analysis pipeline
//! uses — already deduplicates on exactly `(kind, name, file)`. Two artefacts
//! agreeing on all three are not two nodes that this module must tell apart;
//! they are **one node**, merged when the graph was built. So this key is
//! collision-free within a graph by construction, not by luck.
//!
//! The consequence is worth stating plainly rather than leaving implied:
//! **two methods of the same name in different classes of one file are already
//! a single node before identity is computed**, as are nested definitions
//! sharing a name with an enclosing scope. Identity cannot separate what graph
//! construction merged. Distinguishing them would mean giving nodes a scope or
//! signature and changing `node_for`, which would move M10's differential
//! oracle and M12's blast results — a graph-construction change, not an
//! identity one, and out of scope here.
//!
//! # Rename is remove-plus-add, deliberately
//!
//! A renamed artefact takes a new identity, so a diff reports it as one
//! removal and one addition. Matching renames means guessing from similarity,
//! and a wrong guess produces a confidently wrong diff — it claims one artefact
//! evolved into another when the author split, merged or replaced it.
//! Remove-plus-add is honest about what is known. ADR-0014 gate point 4 asks
//! for a defined answer either way; this is that answer.
//!
//! # Encoding
//!
//! The canonical form is defined first, and hashing is then not applied at all.
//! A `u64` fingerprint would buy fixed width and cost collisions and
//! debuggability, and nothing here needs fixed width: identities are compared,
//! ordered, and used as map keys, all of which a string does exactly.
//!
//! Four `|`-separated fields — kind, name, whether a file is known, and the
//! file — with `%` and `|` percent-escaped inside each. The escaping is what
//! makes the encoding injective: a plain separator is not, since a file named
//! `a|b` and a name containing `|` could otherwise produce one string from two
//! different inputs.
//!
//! Escaping rather than length-prefixing is deliberate, and a test caught the
//! difference. Length prefixes compare before the value they describe, so
//! `4:Beta` sorts before `5:Alpha` — deterministic, but ordered by *name
//! length*, which reads as arbitrary in a diff a person is scanning. Escaped
//! fields sort by kind, then name, then file, which is the order a reader
//! expects, and only the rare artefact containing `%` or `|` is affected at
//! all.
//!
//! The presence flag sits before the path so a node with no location orders
//! consistently against one that has a path, without either being expressible
//! as the other.

use std::fmt;

use crate::location::SourceLocation;
use crate::node::{Node, NodeKind};

/// A node's identity, stable across independent analyses of the same source.
///
/// Two nodes in two separately built graphs are the same artefact exactly when
/// their identities are equal. Ordering is the byte order of the canonical
/// encoding: total, and independent of how either graph was built, which is
/// what lets a comparison be emitted in a deterministic order.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct NodeIdentity(String);

impl NodeIdentity {
    /// The canonical encoding, for tests, diagnostics and map keys.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Consumes the identity, yielding its canonical encoding.
    #[must_use]
    pub fn into_string(self) -> String {
        self.0
    }
}

impl fmt::Display for NodeIdentity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// The stable token for a node kind.
///
/// Written out rather than derived from `Debug` so that renaming a variant is
/// a compile error here instead of a silent, repository-wide identity change.
/// The match is exhaustive with no wildcard arm: adding a `NodeKind` will not
/// compile until someone chooses its token.
#[must_use]
pub fn kind_token(kind: NodeKind) -> &'static str {
    match kind {
        NodeKind::Repository => "repository",
        NodeKind::Package => "package",
        NodeKind::Directory => "directory",
        NodeKind::File => "file",
        NodeKind::Module => "module",
        NodeKind::Class => "class",
        NodeKind::Function => "function",
        NodeKind::Method => "method",
        NodeKind::Variable => "variable",
        NodeKind::Route => "route",
        NodeKind::Table => "table",
        NodeKind::Column => "column",
        NodeKind::ExternalService => "external-service",
        NodeKind::EnvVar => "env-var",
    }
}

/// Appends one field, escaping the separator so the encoding stays injective.
///
/// `%` first, then `|` — escaping the escape character before the character it
/// escapes, or `a%7Cb` and a literal `a|b` would decode to the same thing.
fn push_field(out: &mut String, value: &str) {
    for ch in value.chars() {
        match ch {
            '%' => out.push_str("%25"),
            '|' => out.push_str("%7C"),
            other => out.push(other),
        }
    }
}

/// The stable identity of a node.
///
/// Depends only on kind, name and file — never on the node's `NodeId`, never on
/// its line, and never on anything about the graph that holds it.
#[must_use]
pub fn identity_of(node: &Node) -> NodeIdentity {
    identity_parts(
        node.kind(),
        node.name(),
        node.location().map(SourceLocation::file),
    )
}

/// The identity of an artefact described directly, without a [`Node`].
///
/// The same routine `identity_of` uses, exposed so a caller holding the three
/// fields cannot accidentally encode them a second, subtly different way.
#[must_use]
pub fn identity_parts(kind: NodeKind, name: &str, file: Option<&str>) -> NodeIdentity {
    let mut out = String::new();
    push_field(&mut out, kind_token(kind));
    out.push('|');
    push_field(&mut out, name);
    out.push('|');
    // The presence flag is its own field, so a node with no location cannot
    // collide with one whose file is literally the placeholder text: the flag
    // differs before the path is even read.
    match file {
        Some(file) => {
            out.push_str("1|");
            push_field(&mut out, file);
        }
        None => out.push_str("0|"),
    }
    NodeIdentity(out)
}
