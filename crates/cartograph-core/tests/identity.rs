//! Stable cross-run node identity, against ADR-0014's five-point gate.
//!
//! These are unit-level: they pin the identity function's contract on nodes
//! built directly. The gate's point 2 — that two *independent analyses of a
//! real repository* agree — needs the whole pipeline and therefore lives in
//! `crates/cartograph-pipeline/tests/identity_real_repository.rs`, where the
//! analyser is available.

use cartograph_core::{Node, NodeId, NodeKind, SourceLocation, identity_of, identity_parts};

fn at(file: &str, line: u32) -> SourceLocation {
    SourceLocation::new(file, line).expect("valid location")
}

fn node(id: u64, kind: NodeKind, name: &str, location: Option<SourceLocation>) -> Node {
    Node::new(NodeId::from_raw(id), kind, name, location).expect("valid node")
}

// ── Gate 1: content and location semantics only ─────────────────────────

#[test]
fn identity_is_a_pure_function_of_kind_name_and_file() {
    let a = node(
        0,
        NodeKind::Function,
        "handler",
        Some(at("api/routes.py", 10)),
    );
    let b = node(
        0,
        NodeKind::Function,
        "handler",
        Some(at("api/routes.py", 10)),
    );

    assert_eq!(identity_of(&a), identity_of(&b));
}

/// The property the whole milestone rests on: a handle must not leak in.
#[test]
fn a_graph_local_node_id_cannot_affect_identity() {
    let first = node(0, NodeKind::Class, "Order", Some(at("models.py", 3)));
    let later = node(
        9_999_999,
        NodeKind::Class,
        "Order",
        Some(at("models.py", 3)),
    );

    assert_eq!(identity_of(&first), identity_of(&later));
}

#[test]
fn kind_distinguishes_artefacts_that_share_a_name_and_file() {
    let class = node(0, NodeKind::Class, "Order", Some(at("models.py", 3)));
    let function = node(1, NodeKind::Function, "Order", Some(at("models.py", 3)));

    assert_ne!(identity_of(&class), identity_of(&function));
}

#[test]
fn the_same_name_in_two_files_is_two_artefacts() {
    let user = node(
        0,
        NodeKind::Class,
        "Connection",
        Some(at("models/user.py", 3)),
    );
    let order = node(
        1,
        NodeKind::Class,
        "Connection",
        Some(at("models/order.py", 3)),
    );

    assert_ne!(identity_of(&user), identity_of(&order));
}

// ── Gate 3: an unrelated edit must not renumber unaffected nodes ─────────

/// An import added at the top of a file pushes everything below it down. If
/// the line took part in identity, that single edit would rename every
/// artefact in the file and a diff would report the whole file as churn.
#[test]
fn an_edit_above_an_artefact_leaves_its_identity_alone() {
    let before = node(
        0,
        NodeKind::Function,
        "list_orders",
        Some(at("api/routes.py", 12)),
    );
    let after = node(
        0,
        NodeKind::Function,
        "list_orders",
        Some(at("api/routes.py", 47)),
    );

    assert_eq!(identity_of(&before), identity_of(&after));
}

#[test]
fn identity_ignores_the_column_as_well_as_the_line() {
    let plain = Node::new(
        NodeId::from_raw(0),
        NodeKind::Function,
        "handler",
        Some(SourceLocation::new("a.py", 1).expect("location")),
    )
    .expect("node");
    let with_column = Node::new(
        NodeId::from_raw(1),
        NodeKind::Function,
        "handler",
        Some(
            SourceLocation::new("a.py", 80)
                .expect("location")
                .with_column(14)
                .expect("column"),
        ),
    )
    .expect("node");

    assert_eq!(identity_of(&plain), identity_of(&with_column));
}

// ── Gate 4: move and rename have defined, tested answers ────────────────

/// Move — matched. Same file, different line, same artefact.
#[test]
fn a_move_within_a_file_is_the_same_artefact() {
    let before = node(0, NodeKind::Method, "save", Some(at("models/order.py", 41)));
    let after = node(
        7,
        NodeKind::Method,
        "save",
        Some(at("models/order.py", 118)),
    );

    assert_eq!(
        identity_of(&before),
        identity_of(&after),
        "moving a method down its own file must not change what it is"
    );
}

/// Rename — deliberately remove-plus-add, not a fuzzy match.
#[test]
fn a_rename_is_a_different_artefact() {
    let before = node(
        0,
        NodeKind::Function,
        "list_orders",
        Some(at("api/routes.py", 12)),
    );
    let after = node(
        0,
        NodeKind::Function,
        "list_all_orders",
        Some(at("api/routes.py", 12)),
    );

    assert_ne!(
        identity_of(&before),
        identity_of(&after),
        "a rename is reported as a removal and an addition, by policy"
    );
}

