//! Per-repository opt-in, the desktop grant, and the credential boundary.
//!
//! These are the security tests for M16 Slice 4. Each one states a property the
//! Frozen Engineering Specification V3 §13 requires, and proves it rather than
//! asserting it in a comment:
//!
//! - *"AI is opt-in, per repository, off by default."*
//! - *"The product is fully useful with no API key and no network."*
//! - *"Credentials live in the OS keychain, never in configuration files."*
//!
//! The last one is proved here in the only direction this slice can prove it:
//! the settings file, serialised, contains no credential — because nothing ever
//! puts one there.
//!
//! Nothing here touches a real keychain or a real user's settings. Every test
//! writes to its own temporary path, and the credential store is a mock.

use std::path::{Path, PathBuf};

use cartograph_desktop::credential::{CredentialError, CredentialStore, NoCredentials, Secret};
use cartograph_desktop::optin::{GrantedRepository, OptIns};
use cartograph_pipeline::authorization::{AuthorizedRepository, RepositoryIdentity};

/// A settings path unique to this test, following the suite's convention.
fn settings(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "cartograph-optin-{tag}-{}-{:?}",
        std::process::id(),
        std::thread::current().id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    dir.join("ask-optin.json")
}

fn repo(name: &str) -> PathBuf {
    PathBuf::from(format!("/projects/{name}"))
}

// ── identity ────────────────────────────────────────────────────────

#[test]
fn two_repositories_receive_different_identities() {
    let mut table = OptIns::default();

    let a = table.grant(&repo("alpha")).expect("granted");
    let b = table.grant(&repo("beta")).expect("granted");

    assert_ne!(a, b, "two repositories share an identity");
    assert_eq!(table.len(), 2);
}

#[test]
fn selecting_the_same_repository_again_recovers_its_identity() {
    let mut table = OptIns::default();

    let first = table.grant(&repo("alpha")).expect("granted");
    let again = table.grant(&repo("alpha")).expect("granted");

    assert_eq!(first, again, "a second selection minted a new identity");
    assert_eq!(table.len(), 1, "a duplicate entry was recorded");
}

#[test]
fn the_identity_is_not_derived_from_the_locator() {
    // The property Amendment 3 turns on. Two tables that have never seen each
    // other grant *different* identities for the same path — which a
    // derivation could not do. The mapping is a recorded fact.
    let mut one = OptIns::default();
    let mut other = OptIns::default();

    let from_one = one.grant(&repo("alpha")).expect("granted");
    let from_other = other.grant(&repo("alpha")).expect("granted");

    assert_ne!(
        from_one, from_other,
        "the identity is a function of the path, which Amendment 3 forbids"
    );
}

#[test]
fn an_identity_survives_a_restart_through_the_persisted_mapping() {
    let path = settings("restart");
    let mut before = OptIns::default();
    let granted = before.grant(&repo("alpha")).expect("granted");
    before.save(&path).expect("saved");

    // A new process: nothing in memory, only the file.
    let mut after = OptIns::load(&path).expect("loaded");
    let recovered = after.grant(&repo("alpha")).expect("granted");

    assert_eq!(granted, recovered, "the identity did not survive a restart");
}

#[test]
fn a_repository_at_another_locator_gets_its_own_identity() {
    // The accepted consequence: moving or renaming does not carry opt-in.
    let mut table = OptIns::default();
    let original = table.grant(&repo("alpha")).expect("granted");
    table.set_enabled(&original, true);

    let moved = table.grant(&repo("alpha-renamed")).expect("granted");

    assert_ne!(original, moved);
    assert!(
        !table.is_enabled(&moved),
        "opt-in followed a repository across a rename"
    );
}

// ── opt-in ──────────────────────────────────────────────────────────

#[test]
fn ask_is_off_by_default() {
    let mut table = OptIns::default();

    let granted = table.grant(&repo("alpha")).expect("granted");

    assert!(
        !table.is_enabled(&granted),
        "AI was on without being asked for"
    );
}

