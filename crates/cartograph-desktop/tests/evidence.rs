//! Edge evidence: exact fidelity, and refusal to answer a stale selection.
//!
//! Two properties, and the second is the one that matters most.
//!
//! **Fidelity.** Every field is the edge's own. Confidence is compared with
//! `==` rather than a tolerance, because a panel that rounds 0.784 to "78%"
//! and a panel that *computes* 0.78 look identical on screen and are not the
//! same thing.
//!
//! **Refusal.** `EdgeId` is graph-local and restarts at zero per analysis
//! (ADR-0011). A selection held across a re-analysis therefore names an id
//! that almost certainly still exists — as a different relationship. The
//! lookup must refuse it. `a_stale_selection_is_refused_even_though_the_id_exists`
//! is the test that matters: it constructs exactly that situation and proves
//! the answer is an error rather than plausible, well-formed, wrong evidence.

#![allow(clippy::float_cmp)] // exact confidence fidelity is the assertion

use cartograph_core::{
    Confidence, EdgeId, EdgeKind, Evidence, NodeKind, Provenance, SourceLocation,
};
use cartograph_desktop::error::DesktopErrorKind;
use cartograph_desktop::evidence::{AnalysisId, AnalysisSession};
use cartograph_graph::{ArchitectureGraph, EdgeSpec};
use cartograph_testkit::fixed_clock;

/// A graph with one fully specified edge, so every field can be checked.
fn one_edge() -> (ArchitectureGraph, EdgeId) {
    let mut graph = ArchitectureGraph::with_clock(fixed_clock);
    let source = graph
        .add_node(
            NodeKind::Function,
            "createOrder",
            Some(SourceLocation::new("web/lib/api.ts", 42).expect("valid")),
        )
        .expect("valid node");
    let target = graph
        .add_node(
            NodeKind::Route,
            "POST /api/orders",
            Some(SourceLocation::new("api/routes.py", 8).expect("valid")),
        )
        .expect("valid node");
    let edge = graph
        .add_edge(EdgeSpec {
            source,
            target,
            kind: EdgeKind::HttpCall,
            confidence: Confidence::new(0.784).expect("valid"),
            provenance: Provenance::RouteMatcher,
            evidence: Evidence::new("fetch('/api/orders', { method: 'POST' })").expect("non-empty"),
            location: SourceLocation::new("web/lib/api.ts", 42)
                .expect("valid")
                .with_column(11)
                .expect("valid column"),
            commit: None,
        })
        .expect("endpoints exist");
    (graph, edge)
}

// ── Fidelity ────────────────────────────────────────────────────────────

#[test]
fn every_field_is_the_edges_own() {
    let (graph, edge) = one_edge();
    let id = AnalysisId::first();
    let session = AnalysisSession::new(id, graph);

    let record = session.evidence(id, edge).expect("the edge is present");

    assert_eq!(record.analysis, id);
    assert_eq!(record.edge, edge);
    assert_eq!(record.kind, EdgeKind::HttpCall);
    assert_eq!(record.source.label, "createOrder");
    assert_eq!(record.source.kind, NodeKind::Function);
    assert_eq!(record.target.label, "POST /api/orders");
    assert_eq!(record.target.kind, NodeKind::Route);
    assert_eq!(record.provenance, Provenance::RouteMatcher);
    assert_eq!(record.evidence, "fetch('/api/orders', { method: 'POST' })");
    assert_eq!(record.location.file, "web/lib/api.ts");
    assert_eq!(record.location.line, 42);
    assert_eq!(record.location.column, Some(11));
}

/// Exact, not approximate. A rounded value and a recomputed value look the
/// same on screen; only this distinguishes them.
#[test]
fn confidence_is_carried_without_rounding() {
    let (graph, edge) = one_edge();
    let id = AnalysisId::first();
    let session = AnalysisSession::new(id, graph);

    let record = session.evidence(id, edge).expect("present");

    assert_eq!(record.confidence, 0.784_f32);
}

