//! The desktop blast radius: a projection of the core, and a stale guard.
//!
//! Two properties. First, that this surface **adds nothing** — every node,
//! depth and confidence it reports is what `blast_radius` returned, asserted
//! by comparing against the core directly rather than against a copy of its
//! expected output. Second, that a query cannot be answered against the wrong
//! graph, which for a node id is not a theoretical risk: ids restart at zero
//! per analysis, so a stale one usually *exists*.

#![allow(
    clippy::float_cmp,
    reason = "no arithmetic is performed on confidence; see ADR-0018"
)]

use cartograph_core::{
    Confidence, EdgeKind, Evidence, NodeId, NodeKind, Provenance, SourceLocation,
};
use cartograph_desktop::blast;
use cartograph_desktop::error::DesktopErrorKind;
use cartograph_desktop::evidence::{AnalysisId, AnalysisSession};
use cartograph_graph::blast::blast_radius;
use cartograph_graph::{ArchitectureGraph, EdgeSpec};
use cartograph_testkit::fixed_clock;

/// A chain: `caller -> handler -> Model`, the shape the feature is about.
fn chain() -> (ArchitectureGraph, Vec<NodeId>) {
    let mut graph = ArchitectureGraph::with_clock(fixed_clock);
    let names = [
        (NodeKind::Function, "caller", "web/app.ts"),
        (NodeKind::Function, "handler", "api/routes.py"),
        (NodeKind::Class, "Model", "api/models.py"),
    ];
    let ids: Vec<NodeId> = names
        .iter()
        .enumerate()
        .map(|(i, (kind, name, file))| {
            graph
                .add_node(
                    *kind,
                    *name,
                    Some(
                        SourceLocation::new(*file, u32::try_from(i).unwrap_or(0) + 1)
                            .expect("valid"),
                    ),
                )
                .expect("valid node")
        })
        .collect();

    for (from, to, kind, confidence) in [
        (0, 1, EdgeKind::HttpCall, 0.98_f32),
        (1, 2, EdgeKind::OrmAccess, 0.80_f32),
    ] {
        graph
            .add_edge(EdgeSpec {
                source: ids[from],
                target: ids[to],
                kind,
                confidence: Confidence::new(confidence).expect("valid"),
                provenance: Provenance::RouteMatcher,
                evidence: Evidence::new("test edge").expect("non-empty"),
                location: SourceLocation::new("api/routes.py", 1).expect("valid"),
                commit: None,
            })
            .expect("endpoints exist");
    }
    (graph, ids)
}

// ── A projection, not a second implementation ───────────────────────────

/// Compared against the core's own answer, so this cannot pass by agreeing
/// with a hard-coded expectation that has drifted.
#[test]
fn the_payload_is_exactly_what_the_core_returned() {
    let (graph, ids) = chain();
    let id = AnalysisId::first();
    let expected = blast_radius(&graph, ids[2]);
    let session = AnalysisSession::new(id, graph);

    let payload = blast::radius(&session, id, ids[2]).expect("the node is present");

    assert_eq!(payload.target, expected.target);
    assert_eq!(payload.len(), expected.len());
    for (got, want) in payload.reached.iter().zip(&expected.reached) {
        assert_eq!(got.node, want.node);
        assert_eq!(got.depth, want.depth);
        assert_eq!(got.confidence, want.confidence, "confidence was altered");
        assert_eq!(got.via, want.via);
    }
}

#[test]
fn confidence_is_the_weakest_link_and_is_reported_uncalibrated() {
    let (graph, ids) = chain();
    let id = AnalysisId::first();
    let session = AnalysisSession::new(id, graph);

    let payload = blast::radius(&session, id, ids[2]).expect("present");

    // `caller` reaches `Model` through 0.98 then 0.80; the weakest step wins.
    let caller = payload
        .reached
        .iter()
        .find(|e| e.node == ids[0])
        .expect("the caller depends on the model");
    assert_eq!(caller.confidence, 0.80_f32);
    assert_ne!(caller.confidence, 0.98_f32 * 0.80_f32, "a product appeared");
    assert_eq!(caller.depth, 2);
    assert!(!payload.calibrated);
    assert_eq!(payload.routes, "representative");
}

#[test]
fn the_target_is_absent_from_its_own_impact_set() {
    let (graph, ids) = chain();
    let id = AnalysisId::first();
    let session = AnalysisSession::new(id, graph);

    let payload = blast::radius(&session, id, ids[2]).expect("present");

    assert!(payload.reached.iter().all(|e| e.node != ids[2]));
    assert_eq!(payload.target, ids[2]);
}