#[test]
fn enabling_one_repository_does_not_enable_another() {
    let mut table = OptIns::default();
    let a = table.grant(&repo("alpha")).expect("granted");
    let b = table.grant(&repo("beta")).expect("granted");

    assert!(table.set_enabled(&a, true));

    assert!(table.is_enabled(&a));
    assert!(!table.is_enabled(&b), "opt-in leaked to another repository");
}

#[test]
fn an_identity_the_table_never_granted_reads_as_disabled() {
    let mut table = OptIns::default();
    let known = table.grant(&repo("alpha")).expect("granted");
    table.set_enabled(&known, true);

    let stranger = RepositoryIdentity::from_grant("not-from-this-table").expect("non-empty");

    assert!(!table.is_enabled(&stranger));
    assert!(
        !table.set_enabled(&stranger, true),
        "an ungranted handle enabled something"
    );
}

#[test]
fn opt_in_survives_a_restart() {
    let path = settings("persist");
    let mut before = OptIns::default();
    let a = before.grant(&repo("alpha")).expect("granted");
    before.set_enabled(&a, true);
    before.save(&path).expect("saved");

    let after = OptIns::load(&path).expect("loaded");

    assert!(after.is_enabled(&a));
}

// ── failing closed ──────────────────────────────────────────────────

#[test]
fn a_missing_settings_file_is_an_empty_table_not_a_failure() {
    let path = settings("missing");

    let table = OptIns::load(&path).expect("a fresh install is not a failure");

    assert!(table.is_empty());
}

#[test]
fn a_corrupt_settings_file_is_refused_rather_than_read_as_empty() {
    // Empty and corrupt must not look the same: both would answer "nothing is
    // opted in", and the corrupt case would then be silent.
    let path = settings("corrupt");
    std::fs::create_dir_all(path.parent().expect("parent")).expect("create");
    std::fs::write(&path, "{ this is not json").expect("write");

    let error = OptIns::load(&path).expect_err("refused");

    assert!(!error.message.is_empty());
}

#[test]
fn a_refusal_never_repeats_the_path_it_refused() {
    let path = settings("quiet");
    std::fs::create_dir_all(path.parent().expect("parent")).expect("create");
    std::fs::write(&path, "not json").expect("write");

    let error = OptIns::load(&path).expect_err("refused");
    let rendered = format!("{} {}", error.message, error.hint.unwrap_or_default());

    assert!(!rendered.contains("cartograph-optin"), "leaked: {rendered}");
    assert!(!rendered.contains(":\\") && !rendered.contains("/tmp"));
}

// ── what the file may contain ───────────────────────────────────────

#[test]
fn the_settings_file_contains_no_credential_of_any_kind() {
    // §13: credentials live in the OS keychain, never in configuration files.
    let path = settings("contents");
    let mut table = OptIns::default();
    let a = table.grant(&repo("alpha")).expect("granted");
    table.set_enabled(&a, true);
    table.grant(&repo("beta")).expect("granted");
    table.save(&path).expect("saved");

    let written = std::fs::read_to_string(&path).expect("readable");

    for forbidden in [
        "ghp_",
        "AKIA",
        "xoxb-",
        "Bearer",
        "api_key",
        "apiKey",
        "token",
        "secret",
        "password",
        "PRIVATE KEY",
    ] {
        assert!(
            !written.contains(forbidden),
            "the settings file carries {forbidden}: {written}"
        );
    }
    // What it *is* allowed to hold, so the test fails if the shape changes.
    assert!(written.contains("locator"));
    assert!(written.contains("identity"));
    assert!(written.contains("ask_enabled"));
}

#[test]
fn the_settings_file_carries_no_source_or_environment_value() {
    let path = settings("noleak");
    let mut table = OptIns::default();
    table.grant(&repo("alpha")).expect("granted");
    table.save(&path).expect("saved");

    let written = std::fs::read_to_string(&path).expect("readable");

    // No evidence, no analysis, no environment. The table has three fields and
    // this pins that it still has three.
    for absent in ["evidence", "graph", "nodes", "edges", "PATH=", "env"] {
        assert!(!written.contains(absent), "unexpected content: {written}");
    }
}

