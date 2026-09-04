//! Degraded ASK: the answer with no model, and the guards around it.
//!
//! M16's second acceptance clause is *"with AI disabled the feature degrades
//! to showing the raw evidence"*, and these are the tests that hold it up.
//!
//! **Fidelity is asserted with exact equality.** An answer that reflowed,
//! trimmed or retitled a claim would look almost right on screen and would be
//! a different sentence from the one the resolver wrote. The whole value of
//! the degraded path is that a reader is looking at the analysis, not at a
//! rendering of it.
//!
//! **Refusal.** `NodeId` restarts at zero per analysis (ADR-0011), so a
//! selection held across a re-analysis names an artefact that almost certainly
//! still exists — as something else.
//! `a_stale_selection_is_refused_even_though_the_id_exists_here` builds exactly
//! that situation.
//!
//! **Offline.** Every test here builds its graph in memory. There is no
//! repository on disk, no key in the environment and no provider to call, and
//! the answers still arrive — which is the clause, stated as a test.

#![allow(
    clippy::float_cmp,
    reason = "confidence is compared, never computed; see ADR-0018"
)]

use cartograph_core::{
    Confidence, EdgeKind, Evidence, NodeId, NodeKind, Provenance, SourceLocation,
};
use cartograph_desktop::ask::{self, AiState};
use cartograph_desktop::error::DesktopErrorKind;
use cartograph_desktop::evidence::{AnalysisId, AnalysisSession};
use cartograph_graph::{ArchitectureGraph, EdgeSpec};
use cartograph_testkit::fixed_clock;

/// Evidence deliberately carrying punctuation, double spacing and mixed case,
/// so "unchanged" is a claim a reformatting bug would break.
const AWKWARD: &str = "GET /api/orders  matched  GET /api/orders (list_orders); \
                       neither side declared a method.";

/// A chain — `caller -> handler -> Model` — built entirely in memory.
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

    for (from, to, kind, confidence, evidence) in [
        (0, 1, EdgeKind::HttpCall, 0.98_f32, AWKWARD),
        (
            1,
            2,
            EdgeKind::OrmAccess,
            0.80_f32,
            "Model.objects.all(...) in handler at api/routes.py:2",
        ),
    ] {
        graph
            .add_edge(EdgeSpec {
                source: ids[from],
                target: ids[to],
                kind,
                confidence: Confidence::new(confidence).expect("valid"),
                provenance: Provenance::RouteMatcher,
                evidence: Evidence::new(evidence).expect("valid"),
                location: SourceLocation::new("api/routes.py", 2).expect("valid"),
                commit: None,
            })
            .expect("valid edge");
    }

    (graph, ids)
}

fn session_of(graph: ArchitectureGraph) -> AnalysisSession {
    AnalysisSession::new(AnalysisId::first(), graph)
}

// ── the clause ──────────────────────────────────────────────────────

#[test]
fn an_answer_reports_that_no_model_was_involved() {
    let (graph, ids) = chain();
    let session = session_of(graph);

    let answer = ask::answer(&session, session.id(), ids[0]).expect("answers");

    assert_eq!(answer.ai, AiState::Disabled);
}

/// The regression test for M16's second acceptance clause.
///
/// It is written so it still means something after a provider exists: the
/// claim is not "there is no AI" but "with AI disabled, the derived evidence
/// is still what comes back". When Slice 5 lands, this must keep passing for
/// the disabled case, or the clause has quietly stopped being true.
#[test]
fn with_no_model_no_key_and_no_network_the_answer_is_the_derived_evidence() {
    let (graph, ids) = chain();
    let expected: Vec<String> = graph
        .edges()
        .map(|e| e.evidence().as_str().to_owned())
        .collect();
    let session = session_of(graph);

    let answer = ask::answer(&session, session.id(), ids[0]).expect("answers offline");

    assert_eq!(answer.ai, AiState::Disabled);
    assert!(!answer.is_empty(), "the degraded path must still answer");

    let mut got: Vec<String> = answer.entries.iter().map(|e| e.evidence.clone()).collect();
    let mut want = expected;
    got.sort();
    want.sort();
    assert_eq!(got, want, "the answer is not the analysis's own evidence");
}

