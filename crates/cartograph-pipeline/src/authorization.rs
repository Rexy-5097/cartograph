//! The session authorization boundary: one repository, refused before analysis.
//!
//! # The invariant this exists to enforce
//!
//! > One MCP session has exactly one authorized repository identity. Every
//! > graph query is bound to that identity. A query naming any other
//! > repository is refused **before any analysis runs**.
//!
//! That sentence is [ADR-0005](../../../docs/adr/ADR-0005-local-first-privacy.md),
//! `ARCHITECTURE.md` and `SECURITY.md` speaking with one voice, and
//! [ADR-0020](../../../docs/adr/ADR-0020-mcp-boundary-and-authorization.md)
//! makes it M15's. The third clause is the load-bearing one: a refusal that
//! arrives after the analyser has walked a tree has already read the thing it
//! was meant to protect.
//!
//! # Where the identity comes from, and where it does not
//!
//! **The identity is supplied by whoever establishes the session. It is never
//! derived from the tree being analysed.** ADR-0020 Amendment 1 records that
//! decision (option R2) and the investigation that forced it: no derivation
//! from the tree can work.
//!
//! - A **filesystem path** identifies a location on this machine. Two clones of
//!   one repository are two paths; one repository moved is a new path.
//! - A **Git remote** is optional and mutable, and Cartograph's own review
//!   workflow compares a base checkout of `github.repository` against a
//!   *tarball of a fork* — two different remotes for one pull request's
//!   before-and-after. Remote identity would refuse the review M14 was accepted
//!   on.
//! - A **commit** identifies a revision. Two revisions of one repository differ,
//!   which is precisely the case a diff must accept.
//! - A **content hash** identifies content, and changes on every edit.
//!
//! The knowledge that two trees are one repository lives in the component that
//! fetched them — in M14's workflow, the workflow itself, which knows
//! `github.repository`, both SHAs and the head repository, and then hands the
//! analyser two paths and nothing else. So the identity is put where the
//! knowledge already is.
//!
//! # The trust boundary, stated rather than implied
//!
//! **The component that establishes the session is part of the trust
//! boundary.** It asserts the identity and which trees carry it. This module
//! does not verify that assertion and cannot: there is nothing in a directory
//! to verify it against.
//!
//! What that buys, and what it does not:
//!
//! - **Defends against** a client widening its own scope. A query presents a
//!   tree; the session decides whether that tree was granted, and refuses
//!   before any analysis.
//! - **Does not defend against** a mistaken or compromised grant producer. It
//!   can authorize anything — but it is the process that started the server and
//!   could have read those files directly. The guard stops the *client*, not
//!   its own launcher.
//!
//! This is not "a validated repository", not "a secure path" and not "a
//! canonical path". Those are different properties, and two of them are the
//! ones Amendment 1 rejected.
//!
//! # Deliberately not decided here
//!
//! The **identity value** and the **grant mechanism** are both still open in
//! ADR-0020, and nothing in this module may be read as settling either.
//! [`RepositoryIdentity`] is opaque on purpose: it carries whatever the granter
//! supplied and this crate never interprets it, so choosing a derivation later
//! changes the granter and not this boundary.

use std::path::{Path, PathBuf};

use crate::error::CliError;
use crate::pipeline::{self, Analysis, Options};

/// A repository identity, as supplied by the session grant.
///
/// Opaque by construction. There is no accessor returning the inner value and
/// no `Display`: the granter chooses what an identity *is*, and a value this
/// crate never interprets is a value it cannot leak into a message or a log
/// (RULE 015). `Debug` redacts for the same reason — the same discipline
/// `NodeKind::EnvVar` applies by having no value field at all.
///
/// Equality is what the boundary needs, and equality is all it exposes.
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct RepositoryIdentity(String);

impl RepositoryIdentity {
    /// Takes an identity from the session grant.
    ///
    /// The value is not interpreted, canonicalised, hashed or checked against
    /// the filesystem. It is a token whose meaning belongs to the granter.
    ///
    /// # Errors
    ///
    /// [`AuthorizationError::EmptyIdentity`] if the value is empty or only
    /// whitespace. An empty identity would compare equal to another empty one,
    /// which would silently merge two sessions.
    pub fn from_grant(value: impl Into<String>) -> Result<Self, AuthorizationError> {
        let value = value.into();
        if value.trim().is_empty() {
            return Err(AuthorizationError::EmptyIdentity);
        }
        Ok(Self(value))
    }
}