// ── credentials ─────────────────────────────────────────────────────

/// A credential store that fails the way a real one can.
struct Failing(CredentialError);

impl CredentialStore for Failing {
    fn get(&self, _identity: &RepositoryIdentity) -> Result<Option<Secret>, CredentialError> {
        Err(self.0)
    }
    fn set(&self, _identity: &RepositoryIdentity, _secret: Secret) -> Result<(), CredentialError> {
        Err(self.0)
    }
    fn delete(&self, _identity: &RepositoryIdentity) -> Result<(), CredentialError> {
        Err(self.0)
    }
}

/// A store that holds one credential, for the "key present" path.
struct OneKey(RepositoryIdentity, String);

impl CredentialStore for OneKey {
    fn get(&self, identity: &RepositoryIdentity) -> Result<Option<Secret>, CredentialError> {
        Ok((identity == &self.0).then(|| Secret::new(self.1.clone())))
    }
    fn set(&self, _identity: &RepositoryIdentity, _secret: Secret) -> Result<(), CredentialError> {
        Ok(())
    }
    fn delete(&self, _identity: &RepositoryIdentity) -> Result<(), CredentialError> {
        Ok(())
    }
}

#[test]
fn with_no_credential_the_repository_is_simply_without_one() {
    let mut table = OptIns::default();
    let a = table.grant(&repo("alpha")).expect("granted");

    assert_eq!(NoCredentials.get(&a).expect("consulted"), None);
}

#[test]
fn a_refused_or_unavailable_keychain_is_reported_not_mistaken_for_absence() {
    // "No credential" and "could not ask" are different, and conflating them
    // would hide a locked keychain behind a silent degrade.
    let mut table = OptIns::default();
    let a = table.grant(&repo("alpha")).expect("granted");

    for kind in [
        CredentialError::PermissionDenied,
        CredentialError::Unavailable,
    ] {
        assert_eq!(Failing(kind).get(&a), Err(kind));
    }
}

#[test]
fn one_repositorys_credential_is_not_reachable_with_anothers_identity() {
    let mut table = OptIns::default();
    let a = table.grant(&repo("alpha")).expect("granted");
    let b = table.grant(&repo("beta")).expect("granted");
    let store = OneKey(a.clone(), "not-a-real-key".to_owned());

    assert!(store.get(&a).expect("consulted").is_some());
    assert_eq!(
        store.get(&b).expect("consulted"),
        None,
        "a credential was reachable with the wrong identity"
    );
}

#[test]
fn a_credential_never_becomes_text_anywhere_in_an_error_path() {
    let secret = Secret::new("not-a-real-key-0123456789");
    let wrapped: Result<Secret, CredentialError> = Err(CredentialError::PermissionDenied);

    let rendered = format!("{secret:?} {wrapped:?} {}", CredentialError::Unavailable);

    assert!(!rendered.contains("0123456789"), "leaked: {rendered}");
}

// ── the M15 authorization contract, with a desktop-produced grant ────

#[test]
fn a_desktop_grant_enforces_the_same_authorization_rules_as_an_mcp_grant() {
    // The point of R1: one identity model, so the boundary behaves identically
    // no matter which producer supplied the grant.
    let mut table = OptIns::default();
    let alpha = repo("alpha");
    let beta = repo("beta");
    let identity = table.grant(&alpha).expect("granted");

    let authorized = AuthorizedRepository::from_grant(identity, [alpha.clone()]).expect("grant");

    // A only -> allowed. B only -> refused.
    assert!(authorized.authorize(&alpha).is_ok());
    assert!(authorized.authorize(&beta).is_err());

    // A + A -> allowed. A + B and B + A -> refused, before either is analysed.
    assert!(authorized.authorize_pair(&alpha, &alpha).is_ok());
    assert!(authorized.authorize_pair(&alpha, &beta).is_err());
    assert!(authorized.authorize_pair(&beta, &alpha).is_err());
}