/// A file move is also remove-plus-add: the path is part of what the artefact
/// is, and matching across paths would need the same guessing rename does.
#[test]
fn moving_an_artefact_to_another_file_is_a_different_artefact() {
    let before = node(0, NodeKind::Class, "Order", Some(at("models.py", 3)));
    let after = node(0, NodeKind::Class, "Order", Some(at("models/order.py", 3)));

    assert_ne!(identity_of(&before), identity_of(&after));
}

// ── Encoding: injective, and unambiguous about absent locations ─────────

#[test]
fn a_node_without_a_location_has_an_identity() {
    let table = node(0, NodeKind::Table, "orders", None);
    let repository = node(1, NodeKind::Repository, "orders", None);

    assert_ne!(identity_of(&table), identity_of(&repository));
    assert!(!identity_of(&table).as_str().is_empty());
}

/// "No location" must not be expressible as some file path, or an ORM table
/// could collide with a node in a file that happens to be named that way.
#[test]
fn an_absent_location_cannot_collide_with_a_present_one() {
    let absent = identity_parts(NodeKind::Table, "orders", None);
    for candidate in ["", "-", "n", "none", "null", "<none>"] {
        assert_ne!(
            absent,
            identity_parts(NodeKind::Table, "orders", Some(candidate)),
            "absent collided with the file {candidate:?}"
        );
    }
}

/// Length prefixing exists for this: a separator alone would let two different
/// (name, file) pairs encode to one string.
#[test]
fn the_encoding_is_injective_across_separator_characters() {
    let pairs = [
        ("a|b", "c.py"),
        ("a", "b|c.py"),
        ("a%7Cb", "c.py"),
        ("a%b", "c.py"),
        ("a%25b", "c.py"),
        ("a", "b%7Cc.py"),
        ("a|b|c", "d.py"),
        ("a", "b|c|d.py"),
    ];

    let mut seen: Vec<(String, (&str, &str))> = Vec::new();
    for (name, file) in pairs {
        let encoded = identity_parts(NodeKind::Function, name, Some(file)).into_string();
        if let Some((_, other)) = seen.iter().find(|(e, _)| *e == encoded) {
            panic!("{name:?}/{file:?} collided with {other:?}");
        }
        seen.push((encoded, (name, file)));
    }
}

#[test]
fn every_node_kind_has_a_distinct_token() {
    use cartograph_core::kind_token;

    let kinds = [
        NodeKind::Repository,
        NodeKind::Package,
        NodeKind::Directory,
        NodeKind::File,
        NodeKind::Module,
        NodeKind::Class,
        NodeKind::Function,
        NodeKind::Method,
        NodeKind::Variable,
        NodeKind::Route,
        NodeKind::Table,
        NodeKind::Column,
        NodeKind::ExternalService,
        NodeKind::EnvVar,
    ];

    let mut tokens: Vec<&str> = kinds.iter().copied().map(kind_token).collect();
    let count = tokens.len();
    tokens.sort_unstable();
    tokens.dedup();
    assert_eq!(tokens.len(), count, "two kinds share a token");
}

/// The tokens are a wire-visible contract once a diff quotes them, so changing
/// one silently changes every identity in every repository.
#[test]
fn the_kind_tokens_are_the_recorded_ones() {
    use cartograph_core::kind_token;

    assert_eq!(kind_token(NodeKind::Class), "class");
    assert_eq!(kind_token(NodeKind::Function), "function");
    assert_eq!(kind_token(NodeKind::Method), "method");
    assert_eq!(kind_token(NodeKind::Table), "table");
    assert_eq!(kind_token(NodeKind::ExternalService), "external-service");
    assert_eq!(kind_token(NodeKind::EnvVar), "env-var");
}

// ── Determinism independent of ordering ─────────────────────────────────

#[test]
fn identity_does_not_depend_on_the_order_nodes_are_visited() {
    let nodes = [
        node(0, NodeKind::Class, "Order", Some(at("models.py", 3))),
        node(1, NodeKind::Function, "handler", Some(at("api.py", 9))),
        node(2, NodeKind::Table, "orders", None),
    ];

    let forward: Vec<_> = nodes.iter().map(identity_of).collect();
    let mut backward: Vec<_> = nodes.iter().rev().map(identity_of).collect();
    backward.reverse();

    assert_eq!(forward, backward);
}

#[test]
fn repeated_computation_is_stable() {
    let n = node(
        3,
        NodeKind::Route,
        "/api/orders",
        Some(at("api/routes.py", 12)),
    );
    let once = identity_of(&n);

    for _ in 0..64 {
        assert_eq!(identity_of(&n), once);
    }
}

/// Identities sort totally and by content, which is what a diff will rely on
/// to emit a deterministic order.
#[test]
fn identities_order_independently_of_construction_order() {
    let a = identity_parts(NodeKind::Class, "Alpha", Some("a.py"));
    let b = identity_parts(NodeKind::Class, "Beta", Some("a.py"));

    let mut one = vec![b.clone(), a.clone()];
    let mut two = vec![a.clone(), b.clone()];
    one.sort();
    two.sort();

    assert_eq!(one, two);
    assert_eq!(one, vec![a, b]);
}