impl std::fmt::Debug for RepositoryIdentity {
    /// Redacted. An identity may contain anything the granter chose, including
    /// a path, so it is never printed — not in a panic, not in a test failure,
    /// not in a log.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("RepositoryIdentity(<redacted>)")
    }
}

/// Why a request was refused.
///
/// Deliberately **not** an [`crate::error::ErrorCode`]: that enum is the CLI's
/// `--json` contract and its documented exit codes, and widening it would
/// change a published surface for a boundary no client speaks yet. Mapping a
/// refusal onto a wire error belongs to the transport slice (ADR-0020).
///
/// No variant carries a path, an identity or any part of either. A refusal
/// says *that* something was refused, never *what* was asked for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthorizationError {
    /// The grant supplied an empty identity.
    EmptyIdentity,
    /// The grant associated no trees with the identity, so no query could ever
    /// be authorized and the session is unusable rather than permissive.
    EmptyGrant,
    /// The tree named by the query was not granted for this session.
    NotAuthorized,
    /// Two trees of one query do not carry the same authorized identity.
    ///
    /// Structurally unreachable while a session holds exactly one identity, and
    /// checked anyway: the diff rule is the reason this boundary exists, and a
    /// rule enforced only by an invariant elsewhere is a rule that survives
    /// until someone changes the elsewhere.
    IdentityMismatch,
}

impl AuthorizationError {
    /// A message safe to show anywhere.
    ///
    /// Fixed strings. Nothing is interpolated, so no path, parent directory,
    /// identity or environment value can reach a caller through a refusal.
    #[must_use]
    pub const fn message(self) -> &'static str {
        match self {
            Self::EmptyIdentity => "the session grant supplied no repository identity",
            Self::EmptyGrant => "the session grant associated no repository tree with its identity",
            Self::NotAuthorized => {
                "that repository is not the one authorized for this session, so it was not analysed"
            }
            Self::IdentityMismatch => {
                "the two trees are not the same authorized repository, so no diff was run"
            }
        }
    }
}

impl std::fmt::Display for AuthorizationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.message())
    }
}

impl std::error::Error for AuthorizationError {}

/// One session's authorization state: one identity, and the trees the grant
/// said represent it.
///
/// # Why the trees are a set the grant supplies
///
/// A query names a tree. Deciding whether that tree *is* the authorized
/// repository is exactly the derivation ADR-0020 leaves open, so this boundary
/// does not attempt it: the grant lists the trees, and a query is admitted only
/// if it names one of them.
///
/// # Why matching is exact
///
/// Paths are compared as granted, with no normalisation and **no filesystem
/// access**. Two consequences, both deliberate:
///
/// - a different spelling of a granted path — `./x` for `x`, a symlink into the
///   tree, a different case on Windows — is **refused**, not accepted. Any
///   smarter matching is a claim about when two paths are the same repository,
///   which is the deferred derivation wearing a disguise. Failing closed is the
///   safe direction while it is undecided;
/// - because nothing is resolved against the filesystem, there is no
///   time-of-check-to-time-of-use window in the authorization decision itself.
///
/// Slice 3's grant mechanism decides what paths it hands out, and a client
/// echoes back what it was given.
#[derive(Debug, Clone)]
pub struct AuthorizedRepository {
    identity: RepositoryIdentity,
    trees: Vec<PathBuf>,
}

impl AuthorizedRepository {
    /// Builds the session's authorization state from a grant.
    ///
    /// # Errors
    ///
    /// [`AuthorizationError::EmptyGrant`] when no tree is associated with the
    /// identity.
    pub fn from_grant(
        identity: RepositoryIdentity,
        trees: impl IntoIterator<Item = PathBuf>,
    ) -> Result<Self, AuthorizationError> {
        let trees: Vec<PathBuf> = trees.into_iter().collect();
        if trees.is_empty() {
            return Err(AuthorizationError::EmptyGrant);
        }
        Ok(Self { identity, trees })
    }

    /// The identity this session is bound to.
    #[must_use]
    pub const fn identity(&self) -> &RepositoryIdentity {
        &self.identity
    }