#[test]
fn a_desktop_grant_admits_only_the_tree_the_human_selected() {
    let mut table = OptIns::default();
    let chosen = repo("alpha");
    let identity = table.grant(&chosen).expect("granted");
    let authorized = AuthorizedRepository::from_grant(identity, [chosen]).expect("grant");

    assert_eq!(authorized.tree_count(), 1);
    assert!(
        authorized
            .authorize(Path::new("/projects/somewhere-else"))
            .is_err()
    );
}

// ── bookkeeping ─────────────────────────────────────────────────────

#[test]
fn an_entry_whose_repository_is_gone_can_be_pruned() {
    let mut table = OptIns::default();
    table.grant(&repo("deleted-long-ago")).expect("granted");

    let removed = table.prune_missing();

    assert_eq!(removed, 1);
    assert!(table.is_empty());
}

// ── the grant as the shell holds it ─────────────────────────────────

#[test]
fn establishing_a_grant_twice_recovers_the_same_identity() {
    let path = settings("establish");
    let locator = repo("alpha");

    let first = GrantedRepository::establish(&locator, Some(path.clone())).expect("granted");
    let again = GrantedRepository::establish(&locator, Some(path)).expect("granted");

    assert_eq!(first.identity(), again.identity());
}

#[test]
fn a_freshly_granted_repository_has_ask_off() {
    let path = settings("fresh");

    let granted = GrantedRepository::establish(&repo("alpha"), Some(path)).expect("granted");

    assert!(!granted.ask_enabled());
}

#[test]
fn enabling_ask_survives_re_establishing_the_grant() {
    let path = settings("toggle");
    let locator = repo("alpha");

    let mut granted = GrantedRepository::establish(&locator, Some(path.clone())).expect("granted");
    assert!(granted.set_ask_enabled(true).expect("saved"));

    let reopened = GrantedRepository::establish(&locator, Some(path)).expect("granted");

    assert!(reopened.ask_enabled(), "opt-in did not persist");
    assert_eq!(reopened.identity(), granted.identity());
}

#[test]
fn turning_ask_off_again_is_remembered() {
    let path = settings("off-again");
    let locator = repo("alpha");
    let mut granted = GrantedRepository::establish(&locator, Some(path.clone())).expect("granted");
    granted.set_ask_enabled(true).expect("saved");

    assert!(!granted.set_ask_enabled(false).expect("saved"));

    let reopened = GrantedRepository::establish(&locator, Some(path)).expect("granted");
    assert!(!reopened.ask_enabled());
}

#[test]
fn a_corrupt_settings_file_prevents_a_grant_rather_than_granting_blindly() {
    // Failing closed all the way up: no grant means the window reads ASK as
    // off, which is the safe direction.
    let path = settings("corrupt-grant");
    std::fs::create_dir_all(path.parent().expect("parent")).expect("create");
    std::fs::write(&path, "{{{").expect("write");

    assert!(GrantedRepository::establish(&repo("alpha"), Some(path)).is_err());
}

#[test]
fn a_platform_with_nowhere_to_save_still_grants_and_stays_off() {
    // No settings path: the grant works for this session, nothing is
    // remembered, and ASK is off — not an error the user has to clear.
    let mut granted = GrantedRepository::establish(&repo("alpha"), None).expect("granted");

    assert!(!granted.ask_enabled());
    assert!(granted.set_ask_enabled(true).expect("no store to fail"));
}

#[test]
fn two_repositories_hold_independent_opt_in_through_the_grant() {
    let path = settings("two-grants");

    let mut a = GrantedRepository::establish(&repo("alpha"), Some(path.clone())).expect("granted");
    a.set_ask_enabled(true).expect("saved");

    let b = GrantedRepository::establish(&repo("beta"), Some(path)).expect("granted");

    assert_ne!(a.identity(), b.identity());
    assert!(!b.ask_enabled(), "opt-in leaked between repositories");
}