// ── fidelity ────────────────────────────────────────────────────────

#[test]
fn evidence_survives_byte_for_byte() {
    let (graph, ids) = chain();
    let session = session_of(graph);

    let answer = ask::answer(&session, session.id(), ids[0]).expect("answers");

    let quoted: Vec<&str> = answer.entries.iter().map(|e| e.evidence.as_str()).collect();
    assert!(
        quoted.contains(&AWKWARD),
        "evidence was altered: {quoted:?}"
    );
}

#[test]
fn evidence_is_not_reformatted() {
    let (graph, ids) = chain();
    let session = session_of(graph);

    let answer = ask::answer(&session, session.id(), ids[0]).expect("answers");
    let http = answer
        .entries
        .iter()
        .find(|e| e.kind == EdgeKind::HttpCall)
        .expect("the http edge is in the answer");

    // Each of these would be a plausible, wrong "tidy-up".
    assert!(http.evidence.contains("  matched  "), "spacing collapsed");
    assert!(http.evidence.ends_with('.'), "trailing punctuation trimmed");
    assert_eq!(http.evidence, AWKWARD);
}

#[test]
fn every_field_is_the_edge_s_own() {
    let (graph, ids) = chain();
    let expected: Vec<(EdgeKind, f32, Provenance)> = graph
        .edges()
        .map(|e| (e.kind(), e.confidence().get(), e.provenance()))
        .collect();
    let session = session_of(graph);

    let answer = ask::answer(&session, session.id(), ids[0]).expect("answers");

    for entry in &answer.entries {
        assert!(
            expected.contains(&(entry.kind, entry.confidence, entry.provenance)),
            "an entry carries a field the graph did not produce"
        );
        assert!(!entry.calibrated, "confidence must stay uncalibrated");
    }
}

// ── shape ───────────────────────────────────────────────────────────

#[test]
fn an_artefact_with_nothing_downstream_answers_empty_rather_than_failing() {
    let (graph, ids) = chain();
    let session = session_of(graph);

    // `Model` is the end of the chain: nothing leads away from it.
    let answer = ask::answer(&session, session.id(), ids[2]).expect("answers");

    assert!(answer.is_empty());
    assert_eq!(answer.len(), 0);
    assert_eq!(answer.ai, AiState::Disabled);
}

#[test]
fn several_relationships_are_all_carried() {
    let (graph, ids) = chain();
    let session = session_of(graph);

    let answer = ask::answer(&session, session.id(), ids[0]).expect("answers");

    assert_eq!(answer.len(), 2, "an entry was dropped");
}

#[test]
fn asking_twice_gives_the_same_answer() {
    let (graph, ids) = chain();
    let session = session_of(graph);

    let first = ask::answer(&session, session.id(), ids[0]).expect("answers");
    let second = ask::answer(&session, session.id(), ids[0]).expect("answers");

    assert_eq!(first, second);
    assert_eq!(first.scope, second.scope, "scope is not deterministic");
    let order = |a: &ask::AskAnswer| -> Vec<String> {
        a.entries.iter().map(|e| e.evidence.clone()).collect()
    };
    assert_eq!(order(&first), order(&second), "ordering is not stable");
}

// ── refusal ─────────────────────────────────────────────────────────

#[test]
fn a_stale_selection_is_refused() {
    let (graph, ids) = chain();
    let session = session_of(graph);

    let error = ask::answer(&session, session.id().next(), ids[0]).expect_err("refused");

    assert_eq!(error.kind, DesktopErrorKind::StaleSelection);
}