    /// How many trees the grant associated with the identity.
    ///
    /// The count, not the paths: a caller that wants to know whether a
    /// particular tree is authorized should ask by presenting it.
    #[must_use]
    pub fn tree_count(&self) -> usize {
        self.trees.len()
    }

    /// Admits one tree, or refuses.
    ///
    /// # Errors
    ///
    /// [`AuthorizationError::NotAuthorized`] when the tree was not granted for
    /// this session. Nothing on disk is touched, so a refusal costs no I/O and
    /// reveals nothing about whether the path exists.
    pub fn authorize(&self, tree: &Path) -> Result<AuthorizedTree, AuthorizationError> {
        if self.trees.iter().any(|granted| granted == tree) {
            Ok(AuthorizedTree {
                path: tree.to_path_buf(),
                identity: self.identity.clone(),
            })
        } else {
            Err(AuthorizationError::NotAuthorized)
        }
    }

    /// Admits a pair of trees for a two-tree query, or refuses.
    ///
    /// **Both** are decided before either is returned, so a diff cannot analyse
    /// its `before` while its `after` is still unauthorized. `before` from
    /// repository A with `after` from repository B is refused because B was
    /// never granted to this session.
    ///
    /// # Errors
    ///
    /// [`AuthorizationError::NotAuthorized`] when either tree was not granted;
    /// [`AuthorizationError::IdentityMismatch`] if two admitted trees somehow
    /// carry different identities.
    pub fn authorize_pair(
        &self,
        before: &Path,
        after: &Path,
    ) -> Result<(AuthorizedTree, AuthorizedTree), AuthorizationError> {
        let before = self.authorize(before)?;
        let after = self.authorize(after)?;
        if before.identity != after.identity {
            return Err(AuthorizationError::IdentityMismatch);
        }
        Ok((before, after))
    }
}

/// Proof that a tree was admitted for a session's authorized identity.
///
/// Constructing one is the **only** way to reach [`analyze`], so an
/// unauthorized path cannot reach the analyser by mistake — the same technique
/// `cartograph_desktop::repository::ValidatedRepository` uses to keep an
/// unvalidated path away from `session::analyze`, applied to a different
/// property. Its fields are private and it has no public constructor: it can
/// only come from [`AuthorizedRepository::authorize`] or
/// [`AuthorizedRepository::authorize_pair`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthorizedTree {
    path: PathBuf,
    identity: RepositoryIdentity,
}

impl AuthorizedTree {
    /// The identity this tree was admitted under.
    #[must_use]
    pub const fn identity(&self) -> &RepositoryIdentity {
        &self.identity
    }
}

/// Analyses a tree that authorization has already admitted.
///
/// The signature is the boundary: there is no way to call this without an
/// [`AuthorizedTree`], and no way to obtain one except through a session's
/// grant. [`crate::pipeline::run`] itself is unchanged and still public — the
/// CLI and the desktop client call it directly, as they always have, and
/// neither is session-scoped. This is the entry point a session-scoped client
/// uses.
///
/// # Errors
///
/// Whatever the pipeline could not do; see [`crate::pipeline::run`].
pub fn analyze(tree: &AuthorizedTree, options: Options) -> Result<Analysis, CliError> {
    pipeline::run(&tree.path, options)
}