/// An empty impact set is a successful answer, not an error.
#[test]
fn nothing_depending_on_the_target_is_an_empty_success() {
    let (graph, ids) = chain();
    let id = AnalysisId::first();
    let session = AnalysisSession::new(id, graph);

    // `caller` is the outermost node: nothing depends on it.
    let payload = blast::radius(&session, id, ids[0]).expect("still a success");

    assert!(payload.is_empty());
    assert_eq!(payload.target, ids[0]);
}

#[test]
fn every_via_names_an_edge_leaving_the_reached_node() {
    let (graph, ids) = chain();
    let id = AnalysisId::first();
    let session = AnalysisSession::new(id, graph);
    let payload = blast::radius(&session, id, ids[2]).expect("present");

    for entry in &payload.reached {
        let edge = session.graph().edge(entry.via).expect("via is a real edge");
        assert_eq!(edge.source(), entry.node);
        assert!(!edge.evidence().as_str().is_empty(), "RULE 006");
    }
}

// ── Staleness ───────────────────────────────────────────────────────────

/// The case that matters: node ids restart per analysis, so a stale id
/// normally still exists — and answering it would highlight the wrong set.
#[test]
fn a_stale_selection_is_refused_even_though_the_id_exists() {
    let (first_graph, ids) = chain();
    let first_id = AnalysisId::first();
    let target = ids[2];

    // A second analysis whose ids overlap but which describes something else.
    let mut second = ArchitectureGraph::with_clock(fixed_clock);
    for name in ["other_a", "other_b", "other_c"] {
        second
            .add_node(
                NodeKind::Function,
                name,
                Some(SourceLocation::new("other/x.py", 1).expect("valid")),
            )
            .expect("valid");
    }
    assert!(
        second.node(target).is_some(),
        "precondition: the stale id must exist in the new graph, or this proves nothing"
    );
    let _ = first_graph;

    let second_id = first_id.next();
    let session = AnalysisSession::new(second_id, second);

    let refused = blast::radius(&session, first_id, target)
        .expect_err("a selection from the previous analysis must not be answered");

    assert_eq!(refused.kind, DesktopErrorKind::StaleSelection);
    assert!(refused.hint.is_some());
}

#[test]
fn a_node_absent_from_the_graph_is_refused_rather_than_answered_empty() {
    let (graph, _) = chain();
    let id = AnalysisId::first();
    let session = AnalysisSession::new(id, graph);

    let refused = blast::radius(&session, id, NodeId::from_raw(9_999))
        .expect_err("an absent node is a disagreement, not an empty answer");

    assert_eq!(refused.kind, DesktopErrorKind::UnknownNode);
}

// ── The wire contract ───────────────────────────────────────────────────

#[test]
fn the_payload_serialises_under_the_agreed_names() {
    let (graph, ids) = chain();
    let id = AnalysisId::first();
    let session = AnalysisSession::new(id, graph);
    let payload = blast::radius(&session, id, ids[2]).expect("present");

    let json = serde_json::to_value(&payload).expect("serialises");
    let mut keys: Vec<&str> = json
        .as_object()
        .expect("object")
        .keys()
        .map(String::as_str)
        .collect();
    keys.sort_unstable();
    assert_eq!(
        keys,
        vec!["analysis", "calibrated", "reached", "routes", "target"]
    );

    let entry = &json["reached"][0];
    let mut keys: Vec<&str> = entry
        .as_object()
        .expect("object")
        .keys()
        .map(String::as_str)
        .collect();
    keys.sort_unstable();
    // The same names `cartograph blast --json` uses: one contract, two
    // renderings.
    assert_eq!(keys, vec!["confidence", "depth", "node", "via"]);
}

#[test]
fn error_kinds_serialise_under_their_agreed_names() {
    assert_eq!(
        serde_json::to_string(&DesktopErrorKind::UnknownNode).expect("serialises"),
        "\"unknownNode\""
    );
}

