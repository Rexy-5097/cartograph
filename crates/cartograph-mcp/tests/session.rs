//! The MCP session, tested without a transport.
//!
//! Everything that decides *whether* a question may be answered is here, and
//! none of it needs a protocol stream. That separation is deliberate: the
//! security property — one repository per session, refused before analysis — is
//! not a property of MCP, and a test that had to speak JSON-RPC to check it
//! would be testing the wrong layer.
//!
//! Trees are throwaway fixtures written under the temporary directory, so these
//! run anywhere and touch no corpus.

use std::path::{Path, PathBuf};

use cartograph_mcp::server::dispatch;
use cartograph_mcp::{Grant, QueryError, Session};
use rmcp::model::CallToolRequestParams;

/// A pair of tiny repositories, removed when the test finishes.
struct Fixture {
    root: PathBuf,
}

impl Fixture {
    fn new(tag: &str) -> Self {
        let root = std::env::temp_dir().join(format!(
            "cartograph-mcp-{tag}-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        let _ = std::fs::remove_dir_all(&root);
        Self { root }
    }

    /// Writes a minimal analysable repository and returns its path.
    fn repo(&self, name: &str) -> PathBuf {
        let path = self.root.join(name);
        std::fs::create_dir_all(path.join("api")).expect("create");
        std::fs::write(
            path.join("api/models.py"),
            "from django.db import models\n\nclass Order(models.Model):\n    class Meta:\n        db_table = 'orders'\n",
        )
        .expect("write");
        std::fs::write(
            path.join("api/routes.py"),
            "from flask import Flask\nfrom .models import Order\n\napp = Flask(__name__)\n\n@app.route('/api/orders')\ndef list_orders():\n    return Order.objects.all()\n",
        )
        .expect("write");
        path
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

fn session(trees: Vec<PathBuf>) -> Session {
    Session::new(
        Grant::new("granted-repository", trees)
            .authorize()
            .expect("the grant authorizes"),
    )
}

fn call(name: &str, args: &serde_json::Value) -> CallToolRequestParams {
    // `CallToolRequestParams` is `#[non_exhaustive]`; its builder is the
    // supported way to construct one.
    let request = CallToolRequestParams::new(name.to_owned());
    args.as_object().map_or_else(
        || request.clone(),
        |arguments| request.clone().with_arguments(arguments.clone()),
    )
}

fn refusal(error: &QueryError) -> String {
    error.message()
}

// ── the grant ───────────────────────────────────────────────────────

#[test]
fn a_grant_with_no_identity_is_refused_at_startup() {
    assert!(
        Grant::new("", vec![PathBuf::from("x")])
            .authorize()
            .is_err()
    );
    assert!(
        Grant::new("   ", vec![PathBuf::from("x")])
            .authorize()
            .is_err()
    );
}

#[test]
fn a_grant_with_no_tree_is_refused_at_startup() {
    assert!(Grant::new("id", Vec::new()).authorize().is_err());
}

#[test]
fn the_session_reports_how_many_trees_it_holds_but_not_which() {
    let session = session(vec![PathBuf::from("a"), PathBuf::from("b")]);
    assert_eq!(session.tree_count(), 2);
}

// ── one repository per session ──────────────────────────────────────

#[test]
fn the_authorized_repository_answers() {
    let fixture = Fixture::new("authorized");
    let repo = fixture.repo("granted");
    let session = session(vec![repo.clone()]);

    let map = session.map(&repo).expect("the granted tree is analysed");
    assert!(map.totals.artefacts > 0, "the fixture must produce a graph");
    assert!(!map.languages.is_empty());
}

#[test]
fn an_ungranted_repository_is_refused() {
    let fixture = Fixture::new("ungranted");
    let granted = fixture.repo("granted");
    let other = fixture.repo("other");
    let session = session(vec![granted]);

    let error = session.map(&other).expect_err("refused");
    assert!(matches!(error, QueryError::Refused(_)));
}

/// The refusal must arrive before the analyser opens anything. The path does
/// not exist, so a pipeline failure would have been `InvalidPath`.
#[test]
fn a_refusal_precedes_analysis() {
    let session = session(vec![PathBuf::from("granted")]);
    let error = session
        .map(Path::new("no/such/directory/anywhere"))
        .expect_err("refused");

    assert!(
        matches!(error, QueryError::Refused(_)),
        "authorization must answer first, not the analyser"
    );
}

/// A tree granted to one session is not granted to another.
#[test]
fn one_session_cannot_reach_another_sessions_repository() {
    let fixture = Fixture::new("cross");
    let a = fixture.repo("a");
    let b = fixture.repo("b");

    let first = session(vec![a.clone()]);
    let second = session(vec![b.clone()]);

    assert!(matches!(
        first.map(&b).expect_err("refused"),
        QueryError::Refused(_)
    ));
    assert!(matches!(
        second.map(&a).expect_err("refused"),
        QueryError::Refused(_)
    ));
}

// ── the diff rule ───────────────────────────────────────────────────

#[test]
fn a_diff_over_two_granted_trees_of_one_repository_is_allowed() {
    let fixture = Fixture::new("diff-ok");
    let before = fixture.repo("before");
    let after = fixture.repo("after");
    let session = session(vec![before.clone(), after.clone()]);

    let answer = session.diff(&before, &after).expect("both granted");
    // Identical fixtures, so nothing moved. What matters is that it ran.
    assert_eq!(answer.added, 0);
    assert_eq!(answer.removed, 0);
}

/// Repository A as `before` with repository B as `after`, refused in either
/// position because B was never granted.
#[test]
fn a_diff_naming_an_ungranted_repository_is_refused_in_either_position() {
    let fixture = Fixture::new("diff-refused");
    let granted = fixture.repo("granted");
    let other = fixture.repo("other");
    let session = session(vec![granted.clone()]);

    assert!(matches!(
        session.diff(&granted, &other).expect_err("refused"),
        QueryError::Refused(_)
    ));
    assert!(matches!(
        session.diff(&other, &granted).expect_err("refused"),
        QueryError::Refused(_)
    ));
}

// ── the tools ───────────────────────────────────────────────────────

#[test]
fn trace_follows_a_symbol_and_reports_its_evidence() {
    let fixture = Fixture::new("trace");
    let repo = fixture.repo("granted");
    let session = session(vec![repo.clone()]);

    let answer = session
        .trace(&repo, "list_orders", 10)
        .expect("the symbol resolves");
    assert_eq!(answer.from.name, "list_orders");
    assert!(
        !answer.steps.is_empty(),
        "the fixture's handler reaches a model"
    );
    assert!(!answer.steps[0].evidence.is_empty());
}

#[test]
fn blast_reports_what_depends_on_a_symbol() {
    let fixture = Fixture::new("blast");
    let repo = fixture.repo("granted");
    let session = session(vec![repo.clone()]);

    let answer = session.blast(&repo, "orders").expect("the table resolves");
    assert_eq!(answer.target.name, "orders");
}

#[test]
fn an_unknown_symbol_is_reported_as_not_found_rather_than_guessed_at() {
    let fixture = Fixture::new("unknown");
    let repo = fixture.repo("granted");
    let session = session(vec![repo.clone()]);

    let error = session
        .trace(&repo, "no_such_symbol", 10)
        .expect_err("none");
    assert!(matches!(error, QueryError::NotFound(_)));
    assert!(refusal(&error).contains("no_such_symbol"));
}

#[test]
fn a_map_is_byte_identical_across_runs() {
    let fixture = Fixture::new("determinism");
    let repo = fixture.repo("granted");
    let session = session(vec![repo.clone()]);

    let first = serde_json::to_string(&session.map(&repo).expect("map")).expect("json");
    for _ in 0..3 {
        let again = serde_json::to_string(&session.map(&repo).expect("map")).expect("json");
        assert_eq!(first, again, "the map changed between runs");
    }
}

// ── the tool surface ────────────────────────────────────────────────

/// The security surface, asserted rather than assumed: there is no tool that
/// authorizes, sets, switches or adds a repository.
#[test]
fn no_tool_can_change_the_authorization() {
    let session = session(vec![PathBuf::from("granted")]);

    for forbidden in [
        "authorize",
        "set_repository",
        "switch_repository",
        "add_repository",
        "grant",
        "open",
    ] {
        let error = dispatch(
            &session,
            forbidden,
            &call(forbidden, &serde_json::json!({})),
        )
        .expect_err("no such tool");
        assert!(
            matches!(error, QueryError::Invalid(_)),
            "`{forbidden}` must not be a tool"
        );
    }
}

#[test]
fn an_unknown_tool_is_refused_and_names_what_is_offered() {
    let session = session(vec![PathBuf::from("granted")]);
    let error =
        dispatch(&session, "wat", &call("wat", &serde_json::json!({}))).expect_err("unknown");

    let message = refusal(&error);
    for expected in ["map", "trace", "blast", "diff"] {
        assert!(message.contains(expected), "{message}");
    }
}

#[test]
fn a_malformed_request_is_refused_without_analysing_anything() {
    let session = session(vec![PathBuf::from("granted")]);

    // `tree` missing entirely.
    let error = dispatch(&session, "map", &call("map", &serde_json::json!({}))).expect_err("bad");
    assert!(matches!(error, QueryError::Invalid(_)));

    // `tree` present but the wrong type.
    let error = dispatch(
        &session,
        "map",
        &call("map", &serde_json::json!({ "tree": 42 })),
    )
    .expect_err("bad");
    assert!(matches!(error, QueryError::Invalid(_)));

    // `symbol` missing.
    let error = dispatch(
        &session,
        "trace",
        &call("trace", &serde_json::json!({ "tree": "granted" })),
    )
    .expect_err("bad");
    assert!(matches!(error, QueryError::Invalid(_)));
}

#[test]
fn a_tool_call_naming_an_ungranted_tree_is_refused() {
    let session = session(vec![PathBuf::from("granted")]);
    let error = dispatch(
        &session,
        "map",
        &call("map", &serde_json::json!({ "tree": "somewhere-else" })),
    )
    .expect_err("refused");

    assert!(matches!(error, QueryError::Refused(_)));
}

// ── privacy ─────────────────────────────────────────────────────────

/// A refusal must not name what was refused, nor the identity that refused it.
#[test]
fn a_refusal_leaks_neither_path_nor_identity() {
    let secret = if cfg!(windows) {
        r"C:\workspace\clients\acme-secret\repo"
    } else {
        "/srv/clients/acme-secret/repo"
    };
    let session = Session::new(
        Grant::new("s3cr3t-identity-value", vec![PathBuf::from("granted")])
            .authorize()
            .expect("grant"),
    );

    let error = session.map(Path::new(secret)).expect_err("refused");
    let message = refusal(&error);

    for leak in [
        secret,
        "acme-secret",
        "clients",
        "workspace",
        "s3cr3t-identity-value",
        "C:\\",
        "/srv/",
    ] {
        assert!(
            !message.contains(leak),
            "a refusal leaked {leak:?}: {message}"
        );
    }
}

/// An answer is an aggregate and a set of relationships. It must not become a
/// way to read the machine it ran on.
#[test]
fn no_answer_contains_an_absolute_path() {
    let fixture = Fixture::new("no-abs");
    let repo = fixture.repo("granted");
    let session = session(vec![repo.clone()]);

    let map = serde_json::to_string(&session.map(&repo).expect("map")).expect("json");
    let trace = serde_json::to_string(&session.trace(&repo, "list_orders", 10).expect("trace"))
        .expect("json");

    for document in [&map, &trace] {
        assert!(!document.contains("C:\\"), "{document}");
        assert!(!document.contains("/home/"), "{document}");
        assert!(!document.contains("/Users/"), "{document}");
        // The fixture's own temporary root must not appear either.
        let root = fixture.root.to_string_lossy().replace('\\', "/");
        assert!(
            !document.replace('\\', "/").contains(root.as_str()),
            "the analysed path leaked: {document}"
        );
    }
}

/// The identity is opaque and never printed, including through `Debug`.
#[test]
fn the_session_never_prints_its_identity() {
    let session = Session::new(
        Grant::new("s3cr3t-identity-value", vec![PathBuf::from("granted")])
            .authorize()
            .expect("grant"),
    );

    let debugged = format!("{session:?}");
    assert!(!debugged.contains("s3cr3t-identity-value"), "{debugged}");
}

// ── T-008: a session reuses what it parsed, and never answers stale ──

/// Everything a session says about a tree, as one comparable string.
fn answers(session: &Session, tree: &Path) -> String {
    let map = serde_json::to_string(&session.map(tree).expect("map")).expect("json");
    let blast = session.blast(tree, "orders").map_or_else(
        |e| format!("error: {}", refusal(&e)),
        |a| serde_json::to_string(&a).expect("json"),
    );
    let trace = session.trace(tree, "list_orders", 10).map_or_else(
        |e| format!("error: {}", refusal(&e)),
        |a| serde_json::to_string(&a).expect("json"),
    );
    format!("{map}\n{blast}\n{trace}")
}

/// What a session that has never seen the tree says about it now.
fn cold_answers(tree: &Path) -> String {
    answers(&session(vec![tree.to_path_buf()]), tree)
}

#[test]
fn a_repeated_question_reuses_every_file_and_gives_the_same_answer() {
    let fixture = Fixture::new("cache-repeat");
    let repo = fixture.repo("granted");
    let session = session(vec![repo.clone()]);

    assert_eq!(session.last_analysis(&repo), None, "nothing analysed yet");
    let first = serde_json::to_string(&session.map(&repo).expect("map")).expect("json");
    let cold = session.last_analysis(&repo).expect("analysed");
    assert_eq!((cold.seen, cold.parsed, cold.reused), (2, 2, 0));

    let second = serde_json::to_string(&session.map(&repo).expect("map")).expect("json");
    let warm = session.last_analysis(&repo).expect("analysed");
    assert_eq!(
        (warm.seen, warm.parsed, warm.reused),
        (2, 0, 2),
        "an unchanged tree is not parsed again"
    );
    assert_eq!(first, second);

    // A different tool on the same tree reuses the same facts.
    session.trace(&repo, "list_orders", 10).expect("trace");
    assert_eq!(session.last_analysis(&repo).expect("analysed").parsed, 0);
}

#[test]
fn a_changed_file_is_parsed_again_and_the_answer_follows_the_tree() {
    let fixture = Fixture::new("cache-change");
    let repo = fixture.repo("granted");
    let session = session(vec![repo.clone()]);
    let before = answers(&session, &repo);

    // The handler stops touching the model.
    std::fs::write(
        repo.join("api/routes.py"),
        "from flask import Flask\n\napp = Flask(__name__)\n\n@app.route('/api/orders')\ndef list_orders():\n    return []\n",
    )
    .expect("write");

    let after = answers(&session, &repo);
    assert_ne!(before, after, "the answer must follow the edit");
    assert_eq!(after, cold_answers(&repo), "warm and cold must agree");

    session.map(&repo).expect("map");
    let stats = session.last_analysis(&repo).expect("analysed");
    assert_eq!((stats.parsed, stats.reused), (0, 2), "and it settles again");
}

#[test]
fn only_the_file_that_changed_is_parsed_again() {
    let fixture = Fixture::new("cache-one");
    let repo = fixture.repo("granted");
    let session = session(vec![repo.clone()]);
    session.map(&repo).expect("map");

    let mut models = std::fs::read_to_string(repo.join("api/models.py")).expect("read");
    models.push_str(
        "\n\nclass Invoice(models.Model):\n    class Meta:\n        db_table = 'invoices'\n",
    );
    std::fs::write(repo.join("api/models.py"), models).expect("write");

    let map = serde_json::to_string(&session.map(&repo).expect("map")).expect("json");
    let stats = session.last_analysis(&repo).expect("analysed");
    assert_eq!((stats.seen, stats.parsed, stats.reused), (2, 1, 1));
    assert_eq!(
        map,
        serde_json::to_string(&session_map_cold(&repo)).expect("json")
    );
}

fn session_map_cold(tree: &Path) -> impl serde::Serialize {
    session(vec![tree.to_path_buf()]).map(tree).expect("map")
}

#[test]
fn an_added_file_and_a_deleted_file_are_both_seen() {
    let fixture = Fixture::new("cache-add-delete");
    let repo = fixture.repo("granted");
    let session = session(vec![repo.clone()]);
    session.map(&repo).expect("map");

    std::fs::write(
        repo.join("api/extra.py"),
        "from .models import Order\n\ndef count_orders():\n    return Order.objects.count()\n",
    )
    .expect("write");
    assert_eq!(answers(&session, &repo), cold_answers(&repo));
    let added = session.last_analysis(&repo).expect("analysed");
    assert_eq!(added.seen, 3);

    std::fs::remove_file(repo.join("api/extra.py")).expect("remove");
    std::fs::remove_file(repo.join("api/routes.py")).expect("remove");
    session.map(&repo).expect("map");
    let removed = session.last_analysis(&repo).expect("analysed");
    assert_eq!((removed.seen, removed.evicted), (1, 2), "{removed:?}");
    assert_eq!(answers(&session, &repo), cold_answers(&repo));
}

#[test]
fn an_edit_that_restores_the_old_content_is_answered_from_the_new_tree() {
    // Same bytes as before, written again: the timestamp differs and the
    // content does not. The answer is the one a cold session gives.
    let fixture = Fixture::new("cache-restore");
    let repo = fixture.repo("granted");
    let session = session(vec![repo.clone()]);
    let original = std::fs::read_to_string(repo.join("api/routes.py")).expect("read");
    let first = answers(&session, &repo);

    std::fs::write(repo.join("api/routes.py"), "x = 1\n").expect("write");
    assert_ne!(first, answers(&session, &repo));
    std::fs::write(repo.join("api/routes.py"), &original).expect("write");
    assert_eq!(first, answers(&session, &repo));
}

#[test]
fn two_trees_of_one_session_do_not_share_what_they_parsed() {
    let fixture = Fixture::new("cache-two");
    let a = fixture.repo("a");
    let b = fixture.repo("b");
    // Same relative paths, different content.
    std::fs::write(
        b.join("api/models.py"),
        "from django.db import models\n\nclass Order(models.Model):\n    class Meta:\n        db_table = 'purchase_orders'\n",
    )
    .expect("write");
    let session = session(vec![a.clone(), b.clone()]);

    let first_a = serde_json::to_string(&session.map(&a).expect("map")).expect("json");
    let first_b = serde_json::to_string(&session.map(&b).expect("map")).expect("json");
    let stats_b = session.last_analysis(&b).expect("analysed");
    assert_eq!(
        (stats_b.parsed, stats_b.reused),
        (2, 0),
        "the second tree starts cold: nothing of the first is reused"
    );
    assert_ne!(first_a, first_b);
    assert!(session.blast(&b, "purchase_orders").is_ok());
    assert!(
        session.blast(&a, "purchase_orders").is_err(),
        "the first tree does not gain the second tree's table"
    );
    assert_eq!(
        first_a,
        serde_json::to_string(&session.map(&a).expect("map")).expect("json")
    );
}

#[test]
fn a_refused_tree_is_never_analysed_or_remembered() {
    let fixture = Fixture::new("cache-refused");
    let granted = fixture.repo("granted");
    let other = fixture.repo("other");
    let session = session(vec![granted.clone()]);

    assert!(session.map(&other).is_err());
    assert_eq!(session.last_analysis(&other), None);
    session.map(&granted).expect("map");
    assert_eq!(session.last_analysis(&other), None);
}

#[test]
fn a_failed_analysis_leaves_nothing_behind() {
    let fixture = Fixture::new("cache-failure");
    let repo = fixture.repo("granted");
    let session = session(vec![repo.clone()]);
    let good = answers(&session, &repo);

    // No supported source left: the analysis fails.
    let models = std::fs::read_to_string(repo.join("api/models.py")).expect("read");
    let routes = std::fs::read_to_string(repo.join("api/routes.py")).expect("read");
    std::fs::remove_file(repo.join("api/models.py")).expect("remove");
    std::fs::remove_file(repo.join("api/routes.py")).expect("remove");
    assert!(session.map(&repo).is_err(), "nothing to analyse");

    // The sources return. The next analysis starts cold and is correct.
    std::fs::write(repo.join("api/models.py"), models).expect("write");
    std::fs::write(repo.join("api/routes.py"), routes).expect("write");
    assert_eq!(answers(&session, &repo), good);
    assert_eq!(good, cold_answers(&repo));
}

#[test]
fn concurrent_questions_about_one_tree_all_get_the_cold_answer() {
    let fixture = Fixture::new("cache-concurrent");
    let repo = fixture.repo("granted");
    let expected = cold_answers(&repo);
    let session = std::sync::Arc::new(session(vec![repo.clone()]));

    let workers: Vec<_> = (0..8)
        .map(|_| {
            let session = std::sync::Arc::clone(&session);
            let repo = repo.clone();
            std::thread::spawn(move || answers(&session, &repo))
        })
        .collect();
    for worker in workers {
        assert_eq!(worker.join().expect("no panic"), expected);
    }
}

#[test]
fn a_diff_uses_the_cache_and_still_sees_an_edit() {
    let fixture = Fixture::new("cache-diff");
    let before = fixture.repo("before");
    let after = fixture.repo("after");
    let session = session(vec![before.clone(), after.clone()]);

    let same = session.diff(&before, &after).expect("diff");
    assert_eq!((same.added, same.removed, same.changed), (0, 0, 0));

    std::fs::write(
        after.join("api/routes.py"),
        "from flask import Flask\n\napp = Flask(__name__)\n\n@app.route('/api/orders')\ndef list_orders():\n    return []\n",
    )
    .expect("write");
    let edited = session.diff(&before, &after).expect("diff");
    assert!(edited.removed > 0, "the removed model access is reported");
    assert_eq!(session.last_analysis(&before).expect("analysed").parsed, 0);
}