#[test]
fn a_stale_selection_is_refused_even_though_the_id_exists_here() {
    // The hazard, made concrete: the id names a real artefact in *this* graph,
    // so answering would explain a different thing, confidently.
    let (graph, ids) = chain();
    let session = AnalysisSession::new(AnalysisId::first().next(), graph);

    assert!(
        session.graph().node(ids[0]).is_some(),
        "precondition: the stale id resolves in this graph"
    );

    let error = ask::answer(&session, AnalysisId::first(), ids[0]).expect_err("refused");

    assert_eq!(error.kind, DesktopErrorKind::StaleSelection);
}

#[test]
fn an_artefact_outside_the_analysis_is_refused_rather_than_answered_empty() {
    let (graph, _) = chain();
    let session = session_of(graph);

    let error = ask::answer(&session, session.id(), NodeId::from_raw(9_999)).expect_err("refused");

    assert_eq!(error.kind, DesktopErrorKind::UnknownNode);
}

#[test]
fn evidence_that_must_not_leave_the_machine_stops_the_answer() {
    let mut graph = ArchitectureGraph::with_clock(fixed_clock);
    let from = graph
        .add_node(
            NodeKind::Function,
            "caller",
            Some(SourceLocation::new("web/app.ts", 1).expect("valid")),
        )
        .expect("node");
    let to = graph
        .add_node(
            NodeKind::Function,
            "handler",
            Some(SourceLocation::new("api/routes.py", 1).expect("valid")),
        )
        .expect("node");
    graph
        .add_edge(EdgeSpec {
            source: from,
            target: to,
            kind: EdgeKind::HttpCall,
            confidence: Confidence::new(0.9).expect("valid"),
            provenance: Provenance::RouteMatcher,
            // The placeholder form QG-005 allowlists; the shape is still the
            // one the bundle refuses.
            evidence: Evidence::new("resolved from /home/username/src/app.ts").expect("valid"),
            location: SourceLocation::new("api/routes.py", 1).expect("valid"),
            commit: None,
        })
        .expect("edge");
    let session = session_of(graph);

    let error = ask::answer(&session, session.id(), from).expect_err("refused");

    assert_eq!(error.kind, DesktopErrorKind::Internal);
    assert!(
        !error.message.contains("/home/"),
        "the refusal repeated the thing it refused: {}",
        error.message
    );
}

// ── privacy ─────────────────────────────────────────────────────────

#[test]
fn an_answer_carries_no_machine_path_no_source_and_no_credential() {
    let (graph, ids) = chain();
    let session = session_of(graph);

    let answer = ask::answer(&session, session.id(), ids[0]).expect("answers");

    let rendered = answer
        .entries
        .iter()
        .map(|e| {
            format!(
                "{}|{}|{}|{}|{}",
                e.source.label, e.target.label, e.evidence, e.location.file, e.provenance
            )
        })
        .collect::<Vec<_>>()
        .join("\n");

    for marker in [
        "/home/",
        "/Users/",
        "AppData",
        "runner/work",
        "ghp_",
        "AKIA",
    ] {
        assert!(
            !rendered.contains(marker),
            "answer carries a forbidden fragment: {marker}"
        );
    }
    assert!(!rendered.contains(":\\"), "answer carries a drive path");
    for entry in &answer.entries {
        assert!(
            !entry.location.file.starts_with('/'),
            "absolute location reached the answer: {}",
            entry.location.file
        );
    }
}

#[test]
fn the_answer_is_produced_from_memory_alone() {
    // The graph is built here, in memory. There is no repository on disk to
    // read, no configuration to consult and no provider to call, so an answer
    // arriving at all is the property: every input the degraded path has is
    // already in this function.
    let (graph, ids) = chain();
    let session = session_of(graph);

    let answer = ask::answer(&session, session.id(), ids[0]).expect("answers from memory");

    assert!(!answer.is_empty());
    assert_eq!(answer.ai, AiState::Disabled);
}