/// Analyses both trees of a two-tree query.
///
/// Takes the pair by value from [`AuthorizedRepository::authorize_pair`], so
/// the ordering that matters — both authorized, *then* either analysed — is
/// expressed in the types rather than trusted to a caller.
///
/// # Errors
///
/// Whatever the pipeline could not do, for whichever tree failed first.
pub fn analyze_pair(
    pair: &(AuthorizedTree, AuthorizedTree),
    options: Options,
) -> Result<(Analysis, Analysis), CliError> {
    let before = analyze(&pair.0, options)?;
    let after = analyze(&pair.1, options)?;
    Ok((before, after))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn identity(value: &str) -> RepositoryIdentity {
        RepositoryIdentity::from_grant(value).expect("a non-empty identity")
    }

    fn session(trees: &[&str]) -> AuthorizedRepository {
        AuthorizedRepository::from_grant(
            identity("granted-repository"),
            trees.iter().map(PathBuf::from),
        )
        .expect("a grant with at least one tree")
    }

    // ── the grant ───────────────────────────────────────────────────

    #[test]
    fn an_empty_identity_is_refused() {
        assert_eq!(
            RepositoryIdentity::from_grant("").unwrap_err(),
            AuthorizationError::EmptyIdentity
        );
        assert_eq!(
            RepositoryIdentity::from_grant("   ").unwrap_err(),
            AuthorizationError::EmptyIdentity
        );
    }

    /// A grant with no tree would authorize nothing. It is refused at
    /// construction rather than becoming a session that says no to everything
    /// for a reason nobody can see.
    #[test]
    fn a_grant_with_no_tree_is_refused() {
        assert_eq!(
            AuthorizedRepository::from_grant(identity("r"), Vec::new()).unwrap_err(),
            AuthorizationError::EmptyGrant
        );
    }

    /// The value is the granter's, not this crate's: it is stored and compared,
    /// never parsed.
    #[test]
    fn an_identity_is_opaque_and_compares_by_value() {
        assert_eq!(identity("abc"), identity("abc"));
        assert_ne!(identity("abc"), identity("abd"));
        // Anything non-empty is acceptable, because derivation is deferred.
        for value in ["a", "sha256:0000", "github.com/o/r", "{\"any\":\"shape\"}"] {
            assert!(RepositoryIdentity::from_grant(value).is_ok());
        }
    }

    // ── one session, one repository ─────────────────────────────────

    #[test]
    fn the_authorized_tree_is_accepted() {
        let session = session(&["repo"]);
        let admitted = session.authorize(Path::new("repo")).expect("authorized");
        assert_eq!(admitted.identity(), session.identity());
    }

    #[test]
    fn a_tree_the_grant_did_not_name_is_refused() {
        let session = session(&["repo"]);
        assert_eq!(
            session.authorize(Path::new("other-repo")).unwrap_err(),
            AuthorizationError::NotAuthorized
        );
    }

    /// The session holds one identity, and it is the one every admitted tree
    /// carries — there is no second identity for a query to be admitted under.
    #[test]
    fn a_session_holds_exactly_one_identity() {
        let session = session(&["before", "after"]);
        let before = session.authorize(Path::new("before")).expect("authorized");
        let after = session.authorize(Path::new("after")).expect("authorized");

        assert_eq!(before.identity(), after.identity());
        assert_eq!(before.identity(), session.identity());
        assert_eq!(session.tree_count(), 2);
    }

    /// A tree granted in one session is not thereby granted in another.
    #[test]
    fn a_tree_granted_to_one_session_is_refused_by_another() {
        let a = AuthorizedRepository::from_grant(identity("repo-a"), [PathBuf::from("tree-a")])
            .expect("grant");
        let b = AuthorizedRepository::from_grant(identity("repo-b"), [PathBuf::from("tree-b")])
            .expect("grant");

        assert_eq!(
            b.authorize(Path::new("tree-a")).unwrap_err(),
            AuthorizationError::NotAuthorized
        );
        assert_ne!(
            a.authorize(Path::new("tree-a"))
                .expect("authorized")
                .identity(),
            b.authorize(Path::new("tree-b"))
                .expect("authorized")
                .identity(),
        );
    }

    // ── the diff rule ───────────────────────────────────────────────

    #[test]
    fn a_pair_from_one_authorized_repository_is_accepted() {
        let session = session(&["before", "after"]);
        let (before, after) = session
            .authorize_pair(Path::new("before"), Path::new("after"))
            .expect("both authorized");
        assert_eq!(before.identity(), after.identity());
    }

    /// Repository A as `before` with repository B as `after`. B was never
    /// granted, so the pair is refused — and refused *whole*, not half-run.
    #[test]
    fn a_pair_naming_an_unauthorized_repository_is_refused() {
        let session = session(&["before"]);
        assert_eq!(
            session
                .authorize_pair(Path::new("before"), Path::new("someone-elses-repo"))
                .unwrap_err(),
            AuthorizationError::NotAuthorized
        );
        // ...in either position.
        assert_eq!(
            session
                .authorize_pair(Path::new("someone-elses-repo"), Path::new("before"))
                .unwrap_err(),
            AuthorizationError::NotAuthorized
        );
    }

    /// Both sides are decided before either is handed back, so a diff cannot
    /// analyse its `before` while its `after` is still unauthorized.
    #[test]
    fn a_refused_pair_yields_no_authorized_tree_at_all() {
        let session = session(&["before"]);
        assert!(
            session
                .authorize_pair(Path::new("before"), Path::new("elsewhere"))
                .is_err()
        );
    }

    // ── refusal precedes analysis ───────────────────────────────────

    /// The proof needs no instrumentation: the unauthorized path does not
    /// exist, so if the analyser ran at all the failure would be the
    /// pipeline's `InvalidPath`. It is `NotAuthorized`, so nothing was read.
    #[test]
    fn an_unauthorized_query_is_refused_before_the_analyser_touches_the_disk() {
        let session = session(&["repo"]);
        let missing = Path::new("no/such/directory/anywhere");

        assert_eq!(
            session.authorize(missing).unwrap_err(),
            AuthorizationError::NotAuthorized
        );
    }

    /// Same argument for the pair: the second tree is missing *and*
    /// unauthorized, and authorization is what answers.
    #[test]
    fn an_unauthorized_pair_is_refused_before_either_analysis() {
        let session = session(&["repo"]);
        assert_eq!(
            session
                .authorize_pair(Path::new("repo"), Path::new("no/such/directory"))
                .unwrap_err(),
            AuthorizationError::NotAuthorized
        );
    }

    // ── the boundary cannot be walked around ────────────────────────

    /// `analyze` takes an `AuthorizedTree`, whose fields are private and which
    /// has no public constructor. The only way to obtain one is through a
    /// grant, which is the whole point; this test records the property that
    /// the type system enforces, so removing it would be a visible change.
    #[test]
    fn an_authorized_tree_can_only_come_from_a_grant() {
        let session = session(&["repo"]);
        let admitted = session.authorize(Path::new("repo")).expect("authorized");
        // Reachable only because `session.authorize` returned it.
        assert_eq!(admitted.identity(), session.identity());
    }

    // ── determinism and privacy ─────────────────────────────────────

    #[test]
    fn the_same_question_gets_the_same_answer_every_time() {
        let session = session(&["repo"]);
        for _ in 0..5 {
            assert!(session.authorize(Path::new("repo")).is_ok());
            assert_eq!(
                session.authorize(Path::new("elsewhere")).unwrap_err(),
                AuthorizationError::NotAuthorized
            );
        }
    }

    /// A refusal must not name what was refused. Rooted paths, their parents,
    /// and the identity itself all stay out of the message (RULE 015).
    #[test]
    fn a_refusal_names_neither_the_path_nor_its_parents() {
        let secret = if cfg!(windows) {
            r"C:\workspace\clients\acme-secret\repo"
        } else {
            // `username` is the placeholder form QG-005 allows; a real
            // account name in a tracked file is the thing that gate exists
            // to catch, and this test must not become the exception.
            "/home/username/clients/acme-secret/repo"
        };
        let session = AuthorizedRepository::from_grant(
            identity("s3cr3t-identity-value"),
            [PathBuf::from("granted")],
        )
        .expect("grant");

        let refusal = session.authorize(Path::new(secret)).unwrap_err();
        let rendered = format!("{refusal}");
        let debugged = format!("{refusal:?}");

        for text in [&rendered, &debugged] {
            for leak in [
                secret,
                "acme-secret",
                "clients",
                "workspace",
                "username",
                "s3cr3t-identity-value",
                "C:\\",
                "/home/",
            ] {
                assert!(!text.contains(leak), "a refusal leaked {leak:?}: {text}");
            }
        }
    }

    /// An identity may hold anything the granter chose, including a path, so it
    /// is never printed.
    #[test]
    fn an_identity_never_prints_its_value() {
        let id = identity("/home/username/private-repo");
        let debugged = format!("{id:?}");

        assert_eq!(debugged, "RepositoryIdentity(<redacted>)");
        assert!(!debugged.contains("private-repo"));
        assert!(!debugged.contains("username"));
    }

    /// The session itself is `Debug` for diagnostics, and must not become a way
    /// to print the identity either.
    #[test]
    fn a_session_never_prints_its_identity() {
        let session = AuthorizedRepository::from_grant(
            identity("s3cr3t-identity-value"),
            [PathBuf::from("granted")],
        )
        .expect("grant");

        let debugged = format!("{session:?}");
        assert!(!debugged.contains("s3cr3t-identity-value"), "{debugged}");
    }
}
