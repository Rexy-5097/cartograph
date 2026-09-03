//! The session authorization boundary over real repositories.
//!
//! Unit tests establish that the rules are right in the small, over paths that
//! never touch a disk. They cannot establish the property that matters most
//! here: that an **authorized** query actually reaches the analyser and
//! produces a graph, while an **unauthorized** one is refused without the
//! analyser ever opening a file — on trees that really exist, are really large,
//! and were not written for this test.
//!
//! Read-only: the analyser opens files and never writes, and no checkout is
//! mutated. Opt-in because this repository does not vendor the corpora.
//!
//! ```text
//! CARTOGRAPH_AUTH_REPO=<path> \
//! CARTOGRAPH_AUTH_BEFORE=<path> CARTOGRAPH_AUTH_AFTER=<path> \
//!     cargo test -p cartograph-pipeline --release --test authorization_real \
//!     -- --ignored --nocapture
//! ```
//!
//! `CARTOGRAPH_AUTH_BEFORE` and `CARTOGRAPH_AUTH_AFTER` are the two trees of a
//! diff — in Cartograph's own review workflow, a checkout of the base commit
//! and a tarball extraction of the head. Under ADR-0020 Amendment 1 the grant
//! is what says those two trees are one repository; nothing in the trees says
//! so, and this boundary does not pretend otherwise.

use std::path::{Path, PathBuf};

use cartograph_pipeline::authorization::{
    self, AuthorizationError, AuthorizedRepository, RepositoryIdentity,
};
use cartograph_pipeline::pipeline::Options;

fn env_path(name: &str) -> Option<PathBuf> {
    std::env::var(name).ok().map(PathBuf::from)
}

/// A session granted one identity and the trees named by the grant.
fn session(identity: &str, trees: Vec<PathBuf>) -> AuthorizedRepository {
    AuthorizedRepository::from_grant(
        RepositoryIdentity::from_grant(identity).expect("a non-empty identity"),
        trees,
    )
    .expect("a grant with at least one tree")
}

/// An authorized tree analyses; an unauthorized one is refused, and the refusal
/// is what answers rather than any failure from the analyser.
#[test]
#[ignore = "needs CARTOGRAPH_AUTH_REPO; the pinned corpora are not vendored"]
fn a_real_repository_analyses_only_when_the_session_granted_it() {
    let Some(repo) = env_path("CARTOGRAPH_AUTH_REPO") else {
        return;
    };

    let session = session("corpus-identity", vec![repo.clone()]);

    // Authorized: the guard admits it and the analyser runs.
    let admitted = session
        .authorize(&repo)
        .expect("the granted tree is admitted");
    assert_eq!(admitted.identity(), session.identity());

    let analysis =
        authorization::analyze(&admitted, Options { quiet: true }).expect("the corpus analyses");
    assert!(
        analysis.graph.node_count() > 0,
        "precondition: the corpus must produce a graph"
    );
    println!(
        "  authorized   -> {} nodes, {} edges",
        analysis.graph.node_count(),
        analysis.graph.edge_count()
    );

    // Unauthorized: a sibling of the corpus that certainly exists and is
    // certainly readable — its parent. If authorization were skipped the
    // analyser would happily walk it, which is exactly the outcome the
    // boundary exists to prevent.
    let parent = repo.parent().expect("the corpus has a parent directory");
    assert_eq!(
        session.authorize(parent).unwrap_err(),
        AuthorizationError::NotAuthorized,
        "a readable, existing, ungranted tree must still be refused"
    );
    println!("  unauthorized -> refused before analysis");
}