/// The product's whole claim is that it does not present a prior as a
/// probability. That caveat travels with the data rather than living in
/// whichever client happens to remember it.
#[test]
fn confidence_is_reported_as_uncalibrated() {
    let (graph, edge) = one_edge();
    let id = AnalysisId::first();
    let session = AnalysisSession::new(id, graph);

    let record = session.evidence(id, edge).expect("present");

    assert!(
        !record.calibrated,
        "nothing in Cartograph is calibrated; M08 measured that and it has not changed"
    );
}

#[test]
fn a_route_matched_edge_is_reported_as_deterministic() {
    let (graph, edge) = one_edge();
    let id = AnalysisId::first();
    let session = AnalysisSession::new(id, graph);

    let record = session.evidence(id, edge).expect("present");

    // Read from the edge, not decided by the lookup.
    assert_eq!(
        record.deterministic,
        Provenance::RouteMatcher.is_deterministic()
    );
}

/// A column is optional in the model and must stay optional on the wire —
/// inventing one would be a fabricated source position.
#[test]
fn a_missing_column_stays_missing() {
    let mut graph = ArchitectureGraph::with_clock(fixed_clock);
    let a = graph
        .add_node(
            NodeKind::Class,
            "Order",
            Some(SourceLocation::new("api/models.py", 5).expect("valid")),
        )
        .expect("valid");
    let b = graph
        .add_node(NodeKind::Table, "orders", None)
        .expect("valid");
    let edge = graph
        .add_edge(EdgeSpec {
            source: a,
            target: b,
            kind: EdgeKind::Queries,
            confidence: Confidence::new(0.9).expect("valid"),
            provenance: Provenance::OrmResolution,
            evidence: Evidence::new("Order.__tablename__ = 'orders'").expect("non-empty"),
            location: SourceLocation::new("api/models.py", 5).expect("valid"),
            commit: None,
        })
        .expect("endpoints exist");
    let id = AnalysisId::first();
    let session = AnalysisSession::new(id, graph);

    let record = session.evidence(id, edge).expect("present");

    assert_eq!(record.location.column, None);
    assert_eq!(record.target.label, "orders");
    // A table has no source location of its own; the *edge* still does.
    assert_eq!(record.location.file, "api/models.py");
}

/// Repository-relative, always. `SourceLocation` rejects absolute paths at
/// construction, and this pins that the lookup does not reintroduce one.
#[test]
fn the_location_is_repository_relative() {
    let (graph, edge) = one_edge();
    let id = AnalysisId::first();
    let session = AnalysisSession::new(id, graph);

    let record = session.evidence(id, edge).expect("present");

    let file = &record.location.file;
    assert!(
        !file.contains(':'),
        "a drive letter reached the panel: {file}"
    );
    assert!(
        !file.starts_with('/') && !file.starts_with('\\'),
        "rooted: {file}"
    );
    let json = serde_json::to_string(&record).expect("serialises");
    assert!(
        !json.contains("C:\\") && !json.contains("/home/"),
        "absolute path in payload"
    );
}

// ── Refusal ─────────────────────────────────────────────────────────────

