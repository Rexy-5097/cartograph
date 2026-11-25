//! The M18 browser experiment's additions: report, trace and source excerpt.
//!
//! Each is checked against the core it projects, not against a hand-written
//! expectation, and each is checked to refuse what it must: a stale analysis,
//! an unknown artefact, and (for source) any file the analysis does not cite.

use std::path::{Path, PathBuf};

use cartograph_desktop::error::DesktopErrorKind;
use cartograph_desktop::evidence::AnalysisId;
use cartograph_desktop::repository::validate;
use cartograph_desktop::session::{analyze_as, analyze_report_as};
use cartograph_desktop::{inspect, source, trace};
use cartograph_graph::trace::{DEFAULT_MAX_DEPTH, trace as core_trace};
use cartograph_pipeline::pipeline::{self, Options};

struct Fixture {
    root: PathBuf,
}

impl Fixture {
    fn new(name: &str) -> Self {
        use std::sync::atomic::{AtomicUsize, Ordering};
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let unique = NEXT.fetch_add(1, Ordering::Relaxed);
        let root = std::env::temp_dir()
            .join("cartograph-m18-browser")
            .join(format!("{name}-{}-{unique}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).expect("fixture root");
        let fixture = Self { root };
        fixture.write(
            "shop/models.py",
            "from django.db import models\n\n\nclass Order(models.Model):\n    class Meta:\n        db_table = 'orders'\n",
        );
        fixture.write(
            "shop/views.py",
            "from fastapi import APIRouter\nfrom shop.models import Order\n\nrouter = APIRouter()\n\n\n@router.get('/api/orders/')\ndef list_orders():\n    return Order.objects.all()\n",
        );
        fixture.write(
            "web/api.ts",
            "export function loadOrders() {\n  return fetch('/api/orders/');\n}\nexport function loadMissing() {\n  return fetch('/api/missing/');\n}\n",
        );
        fixture
    }

    fn write(&self, rel: &str, contents: &str) {
        let path = self.root.join(rel);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).expect("fixture parent");
        }
        std::fs::write(path, contents).expect("write fixture");
    }

