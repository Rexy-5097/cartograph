//! The repository lifecycle, from a path the user chose to something drawable.
//!
//! # What is being defended
//!
//! A desktop application receives paths from a file dialog, from a text field,
//! and eventually from a drag from somewhere else on the machine. Every one of
//! those is untrusted, and the failure modes are boring right up until they are
//! not: a folder that vanished between the dialog closing and the button being
//! pressed, a file selected where a folder was meant, a directory the process
//! cannot enumerate, a path containing characters nobody tested.
//!
//! None of those may crash the window, and none of them may reach the user as
//! an unclassified string. So each has a case here.
//!
//! The second thing being defended is quieter: **an absolute path must never
//! appear in a message**. M09 shipped that defect in the CLI — redaction was
//! gated on `Path::is_absolute()`, which is false on Windows for a drive-less
//! rooted path — and it reached serialised output before anyone noticed. The
//! desktop reuses the same predicate rather than writing a second one, and the
//! tests below check the outcome rather than trusting the reuse.

use std::path::{Path, PathBuf};

use cartograph_desktop::Phase;
use cartograph_desktop::error::{DesktopError, DesktopErrorKind};
use cartograph_desktop::repository::{display_name, validate};
use cartograph_desktop::session::analyze;

/// A disposable directory, named by a counter rather than a clock so two runs
/// in one millisecond cannot collide and nothing in a path is nondeterministic.
struct Fixture {
    root: PathBuf,
}