/// The payload is serialise-only by design — it travels to the window and
/// never back — so the property worth pinning is that it serialises to valid,
/// complete JSON rather than that it round-trips.
#[test]
fn the_payload_serialises_to_complete_json() {
    let (graph, ids) = chain();
    let id = AnalysisId::first();
    let session = AnalysisSession::new(id, graph);
    let payload = blast::radius(&session, id, ids[2]).expect("present");

    let text = serde_json::to_string(&payload).expect("serialises");
    let parsed: serde_json::Value = serde_json::from_str(&text).expect("valid JSON");

    assert_eq!(
        parsed["reached"].as_array().expect("array").len(),
        payload.len()
    );
    assert_eq!(parsed["calibrated"], false);
    assert_eq!(parsed["routes"], "representative");
    for entry in parsed["reached"].as_array().expect("array") {
        assert!(entry["depth"].as_u64().expect("depth") >= 1);
        let confidence = entry["confidence"].as_f64().expect("confidence");
        assert!((0.0..=1.0).contains(&confidence));
    }
}

// ── A real repository ───────────────────────────────────────────────────

/// The desktop answer must equal the core's on a real analysis, for **every**
/// node — not a sample, because a projection that dropped one entry in a
/// thousand would pass a spot check.
#[test]
fn on_a_real_repository_the_desktop_answer_equals_the_core() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("the repository root");
    let repository =
        cartograph_desktop::repository::validate(&root).expect("this repository validates");
    let id = AnalysisId::first();
    let (payload, session) =
        cartograph_desktop::session::analyze_as(&repository, id).expect("it analyses");
    assert!(
        payload.scene.node_count() > 0,
        "precondition: the analysis must produce nodes"
    );

    for drawn in &payload.scene.nodes {
        let desktop = blast::radius(&session, id, drawn.id).expect("every drawn node answers");
        let core = blast_radius(session.graph(), drawn.id);

        assert_eq!(desktop.len(), core.len(), "count differed for {}", drawn.id);
        for (got, want) in desktop.reached.iter().zip(&core.reached) {
            assert_eq!(got.node, want.node);
            assert_eq!(got.depth, want.depth);
            assert_eq!(got.confidence, want.confidence);
            assert_eq!(got.via, want.via);
        }
        assert!(desktop.reached.iter().all(|e| e.node != drawn.id));
    }
}

/// The pinned corpora, read-only and opt-in.
///
/// Reports the desktop answer beside the core's for named targets, so the
/// numbers in the slice report are observed rather than asserted from memory.
#[test]
#[ignore = "needs CARTOGRAPH_BLAST_REPO; the pinned corpora are not vendored"]
fn on_a_pinned_corpus_the_desktop_answer_equals_the_core() {
    let Ok(path) = std::env::var("CARTOGRAPH_BLAST_REPO") else {
        return;
    };
    let repository = cartograph_desktop::repository::validate(std::path::Path::new(&path))
        .expect("the corpus validates");
    let id = AnalysisId::first();
    let (payload, session) =
        cartograph_desktop::session::analyze_as(&repository, id).expect("it analyses");

    let wanted: Vec<String> = std::env::var("CARTOGRAPH_BLAST_TARGETS")
        .unwrap_or_default()
        .split(',')
        .filter(|s| !s.is_empty())
        .map(str::to_owned)
        .collect();

    println!(
        "corpus: {} nodes, {} edges",
        payload.summary.nodes, payload.summary.edges
    );

    let mut checked = 0usize;
    for drawn in &payload.scene.nodes {
        if !wanted.is_empty() && !wanted.iter().any(|w| w == &drawn.label) {
            continue;
        }
        let desktop = blast::radius(&session, id, drawn.id).expect("answers");
        let core = blast_radius(session.graph(), drawn.id);
        assert_eq!(desktop.len(), core.len(), "desktop disagreed with the core");
        for (got, want) in desktop.reached.iter().zip(&core.reached) {
            assert_eq!(got.node, want.node);
            assert_eq!(got.depth, want.depth);
            assert_eq!(got.confidence, want.confidence);
        }

        let ts = desktop
            .reached
            .iter()
            .filter(|e| {
                session
                    .graph()
                    .node(e.node)
                    .and_then(|n| n.location())
                    .is_some_and(|l| {
                        matches!(
                            std::path::Path::new(l.file())
                                .extension()
                                .and_then(std::ffi::OsStr::to_str),
                            Some("ts" | "tsx")
                        )
                    })
            })
            .count();
        let depth = desktop.reached.iter().map(|e| e.depth).max().unwrap_or(0);
        println!(
            "  {:<28} reached={:<5} TS={:<4} maxdepth={}  (core={})",
            drawn.label,
            desktop.len(),
            ts,
            depth,
            core.len()
        );
        checked += 1;
    }
    assert!(
        checked > 0,
        "no target matched; check CARTOGRAPH_BLAST_TARGETS"
    );
}
