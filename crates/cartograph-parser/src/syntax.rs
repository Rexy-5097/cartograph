//! Tree-sitter helpers shared by the language extractors.
//!
//! Private: this module names tree-sitter types, and nothing here is
//! re-exported. It exists so the TypeScript and Python extractors compute
//! spans and read node text identically — a span that means different things
//! in two languages would quietly corrupt every downstream location.

use tree_sitter::Node;

use crate::model::Span;

/// Converts a node's extent to a 1-based [`Span`].
pub(crate) fn span_of(node: Node<'_>) -> Span {
    let start = node.start_position();
    let end = node.end_position();
    #[allow(clippy::cast_possible_truncation)] // >4G-line files are not a real input
    Span {
        start_line: start.row as u32 + 1,
        start_column: start.column as u32 + 1,
        end_line: end.row as u32 + 1,
        end_column: end.column as u32 + 1,
    }
}

/// The node's source text, or empty when it is not valid UTF-8.
pub(crate) fn text_of(node: Node<'_>, source: &str) -> String {
    node.utf8_text(source.as_bytes())
        .unwrap_or_default()
        .to_owned()
}

/// Whether the node has a bare (unnamed) keyword token child with this text.
pub(crate) fn has_keyword_child(node: Node<'_>, keyword: &str) -> bool {
    let mut cursor = node.walk();
    node.children(&mut cursor)
        .any(|c| !c.is_named() && c.kind() == keyword)
}