impl Fixture {
    fn new(name: &str) -> Self {
        use std::sync::atomic::{AtomicUsize, Ordering};
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let unique = NEXT.fetch_add(1, Ordering::Relaxed);
        let root = std::env::temp_dir()
            .join("cartograph-m11-desktop")
            .join(format!("{name}-{}-{unique}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).expect("fixture root");
        Self { root }
    }

    fn write(&self, rel: &str, contents: &str) -> &Self {
        let path = self.root.join(rel);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).expect("fixture parent");
        }
        std::fs::write(path, contents).expect("write fixture");
        self
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

/// A tiny but genuinely analysable repository: a Django model, a route that
/// touches it, and a TypeScript caller. Small enough to reason about, real
/// enough that the analyser produces a graph rather than nothing.
fn analysable(fixture: &Fixture) {
    fixture
        .write(
            "api/models.py",
            "from django.db import models\n\n\nclass Order(models.Model):\n    class Meta:\n        db_table = \"orders\"\n",
        )
        .write(
            "api/views.py",
            "from django.urls import path\nfrom .models import Order\n\n\ndef create_order(request):\n    return Order.objects.all()\n\n\nurlpatterns = [path(\"orders/\", create_order)]\n",
        )
        .write(
            "web/cart.ts",
            "export async function submit() {\n  return fetch(\"/orders/\", { method: \"GET\" });\n}\n",
        );
}

// ── Validation: the paths a user can actually produce ───────────────────

#[test]
fn a_readable_directory_validates() {
    let fixture = Fixture::new("readable");
    let repository = validate(fixture.path()).expect("a real directory validates");

    assert!(
        repository.display_name().starts_with("readable-"),
        "the shown name should be the final component, got {:?}",
        repository.display_name()
    );
    assert_eq!(repository.path(), fixture.path());
}

#[test]
fn a_path_that_does_not_exist_is_not_found() {
    let fixture = Fixture::new("missing");
    let error = validate(&fixture.path().join("nowhere")).expect_err("must fail");

    assert_eq!(error.kind, DesktopErrorKind::NotFound);
    assert!(error.hint.is_some(), "the user needs a next action");
    assert!(error.kind.is_user_correctable());
}

/// Selecting a file where a folder was meant is the single most likely
/// mis-selection in a file dialog, and it must not read as "does not exist" —
/// the user would go looking for a folder that is right there.
#[test]
fn a_file_is_reported_as_not_a_directory_rather_than_missing() {
    let fixture = Fixture::new("file-not-dir");
    fixture.write("a_file.py", "x = 1\n");
    let error = validate(&fixture.path().join("a_file.py")).expect_err("must fail");

    assert_eq!(error.kind, DesktopErrorKind::NotADirectory);
    assert_ne!(
        error.kind,
        DesktopErrorKind::NotFound,
        "a file that exists must not be reported as absent"
    );
}

#[test]
fn an_empty_path_is_rejected_without_touching_the_filesystem() {
    let error = validate(Path::new("")).expect_err("must fail");
    assert_eq!(error.kind, DesktopErrorKind::NotFound);
}

#[test]
fn a_path_containing_spaces_validates() {
    let fixture = Fixture::new("with spaces");
    let repository = validate(fixture.path()).expect("spaces are ordinary");
    assert!(repository.display_name().contains(' '));
}

/// Non-ASCII path components are not exotic — they are what a large share of
/// the world's filesystems contain.
#[test]
fn a_unicode_path_validates_and_survives_the_round_trip() {
    let fixture = Fixture::new("プロジェクト-Ω-café");
    let repository = validate(fixture.path()).expect("unicode is ordinary");

    assert!(
        repository.display_name().contains("プロジェクト"),
        "the shown name lost its characters: {:?}",
        repository.display_name()
    );

    let error = DesktopError::new(DesktopErrorKind::NotFound, repository.display_name());
    let json = serde_json::to_string(&error).expect("serialises");
    let back: DesktopError = serde_json::from_str(&json).expect("deserialises");
    assert_eq!(error, back, "unicode must survive the IPC boundary");
}

#[test]
fn a_very_long_path_fails_cleanly_rather_than_panicking() {
    let fixture = Fixture::new("long");
    // Deep rather than one enormous component: Windows rejects a component
    // over 255 characters outright, and the interesting case is the total.
    let mut deep = fixture.path().to_path_buf();
    for i in 0..40 {
        deep = deep.join(format!("segment_{i}_padding_padding_padding_padding"));
    }

    // Either outcome is acceptable; crashing is not, and neither is a message
    // that fails to classify.
    match validate(&deep) {
        Ok(_) => panic!("a directory that was never created must not validate"),
        Err(error) => assert!(
            matches!(
                error.kind,
                DesktopErrorKind::NotFound | DesktopErrorKind::PermissionDenied
            ),
            "unexpected classification: {:?}",
            error.kind
        ),
    }
}

// ── Redaction ───────────────────────────────────────────────────────────

/// The M09 defect, checked in the desktop's own terms. Both rooted forms must
/// be reduced to their final component — `is_absolute()` alone returns false
/// for the drive-less one on Windows, which is exactly how the original leak
/// escaped review.
#[test]
fn a_rooted_path_is_never_shown_in_full() {
    #[cfg(windows)]
    const ROOTED: &[&str] = &[
        r"C:\workspace\acme-secret",
        r"\workspace\acme-secret",
        "/workspace/acme-secret",
        r"\\server\share\acme-secret",
    ];
    #[cfg(not(windows))]
    const ROOTED: &[&str] = &["/workspace/acme-secret"];

    for raw in ROOTED {
        let shown = display_name(Path::new(raw));
        assert_eq!(
            shown, "acme-secret",
            "{raw} was not reduced to its final component"
        );
        assert!(
            !shown.contains("workspace"),
            "{raw} leaked a parent directory as {shown}"
        );
    }
}

/// Redaction that redacted everything would be useless: a relative path tells
/// the user nothing they did not type, and hiding it makes the window confusing.
#[test]
fn a_relative_path_is_shown_as_typed() {
    assert_eq!(display_name(Path::new("projects/app")), "projects/app");
    assert_eq!(display_name(Path::new("app")), "app");
}

#[test]
fn no_error_message_from_a_rooted_path_contains_its_parents() {
    let fixture = Fixture::new("leak-check");
    let missing = fixture.path().join("definitely-not-here");
    let error = validate(&missing).expect_err("must fail");

    let parent = fixture
        .path()
        .parent()
        .expect("the fixture has a parent")
        .to_string_lossy()
        .into_owned();
    assert!(
        !error.message.contains(&parent),
        "the message leaked an absolute prefix: {}",
        error.message
    );
    let serialised = serde_json::to_string(&error).expect("serialises");
    assert!(
        !serialised.contains(&parent),
        "the serialised payload leaked an absolute prefix: {serialised}"
    );
}

// ── Analysis ────────────────────────────────────────────────────────────

#[test]
fn a_repository_with_supported_sources_analyses_and_lays_out() {
    let fixture = Fixture::new("analysable");
    analysable(&fixture);

    let repository = validate(fixture.path()).expect("validates");
    let payload = analyze(&repository).expect("analyses");

    assert!(
        payload.summary.files >= 3,
        "precondition: the fixture has three source files, saw {}",
        payload.summary.files
    );
    assert!(
        payload.summary.nodes > 0,
        "a repository with a model and a route must produce nodes"
    );
    assert_eq!(
        payload.scene.node_count(),
        payload.summary.nodes,
        "every node must be positioned"
    );
    assert!(
        payload.summary.clusters > 0,
        "positioned nodes must belong to clusters"
    );
    assert_eq!(payload.repository, repository.display_name());

    for node in &payload.scene.nodes {
        assert!(
            node.x.is_finite() && node.y.is_finite(),
            "a non-finite coordinate would silently drop the node from a WebGL scene"
        );
    }
}

/// An empty folder is a legitimate mistake, and a blank window with no
/// explanation is the worst possible response to it.
#[test]
fn a_directory_with_no_supported_sources_is_an_explained_failure() {
    let fixture = Fixture::new("empty");
    fixture.write("README.md", "# nothing to analyse\n");

    let repository = validate(fixture.path()).expect("validates");
    let error = analyze(&repository).expect_err("must fail rather than draw nothing");

    assert_eq!(error.kind, DesktopErrorKind::NoSupportedSources);
    assert!(error.kind.is_user_correctable());
    assert!(error.hint.is_some());
}

/// The payload is what crosses into JavaScript. It must carry counts, names of
/// clusters, and coordinates — and no source text.
#[test]
fn the_payload_carries_no_source_text() {
    let fixture = Fixture::new("no-source-text");
    analysable(&fixture);

    let repository = validate(fixture.path()).expect("validates");
    let payload = analyze(&repository).expect("analyses");
    let json = serde_json::to_string(&payload).expect("serialises");

    for fragment in [
        "from django.db import models",
        "return Order.objects.all()",
        "export async function submit",
        "urlpatterns",
    ] {
        assert!(
            !json.contains(fragment),
            "source text reached the frontend payload: {fragment}"
        );
    }
}

#[test]
fn the_payload_survives_the_ipc_boundary() {
    let fixture = Fixture::new("round-trip");
    analysable(&fixture);

    let repository = validate(fixture.path()).expect("validates");
    let payload = analyze(&repository).expect("analyses");
    let json = serde_json::to_string(&payload).expect("serialises");
    let back: cartograph_desktop::session::AnalysisPayload =
        serde_json::from_str(&json).expect("deserialises");

    assert_eq!(payload.summary, back.summary);
    assert_eq!(payload.repository, back.repository);
    assert_eq!(payload.scene.node_count(), back.scene.node_count());
}

/// Two analyses of one unchanged repository must agree. If they did not, the
/// window would appear to redraw itself for no reason.
#[test]
fn analysing_twice_produces_the_same_payload() {
    let fixture = Fixture::new("deterministic");
    analysable(&fixture);

    let repository = validate(fixture.path()).expect("validates");
    let first = analyze(&repository).expect("analyses");
    let second = analyze(&repository).expect("analyses again");

    assert_eq!(
        first, second,
        "the session must be a pure function of the tree"
    );
}

// ── The error contract ──────────────────────────────────────────────────

/// The frontend branches on `kind`. If the wire name changed, every branch
/// would silently stop matching, so the names are pinned here rather than
/// discovered in the browser.
#[test]
fn error_kinds_serialise_under_their_agreed_names() {
    let expected = [
        (DesktopErrorKind::NotFound, "\"notFound\""),
        (DesktopErrorKind::NotADirectory, "\"notADirectory\""),
        (DesktopErrorKind::PermissionDenied, "\"permissionDenied\""),
        (
            DesktopErrorKind::NoSupportedSources,
            "\"noSupportedSources\"",
        ),
        (DesktopErrorKind::AnalysisFailed, "\"analysisFailed\""),
        (DesktopErrorKind::Cancelled, "\"cancelled\""),
        (DesktopErrorKind::Internal, "\"internal\""),
    ];
    for (kind, wire) in expected {
        assert_eq!(serde_json::to_string(&kind).expect("serialises"), wire);
    }
}

#[test]
fn phase_names_match_the_frontend_union() {
    let expected = [
        (Phase::Idle, "\"idle\""),
        (Phase::Selecting, "\"selecting\""),
        (Phase::Analyzing, "\"analyzing\""),
        (Phase::Ready, "\"ready\""),
        (Phase::Failed, "\"failed\""),
        (Phase::Cancelled, "\"cancelled\""),
    ];
    for (phase, wire) in expected {
        assert_eq!(serde_json::to_string(&phase).expect("serialises"), wire);
    }
}

/// Starting a second analysis while one is running is how two results race to
/// populate one window. Every other state accepts a new selection, including
/// the failure states — recovering must not need a restart.
#[test]
fn only_an_analysis_in_flight_refuses_a_new_selection() {
    assert!(!Phase::Analyzing.accepts_selection());
    for phase in [
        Phase::Idle,
        Phase::Selecting,
        Phase::Ready,
        Phase::Failed,
        Phase::Cancelled,
    ] {
        assert!(
            phase.accepts_selection(),
            "{phase:?} must allow choosing another repository"
        );
    }
    assert!(Phase::Ready.has_result());
    assert!(!Phase::Cancelled.has_result());
}

/// Cancellation is a frontend decision, not an interruption. What must hold is
/// that abandoning a result leaves nothing behind: the next analysis of the
/// same tree is identical to one run with no cancellation before it.
#[test]
fn abandoning_a_result_leaves_no_residue() {
    let fixture = Fixture::new("cancelled");
    analysable(&fixture);
    let repository = validate(fixture.path()).expect("validates");

    let baseline = analyze(&repository).expect("analyses");

    let abandoned = analyze(&repository).expect("analyses");
    drop(abandoned); // the window navigated away; the result is discarded

    let after = analyze(&repository).expect("analyses");
    assert_eq!(
        baseline, after,
        "a discarded analysis must not affect the next one"
    );
}

/// A hint is advice, not a second message channel: it must never carry a path
/// either.
#[test]
fn the_internal_error_says_it_is_a_defect() {
    let error = DesktopError::internal("the graph had no root");
    assert_eq!(error.kind, DesktopErrorKind::Internal);
    assert!(!error.kind.is_user_correctable());
    assert!(
        error
            .hint
            .expect("internal errors carry a hint")
            .contains("defect"),
        "the user should be told this is not their fault"
    );
}

// ── A real repository ───────────────────────────────────────────────────

/// The repository this crate lives in.
fn own_repository() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("the crate sits two levels below the repository root")
        .to_path_buf()
}

/// The whole lifecycle over a tree nobody wrote for the test.
///
/// The fixtures above contain the shapes this author thought of. A real
/// checkout contains files at the root, artefacts with no source location,
/// directories holding one node, and paths with characters nobody chose.
///
/// The subject is **this repository**, which is always present, so this runs
/// everywhere rather than being skipped when a corpus is missing. It is
/// read-only: the analyser opens files and never writes.
#[test]
fn the_lifecycle_runs_over_a_real_repository() {
    let root = own_repository();
    let repository = validate(&root).expect("the repository validates");
    let payload = analyze(&repository).expect("the repository analyses");

    assert!(
        payload.summary.files > 0,
        "precondition: a real checkout must yield files"
    );
    assert_eq!(
        payload.scene.node_count(),
        payload.summary.nodes,
        "every node the analyser found must be positioned"
    );
    for node in &payload.scene.nodes {
        assert!(node.x.is_finite() && node.y.is_finite());
        assert!(
            payload.scene.clusters.iter().any(|c| c.id == node.cluster),
            "node {} names a cluster outside the table",
            node.id
        );
    }
    assert!(
        !payload.repository.contains(std::path::MAIN_SEPARATOR),
        "the shown name must be a component, not a path: {}",
        payload.repository
    );

    println!(
        "cartograph: {} files -> {} nodes, {} edges, {} clusters",
        payload.summary.files,
        payload.summary.nodes,
        payload.summary.edges,
        payload.summary.clusters
    );
}

/// The same lifecycle against a pinned benchmark corpus.
///
/// Opt-in: this repository does not vendor the corpora, and the checkout is
/// **never mutated** — the analyser only reads.
#[test]
#[ignore = "needs CARTOGRAPH_LAYOUT_REPO; the pinned corpora are not vendored here"]
fn the_lifecycle_runs_over_a_pinned_corpus() {
    let Ok(path) = std::env::var("CARTOGRAPH_LAYOUT_REPO") else {
        return;
    };
    let repository = validate(Path::new(&path)).expect("the corpus validates");
    let payload = analyze(&repository).expect("the corpus analyses");

    assert!(payload.summary.nodes > 0, "a corpus must produce nodes");
    assert_eq!(payload.scene.node_count(), payload.summary.nodes);
    println!(
        "corpus: {} files -> {} nodes, {} edges, {} clusters",
        payload.summary.files,
        payload.summary.nodes,
        payload.summary.edges,
        payload.summary.clusters
    );
}
