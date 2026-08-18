//! Nodes: the artefacts an architecture is made of.

use serde::{Deserialize, Serialize};

use crate::{error::CoreError, id::NodeId, location::SourceLocation};

/// What kind of artefact a node represents.
///
/// The set is fixed by the frozen specification. It spans three domains
/// deliberately — code (`Module` through `Variable`), the HTTP boundary
/// (`Route`, `ExternalService`) and data (`Table`, `Column`) — because the
/// product's differentiating output is a path that crosses all three.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
#[non_exhaustive]
pub enum NodeKind {
    /// The repository being analysed.
    Repository,
    /// A published or declared package within the repository.
    Package,
    /// A directory.
    Directory,
    /// A source file.
    File,
    /// A module or namespace.
    Module,
    /// A class.
    Class,
    /// A free function.
    Function,
    /// A method bound to a class.
    Method,
    /// A variable or constant binding.
    Variable,
    /// An HTTP route, in canonical form.
    Route,
    /// A database table.
    Table,
    /// A database column.
    Column,
    /// A service outside the repository, reached over the network.
    ExternalService,
    /// An environment variable *name*.
    ///
    /// Only the name is ever recorded. Values are never read, stored, or
    /// logged (RULE 015).
    EnvVar,
}

/// An artefact in the architecture graph.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Node {
    id: NodeId,
    kind: NodeKind,
    name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    location: Option<SourceLocation>,
}

impl Node {
    /// Constructs a node.
    ///
    /// `location` is optional because some node kinds genuinely have none — a
    /// `Repository` does not sit at a line, and a `Table` discovered through an
    /// ORM model is named by the model, not by a position in a `.sql` file.
    /// Absent means unknown, and unknown stays unknown (RULE 009).
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::Empty`] if `name` is empty or only whitespace.
    pub fn new(
        id: NodeId,
        kind: NodeKind,
        name: impl Into<String>,
        location: Option<SourceLocation>,
    ) -> Result<Self, CoreError> {
        let name = name.into();
        if name.trim().is_empty() {
            return Err(CoreError::Empty { field: "node name" });
        }
        Ok(Self {
            id,
            kind,
            name,
            location,
        })
    }

    /// The node's handle.
    #[must_use]
    pub fn id(&self) -> NodeId {
        self.id
    }

    /// What the node represents.
    #[must_use]
    pub fn kind(&self) -> NodeKind {
        self.kind
    }

    /// The node's name, as written in the source.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Where the node was found, if that is known.
    #[must_use]
    pub fn location(&self) -> Option<&SourceLocation> {
        self.location.as_ref()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn carries_kind_name_and_location() {
        let location = SourceLocation::new("api/orders.py", 12).unwrap();
        let node = Node::new(
            NodeId::from_raw(1),
            NodeKind::Function,
            "create_order",
            Some(location.clone()),
        )
        .unwrap();

        assert_eq!(node.kind(), NodeKind::Function);
        assert_eq!(node.name(), "create_order");
        assert_eq!(node.location(), Some(&location));
    }

    #[test]
    fn allows_nodes_that_have_no_position() {
        let node = Node::new(NodeId::from_raw(0), NodeKind::Repository, "ecommerce", None).unwrap();
        assert_eq!(node.location(), None);
    }

    #[test]
    fn rejects_an_unnamed_node() {
        assert!(Node::new(NodeId::from_raw(1), NodeKind::File, "  ", None).is_err());
    }

    #[test]
    fn serialises_kinds_in_kebab_case() {
        let json = serde_json::to_value(NodeKind::ExternalService).unwrap();
        assert_eq!(json, serde_json::json!("external-service"));
    }
}