/// The test this module exists for.
///
/// Two analyses, each with edges numbered from zero. A selection made against
/// the first names an id the second also has — pointing at an entirely
/// different relationship. Answering it would be worse than failing.
#[test]
fn a_stale_selection_is_refused_even_though_the_id_exists() {
    let (first_graph, edge) = one_edge();
    let first_id = AnalysisId::first();

    // A second, different analysis whose edge ids overlap.
    let mut second_graph = ArchitectureGraph::with_clock(fixed_clock);
    let a = second_graph
        .add_node(
            NodeKind::Function,
            "unrelated",
            Some(SourceLocation::new("other/x.py", 1).expect("valid")),
        )
        .expect("valid");
    let b = second_graph
        .add_node(
            NodeKind::Class,
            "Different",
            Some(SourceLocation::new("other/y.py", 2).expect("valid")),
        )
        .expect("valid");
    let overlapping = second_graph
        .add_edge(EdgeSpec {
            source: a,
            target: b,
            kind: EdgeKind::Import,
            confidence: Confidence::new(0.5).expect("valid"),
            provenance: Provenance::StaticImportResolution,
            evidence: Evidence::new("import y").expect("non-empty"),
            location: SourceLocation::new("other/x.py", 1).expect("valid"),
            commit: None,
        })
        .expect("endpoints exist");

    // Precondition: the ids genuinely collide, or this proves nothing.
    assert_eq!(
        edge, overlapping,
        "precondition: both analyses must number this edge identically"
    );
    let _ = first_graph;

    let second_id = first_id.next();
    let session = AnalysisSession::new(second_id, second_graph);

    let refused = session
        .evidence(first_id, edge)
        .expect_err("a selection from the previous analysis must not be answered");

    assert_eq!(refused.kind, DesktopErrorKind::StaleSelection);
    assert!(refused.hint.is_some(), "the user needs to know what to do");
}

#[test]
fn an_unknown_edge_is_refused() {
    let (graph, _) = one_edge();
    let id = AnalysisId::first();
    let session = AnalysisSession::new(id, graph);

    let refused = session
        .evidence(id, EdgeId::from_raw(9_999))
        .expect_err("no such edge");

    assert_eq!(refused.kind, DesktopErrorKind::UnknownEdge);
}

#[test]
fn an_empty_graph_answers_nothing() {
    let id = AnalysisId::first();
    let session = AnalysisSession::new(id, ArchitectureGraph::with_clock(fixed_clock));

    let refused = session
        .evidence(id, EdgeId::from_raw(0))
        .expect_err("an empty graph has no edges");

    assert_eq!(refused.kind, DesktopErrorKind::UnknownEdge);
}

/// Ids advance, so a selection can never accidentally match a later analysis.
#[test]
fn analysis_ids_are_monotonic() {
    let first = AnalysisId::first();
    let second = first.next();
    let third = second.next();

    assert!(first < second && second < third);
    assert_ne!(first, second);
}

// ── The wire contract ───────────────────────────────────────────────────

/// The frontend branches on these names. A rename here is a silent breakage
/// there — a branch that stops being taken, which is the hardest kind to see.
#[test]
fn error_kinds_serialise_under_their_agreed_names() {
    for (kind, expected) in [
        (DesktopErrorKind::StaleSelection, "\"staleSelection\""),
        (DesktopErrorKind::UnknownEdge, "\"unknownEdge\""),
        (DesktopErrorKind::NoAnalysis, "\"noAnalysis\""),
    ] {
        assert_eq!(serde_json::to_string(&kind).expect("serialises"), expected);
    }
}

#[test]
fn the_record_serialises_under_the_agreed_names() {
    let (graph, edge) = one_edge();
    let id = AnalysisId::first();
    let session = AnalysisSession::new(id, graph);
    let record = session.evidence(id, edge).expect("present");

    let json = serde_json::to_value(&record).expect("serialises");
    let object = json.as_object().expect("object");
    let mut keys: Vec<&str> = object.keys().map(String::as_str).collect();
    keys.sort_unstable();
    assert_eq!(
        keys,
        vec![
            "analysis",
            "calibrated",
            "confidence",
            "deterministic",
            "edge",
            "evidence",
            "kind",
            "location",
            "provenance",
            "source",
            "target",
        ]
    );

    // Kinds arrive kebab-cased, as everywhere else on this boundary.
    assert_eq!(object["kind"], serde_json::json!("http-call"));
    assert_eq!(object["provenance"], serde_json::json!("route-matcher"));
}