    fn path(&self) -> &Path {
        &self.root
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

#[test]
fn the_report_is_the_pipelines_own_counts_and_decisions() {
    let fixture = Fixture::new("report");
    let repository = validate(fixture.path()).expect("validates");
    let (payload, report, _) =
        analyze_report_as(&repository, AnalysisId::first()).expect("analyses");
    let direct = pipeline::run(fixture.path(), Options { quiet: true }).expect("runs");

    assert_eq!(report.analysis, payload.analysis);
    assert_eq!(report.totals.nodes, direct.totals.nodes);
    assert_eq!(report.totals.edges, direct.totals.edges);
    assert_eq!(report.totals.routes, direct.totals.routes);
    assert_eq!(
        report.totals.http_observations,
        direct.totals.http_observations
    );
    assert_eq!(report.decisions.len(), direct.results.len());
    assert_eq!(report.totals.nodes, payload.summary.nodes);
    assert_eq!(report.nodes.len(), direct.graph.node_count());
    for located in &report.nodes {
        let node = direct.graph.node(located.id).expect("a real node");
        assert_eq!(
            located.file.as_deref(),
            node.location().map(cartograph_core::SourceLocation::file)
        );
    }
    // The unmatched request is reported as a refusal, not dropped.
    assert!(
        report
            .decisions
            .iter()
            .any(|d| d.status == "no-match" && d.client_route.contains("missing")),
        "{:?}",
        report.decisions
    );
    // Paths stay repository-relative.
    assert!(
        report
            .decisions
            .iter()
            .all(|d| !Path::new(&d.client_file).is_absolute())
    );
}

#[test]
fn analyze_as_is_unchanged_by_the_report() {
    let fixture = Fixture::new("same");
    let repository = validate(fixture.path()).expect("validates");
    let (with_report, _, _) =
        analyze_report_as(&repository, AnalysisId::first()).expect("analyses");
    let (without, _) = analyze_as(&repository, AnalysisId::first()).expect("analyses");
    assert_eq!(with_report.summary, without.summary);
    assert_eq!(with_report.scene.nodes.len(), without.scene.nodes.len());
    assert_eq!(with_report.scene.edges.len(), without.scene.edges.len());
}

#[test]
fn a_trace_is_the_core_walk_and_refuses_a_stale_or_unknown_start() {
    let fixture = Fixture::new("trace");
    let repository = validate(fixture.path()).expect("validates");
    let (payload, session) = analyze_as(&repository, AnalysisId::first()).expect("analyses");
    let graph = session.graph();
    let start = graph
        .nodes()
        .find(|n| n.name() == "loadOrders")
        .expect("the caller is a node")
        .id();

    let walked = trace::walk(&session, payload.analysis, start).expect("traces");
    let core = core_trace(graph, start, DEFAULT_MAX_DEPTH);
    assert_eq!(walked.from, core.from);
    assert_eq!(walked.max_depth, core.max_depth);
    assert_eq!(walked.stop, core.stop);
    assert_same_hops(&walked.steps, &core.steps);
    assert!(!walked.steps.is_empty(), "the request reaches its handler");
    assert!(
        walked.steps.iter().any(|s| !s.next.is_empty()),
        "the fixture's trace is deeper than one hop, so nesting is exercised"
    );

    let stale = trace::walk(&session, AnalysisId::first().next(), start).expect_err("stale");
    assert_eq!(stale.kind, DesktopErrorKind::StaleSelection);
    let missing = graph.nodes().map(|n| n.id().as_u64()).max().expect("nodes");
    let unknown = trace::walk(
        &session,
        payload.analysis,
        cartograph_core::NodeId::from_raw(missing + 1000),
    )
    .expect_err("unknown");
    assert_eq!(unknown.kind, DesktopErrorKind::UnknownNode);
}

#[test]
fn a_source_excerpt_reads_only_files_the_analysis_cites() {
    let fixture = Fixture::new("source");
    fixture.write("secret.txt", "not source\n");
    let repository = validate(fixture.path()).expect("validates");
    let (payload, session) = analyze_as(&repository, AnalysisId::first()).expect("analyses");

    let excerpt = source::excerpt(
        &repository,
        &session,
        payload.analysis,
        "web/api.ts",
        Some(2),
    )
    .expect("a cited file previews");
    assert_eq!(excerpt.file, "web/api.ts");
    assert_eq!(excerpt.highlight, Some(2));
    assert_eq!(excerpt.first_line, 1);
    assert!(excerpt.lines[1].contains("fetch('/api/orders/')"));

    for refused in [
        "secret.txt",
        "../secret.txt",
        "web/../secret.txt",
        "/etc/passwd",
    ] {
        let error = source::excerpt(&repository, &session, payload.analysis, refused, None)
            .expect_err("an uncited path is refused");
        assert_eq!(error.kind, DesktopErrorKind::NotFound, "{refused}");
    }
    let stale = source::excerpt(
        &repository,
        &session,
        AnalysisId::first().next(),
        "web/api.ts",
        None,
    )
    .expect_err("stale");
    assert_eq!(stale.kind, DesktopErrorKind::StaleSelection);
}

#[test]
fn a_node_detail_is_the_graphs_own_location_and_edges() {
    let fixture = Fixture::new("inspect");
    let repository = validate(fixture.path()).expect("validates");
    let (payload, session) = analyze_as(&repository, AnalysisId::first()).expect("analyses");
    let graph = session.graph();
    let caller = graph
        .nodes()
        .find(|n| n.name() == "loadOrders")
        .expect("the caller is a node");

    let detail = inspect::node(&session, payload.analysis, caller.id()).expect("inspects");
    assert_eq!(detail.label, "loadOrders");
    assert_eq!(
        detail.file.as_deref(),
        caller.location().map(cartograph_core::SourceLocation::file)
    );
    assert_eq!(
        detail.line,
        caller.location().map(cartograph_core::SourceLocation::line)
    );
    assert_eq!(detail.outgoing.len(), graph.edges_from(caller.id()).count());
    assert_eq!(detail.incoming.len(), graph.edges_to(caller.id()).count());
    for out in &detail.outgoing {
        let edge = graph.edge(out.edge).expect("a real edge");
        assert_eq!(edge.source(), caller.id());
        assert_eq!(edge.target(), out.node);
    }

    let stale =
        inspect::node(&session, AnalysisId::first().next(), caller.id()).expect_err("stale");
    assert_eq!(stale.kind, DesktopErrorKind::StaleSelection);
}

/// Every hop, at every depth, with its stop reason, equals the core's.
fn assert_same_hops(ours: &[trace::TraceHop], core: &[cartograph_graph::trace::TraceStep]) {
    assert_eq!(ours.len(), core.len());
    for (a, b) in ours.iter().zip(core) {
        assert_eq!(a.edge, b.edge);
        assert_eq!(a.to, b.to);
        assert_eq!(a.stop, b.stop);
        assert_same_hops(&a.next, &b.next);
    }
}