/// Both trees of a diff are admitted only when the grant associated both with
/// the one authorized identity, and the pair is decided before either analysis.
#[test]
#[ignore = "needs CARTOGRAPH_AUTH_BEFORE and CARTOGRAPH_AUTH_AFTER"]
fn a_real_two_tree_diff_is_authorized_as_one_repository() {
    let (Some(before), Some(after)) = (
        env_path("CARTOGRAPH_AUTH_BEFORE"),
        env_path("CARTOGRAPH_AUTH_AFTER"),
    ) else {
        return;
    };

    // The grant says these two trees are one repository. That claim comes from
    // whoever fetched them, not from the trees.
    let granted = session("one-repository", vec![before.clone(), after.clone()]);

    let pair = granted
        .authorize_pair(&before, &after)
        .expect("both trees were granted");
    assert_eq!(
        pair.0.identity(),
        pair.1.identity(),
        "a diff pair must carry one identity"
    );

    let (left, right) =
        authorization::analyze_pair(&pair, Options { quiet: true }).expect("both trees analyse");
    assert!(left.graph.node_count() > 0 && right.graph.node_count() > 0);
    println!(
        "  same identity     -> before {} nodes, after {} nodes",
        left.graph.node_count(),
        right.graph.node_count()
    );

    // Repository A as `before`, repository B as `after`. B is a real, readable
    // tree that this session was never granted, so the pair is refused whole.
    let only_before = session("one-repository", vec![before.clone()]);
    assert_eq!(
        only_before.authorize_pair(&before, &after).unwrap_err(),
        AuthorizationError::NotAuthorized,
        "an ungranted `after` must refuse the whole pair"
    );
    assert_eq!(
        only_before.authorize_pair(&after, &before).unwrap_err(),
        AuthorizationError::NotAuthorized,
        "...in either position"
    );
    println!("  different repos   -> refused before either analysis");
}

/// A tree granted to one session is not thereby granted to another, even when
/// both trees are real and both sessions are valid.
#[test]
#[ignore = "needs CARTOGRAPH_AUTH_BEFORE and CARTOGRAPH_AUTH_AFTER"]
fn one_session_cannot_reach_another_sessions_repository() {
    let (Some(before), Some(after)) = (
        env_path("CARTOGRAPH_AUTH_BEFORE"),
        env_path("CARTOGRAPH_AUTH_AFTER"),
    ) else {
        return;
    };

    let first = session("repository-one", vec![before.clone()]);
    let second = session("repository-two", vec![after.clone()]);

    assert_eq!(
        first.authorize(&after).unwrap_err(),
        AuthorizationError::NotAuthorized
    );
    assert_eq!(
        second.authorize(&before).unwrap_err(),
        AuthorizationError::NotAuthorized
    );
    assert_ne!(
        first.authorize(&before).expect("granted").identity(),
        second.authorize(&after).expect("granted").identity(),
    );
    println!("  cross-session     -> refused in both directions");
}

/// A refusal about a real, rooted corpus path must not name it.
#[test]
#[ignore = "needs CARTOGRAPH_AUTH_REPO; the pinned corpora are not vendored"]
fn a_refusal_about_a_real_path_leaks_nothing_about_it() {
    let Some(repo) = env_path("CARTOGRAPH_AUTH_REPO") else {
        return;
    };

    let session = session("corpus-identity", vec![PathBuf::from("granted-elsewhere")]);
    let refusal = session
        .authorize(&repo)
        .expect_err("the corpus was not granted to this session");

    let rendered = format!("{refusal}");
    let debugged = format!("{refusal:?}");
    let text = format!("{rendered}{debugged}");

    // Nothing from the real path, and none of its ancestors, may appear.
    let mut checked = 0usize;
    for ancestor in repo.ancestors() {
        if let Some(name) = ancestor.file_name().and_then(|n| n.to_str()) {
            assert!(
                !text.contains(name),
                "a refusal leaked the path component {name:?}: {text}"
            );
            checked += 1;
        }
    }
    assert!(
        !text.contains(&repo.to_string_lossy().to_string()),
        "a refusal leaked the whole path"
    );
    assert!(checked > 0, "precondition: the corpus path has components");
    println!("  refusal text      -> leaked none of {checked} path components");
}

/// The corpus is never mutated: authorization touches no filesystem at all, so
/// a refusal cannot even reveal whether a path exists.
#[test]
#[ignore = "needs CARTOGRAPH_AUTH_REPO; the pinned corpora are not vendored"]
fn refusing_touches_no_filesystem() {
    let Some(repo) = env_path("CARTOGRAPH_AUTH_REPO") else {
        return;
    };

    let session = session("corpus-identity", vec![PathBuf::from("granted-elsewhere")]);

    // An existing tree and a path that cannot exist are refused identically.
    let existing = session.authorize(&repo).unwrap_err();
    let missing = session
        .authorize(Path::new("no/such/directory/anywhere"))
        .unwrap_err();

    assert_eq!(existing, missing, "a refusal must not distinguish the two");
    println!("  existence         -> indistinguishable in a refusal");
}