#[test]
fn a_record_survives_a_json_round_trip() {
    let (graph, edge) = one_edge();
    let id = AnalysisId::first();
    let session = AnalysisSession::new(id, graph);
    let record = session.evidence(id, edge).expect("present");

    let text = serde_json::to_string(&record).expect("serialises");
    let back: cartograph_desktop::evidence::EvidenceRecord =
        serde_json::from_str(&text).expect("deserialises");

    assert_eq!(back, record);
    assert_eq!(back.confidence, record.confidence);
}

// ── A real repository ───────────────────────────────────────────────────

/// Fixtures contain the shapes this author thought of. This runs the lookup
/// over **every edge of a real analysis** — this repository, which is always
/// present — and checks each record against the graph it came from.
///
/// The assertion is exhaustive rather than sampled: a lookup that resolved the
/// wrong endpoint for one edge in fifty would pass a spot check.
#[test]
fn every_edge_of_a_real_repository_reports_its_own_evidence() {
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
        payload.scene.edge_count() > 0,
        "precondition: the analysis must produce edges to inspect"
    );

    for drawn in &payload.scene.edges {
        let record = session
            .evidence(id, drawn.id)
            .expect("every drawn edge must have evidence");

        // The record describes the edge the renderer drew.
        assert_eq!(record.edge, drawn.id);
        assert_eq!(record.kind, drawn.kind, "kind disagrees with the scene");
        assert_eq!(record.source.id, drawn.source, "source disagrees");
        assert_eq!(record.target.id, drawn.target, "target disagrees");

        // And it agrees with the graph the analysis produced.
        let edge = session.graph().edge(drawn.id).expect("in the graph");
        assert_eq!(record.confidence, edge.confidence().get());
        assert_eq!(record.provenance, edge.provenance());
        assert_eq!(record.evidence, edge.evidence().as_str());
        assert_eq!(record.location.file, edge.location().file());
        assert_eq!(record.location.line, edge.location().line());
        assert_eq!(record.deterministic, edge.is_deterministic());

        // Invariants that hold for every edge Cartograph produces.
        assert!(
            !record.evidence.is_empty(),
            "RULE 006: evidence is required"
        );
        assert!(
            (0.0..=1.0).contains(&record.confidence),
            "confidence outside [0,1]: {}",
            record.confidence
        );
        assert!(!record.calibrated, "nothing is calibrated yet");
        assert!(
            !record.location.file.contains(':') && !record.location.file.starts_with('/'),
            "an absolute path reached the panel: {}",
            record.location.file
        );
        assert!(
            !record.source.label.is_empty() && !record.target.label.is_empty(),
            "an endpoint had no name"
        );
    }
}

/// Re-analysing produces a new id, and the previous selection stops resolving.
/// This is the stale case as it actually arises — through the real API rather
/// than a hand-built pair of graphs.
#[test]
fn re_analysing_a_repository_invalidates_the_previous_selection() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("the repository root");
    let repository = cartograph_desktop::repository::validate(&root).expect("validates");

    let first_id = AnalysisId::first();
    let (first, _first_session) =
        cartograph_desktop::session::analyze_as(&repository, first_id).expect("analyses");
    let selected = first.scene.edges.first().expect("at least one edge").id;

    let second_id = first_id.next();
    let (_second, second_session) =
        cartograph_desktop::session::analyze_as(&repository, second_id).expect("analyses");

    // The same edge id exists in the new analysis — that is exactly why the
    // generation check is needed rather than an existence check.
    assert!(
        second_session.graph().edge(selected).is_some(),
        "precondition: the id must still exist, or this proves nothing"
    );

    let refused = second_session
        .evidence(first_id, selected)
        .expect_err("the old selection must not resolve against the new graph");
    assert_eq!(refused.kind, DesktopErrorKind::StaleSelection);

    // Presented correctly, the same edge answers.
    assert!(second_session.evidence(second_id, selected).is_ok());
}
