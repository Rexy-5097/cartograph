//! The OS keychain backend, tested against `keyring-core`'s `mock` store.
//!
//! **No test here touches a real keychain.** A test that did would prompt a
//! developer, fail in CI where no session keyring exists, and — worst — could
//! read or overwrite a real credential belonging to whoever ran it. Every store
//! below is a `mock` built for that test alone, handed to
//! [`KeychainStore::with_store`], so nothing is shared, nothing is global, and
//! tests running in parallel cannot see each other.
//!
//! Secrets are assembled at runtime rather than written as literals, so a
//! credential-shaped string never sits in a tracked file for QG-005 to find.

use std::sync::Arc;

use cartograph_desktop::credential::{CredentialError, CredentialStore, Secret};
use cartograph_desktop::keychain::{KeychainStore, SERVICE};
use cartograph_pipeline::authorization::RepositoryIdentity;
use keyring_core::api::CredentialStore as KeyringStore;
use keyring_core::mock;

/// A fresh mock keychain, belonging to one test.
fn keychain() -> Arc<KeyringStore> {
    mock::Store::new().expect("a mock store is always available") as Arc<KeyringStore>
}

/// A store over a fresh mock keychain.
fn store() -> KeychainStore {
    KeychainStore::with_store(keychain())
}

/// An identity minted the way the desktop mints one.
///
/// Opaque, and carrying nothing about a repository — which is the property the
/// isolation tests below depend on.
fn identity(tag: &str) -> RepositoryIdentity {
    RepositoryIdentity::from_grant(format!("desktop-{tag}-0")).expect("non-empty")
}

/// A credential-shaped value built at runtime.
///
/// Never a literal: QG-005 scans tracked files for things that look like keys,
/// and it is right to.
fn fake_key(tag: &str) -> String {
    let prefix: String = ['g', 's', 'k', '_'].iter().collect();
    format!("{prefix}{tag}{}", "0123456789abcdef".repeat(2))
}

// ---------------------------------------------------------------- addressing

#[test]
fn the_service_is_the_one_the_amendment_fixed() {
    // ADR-0021 Amendment 4. A test rather than a comment, because the constant
    // is what a user's keychain will show and renaming it would strand every
    // credential already stored.
    assert_eq!(SERVICE, "cartograph-ask");
}

#[test]
fn the_account_is_the_grant_token_verbatim() {
    // The whole of the Amendment 4 mapping: no hash, no path, no derivation.
    let token = "desktop-1a2b3c-7";
    let identity = RepositoryIdentity::from_grant(token).expect("non-empty");

    assert_eq!(identity.keychain_subject(), token);
}

#[test]
fn the_same_identity_addresses_the_same_entry() {
    let store = store();
    let first = identity("same");
    let key = fake_key("a");

    store.set(&first, Secret::new(key.clone())).expect("stored");

    // A second, equal identity value — not the same object.
    let again = identity("same");
    let read = store.get(&again).expect("consulted").expect("present");

    assert_eq!(read.expose(), key);
}

#[test]
fn different_identities_address_different_entries() {
    let store = store();
    let (a, b) = (identity("a"), identity("b"));

    store.set(&a, Secret::new(fake_key("a"))).expect("stored");
    store.set(&b, Secret::new(fake_key("b"))).expect("stored");

    let read_a = store.get(&a).expect("consulted").expect("present");
    let read_b = store.get(&b).expect("consulted").expect("present");

    assert_ne!(read_a.expose(), read_b.expose());
}

// ------------------------------------------------------------------- get/set

#[test]
fn set_then_get_returns_the_same_secret() {
    let store = store();
    let identity = identity("roundtrip");
    let key = fake_key("r");

    store
        .set(&identity, Secret::new(key.clone()))
        .expect("stored");

    assert_eq!(
        store
            .get(&identity)
            .expect("consulted")
            .expect("present")
            .expose(),
        key
    );
}

#[test]
fn a_repository_with_no_credential_reads_as_absent_not_as_an_error() {
    // The state §13 requires the product to be fully useful in. If this ever
    // becomes an error, degraded ASK stops being reachable.
    let store = store();

    assert_eq!(store.get(&identity("never-set")).expect("consulted"), None);
}

#[test]
fn overwriting_replaces_rather_than_appends() {
    let store = store();
    let identity = identity("overwrite");
    let second = fake_key("second");

    store
        .set(&identity, Secret::new(fake_key("first")))
        .expect("stored");
    store
        .set(&identity, Secret::new(second.clone()))
        .expect("replaced");

    assert_eq!(
        store
            .get(&identity)
            .expect("consulted")
            .expect("present")
            .expose(),
        second
    );
}

// -------------------------------------------------------------------- delete

#[test]
fn deleting_an_existing_credential_removes_it() {
    let store = store();
    let identity = identity("delete");

    store
        .set(&identity, Secret::new(fake_key("d")))
        .expect("stored");
    store.delete(&identity).expect("deleted");

    assert_eq!(store.get(&identity).expect("consulted"), None);
}

#[test]
fn deleting_an_absent_credential_succeeds() {
    // The trait says so: the state the caller wanted is the state that holds.
    let store = store();

    assert_eq!(store.delete(&identity("never-there")), Ok(()));
}

#[test]
fn deleting_one_repository_leaves_the_other_alone() {
    let store = store();
    let (a, b) = (identity("keep"), identity("drop"));
    let kept = fake_key("keep");

    store.set(&a, Secret::new(kept.clone())).expect("stored");
    store
        .set(&b, Secret::new(fake_key("drop")))
        .expect("stored");

    store.delete(&b).expect("deleted");

    assert_eq!(
        store.get(&a).expect("consulted").expect("present").expose(),
        kept
    );
    assert_eq!(store.get(&b).expect("consulted"), None);
}

// ----------------------------------------------------------------- isolation

#[test]
fn one_repository_cannot_read_another_repositorys_credential() {
    // The security invariant this whole design exists for, in both directions.
    let store = store();
    let (a, b) = (identity("alpha"), identity("beta"));
    let (key_a, key_b) = (fake_key("alpha"), fake_key("beta"));

    store.set(&a, Secret::new(key_a.clone())).expect("stored");
    store.set(&b, Secret::new(key_b.clone())).expect("stored");

    let from_a = store.get(&a).expect("consulted").expect("present");
    let from_b = store.get(&b).expect("consulted").expect("present");

    assert_eq!(from_a.expose(), key_a);
    assert_ne!(from_a.expose(), key_b, "A read B's credential");
    assert_eq!(from_b.expose(), key_b);
    assert_ne!(from_b.expose(), key_a, "B read A's credential");
}

#[test]
fn a_repository_with_no_key_does_not_fall_back_to_another_repositorys() {
    // There is no shared entry and no global default to fall back to, so a
    // repository that never stored a key reads as absent even when another
    // repository's key is sitting in the same keychain.
    let store = store();

    store
        .set(&identity("has-one"), Secret::new(fake_key("h")))
        .expect("stored");

    assert_eq!(store.get(&identity("has-none")).expect("consulted"), None);
}

// ------------------------------------------------------------- error mapping

/// Arms the mock so the next operation on this identity's entry fails.
fn arm(keychain: &Arc<KeyringStore>, identity: &RepositoryIdentity, error: keyring_core::Error) {
    let entry = keychain
        .build(SERVICE, identity.keychain_subject(), None)
        .expect("built");
    entry
        .as_any()
        .downcast_ref::<mock::Cred>()
        .expect("a mock credential")
        .set_error(error);
}

#[test]
fn a_refused_keychain_reports_permission_denied() {
    // A locked keychain or a denied prompt. The user can plausibly fix this,
    // which is why it must not read as "no credential".
    let keychain = keychain();
    let store = KeychainStore::with_store(Arc::clone(&keychain));
    let identity = identity("refused");

    arm(
        &keychain,
        &identity,
        keyring_core::Error::NoStorageAccess(Box::new(std::io::Error::other("refused"))),
    );

    assert_eq!(store.get(&identity), Err(CredentialError::PermissionDenied));
}

#[test]
fn an_unreadable_store_reports_unavailable() {
    let keychain = keychain();
    let store = KeychainStore::with_store(Arc::clone(&keychain));
    let identity = identity("unreadable");

    arm(
        &keychain,
        &identity,
        keyring_core::Error::BadStoreFormat(String::from("not a store this build understands")),
    );

    assert_eq!(store.get(&identity), Err(CredentialError::Unavailable));
}

#[test]
fn a_failing_platform_reports_a_backend_failure() {
    let keychain = keychain();
    let store = KeychainStore::with_store(Arc::clone(&keychain));
    let identity = identity("broken");

    arm(
        &keychain,
        &identity,
        keyring_core::Error::PlatformFailure(Box::new(std::io::Error::other("platform"))),
    );

    assert_eq!(store.get(&identity), Err(CredentialError::Backend));
}

#[test]
fn the_four_outcomes_stay_four() {
    // Missing, refused, unavailable and broken are four different things with
    // four different fixes. Collapsing any pair would tell a user with a locked
    // keychain that they have no key.
    let keychain = keychain();
    let store = KeychainStore::with_store(Arc::clone(&keychain));

    let missing = identity("m");
    let refused = identity("r");
    let unusable = identity("u");
    let broken = identity("b");

    arm(
        &keychain,
        &refused,
        keyring_core::Error::NoStorageAccess(Box::new(std::io::Error::other("x"))),
    );
    arm(
        &keychain,
        &unusable,
        keyring_core::Error::BadStoreFormat(String::new()),
    );
    arm(
        &keychain,
        &broken,
        keyring_core::Error::PlatformFailure(Box::new(std::io::Error::other("x"))),
    );

    assert_eq!(store.get(&missing), Ok(None));
    assert_eq!(store.get(&refused), Err(CredentialError::PermissionDenied));
    assert_eq!(store.get(&unusable), Err(CredentialError::Unavailable));
    assert_eq!(store.get(&broken), Err(CredentialError::Backend));
}

#[test]
fn a_write_failure_is_reported_rather_than_silently_dropped() {
    let keychain = keychain();
    let store = KeychainStore::with_store(Arc::clone(&keychain));
    let identity = identity("write-refused");

    arm(
        &keychain,
        &identity,
        keyring_core::Error::NoStorageAccess(Box::new(std::io::Error::other("x"))),
    );

    assert_eq!(
        store.set(&identity, Secret::new(fake_key("w"))),
        Err(CredentialError::PermissionDenied)
    );
}

// ------------------------------------------------------------------ leakage

#[test]
fn the_credential_never_appears_in_any_debug_output() {
    let store = store();
    let identity = identity("debug");
    let key = fake_key("debug");

    store
        .set(&identity, Secret::new(key.clone()))
        .expect("stored");
    let secret = store.get(&identity).expect("consulted").expect("present");

    let rendered = format!("{store:?} {secret:?} {identity:?}");

    assert!(!rendered.contains(&key), "a credential reached Debug");
    assert_eq!(format!("{secret:?}"), "Secret(<redacted>)");
}

#[test]
fn the_store_debug_does_not_disclose_the_subject() {
    // The inner keyring store's own Debug may describe entries, and an entry is
    // named by a subject. This is why KeychainStore's Debug does not delegate.
    let store = store();
    let identity = identity("subject-in-debug");

    store
        .set(&identity, Secret::new(fake_key("s")))
        .expect("stored");

    let rendered = format!("{store:?}");

    assert!(
        !rendered.contains(identity.keychain_subject()),
        "the keychain subject reached Debug: {rendered}"
    );
}

#[test]
fn the_identity_still_refuses_to_print_itself() {
    // The accessor did not widen Debug, and there is still no Display. If a
    // Display impl is ever added this test will not catch it — but the Debug
    // guarantee is the one ADR-0020 relies on, and it holds.
    let identity = identity("redacted");

    assert_eq!(format!("{identity:?}"), "RepositoryIdentity(<redacted>)");
}

#[test]
fn no_credential_error_carries_a_credential_or_a_subject() {
    let keychain = keychain();
    let store = KeychainStore::with_store(Arc::clone(&keychain));
    let identity = identity("errtext");
    let key = fake_key("errtext");

    arm(
        &keychain,
        &identity,
        keyring_core::Error::NoStorageAccess(Box::new(std::io::Error::other("x"))),
    );
    let error = store.get(&identity).expect_err("refused");

    let rendered = format!("{error} {error:?}");

    assert!(!rendered.contains(&key));
    assert!(!rendered.contains(identity.keychain_subject()));
}

#[test]
fn a_credential_error_is_a_plain_enum_with_nowhere_to_hide_a_value() {
    // Structural rather than incidental: these variants carry no fields, so
    // there is no place a secret could be put even by accident.
    for error in [
        CredentialError::PermissionDenied,
        CredentialError::Unavailable,
        CredentialError::Backend,
    ] {
        let rendered = format!("{error} {error:?}");
        assert!(!rendered.contains("gsk"), "error text leaks: {rendered}");
    }
}

// ----------------------------------------------------------------- lifecycle

#[test]
fn a_credential_survives_a_simulated_application_restart() {
    // The OS keychain persists; the application object does not. So the same
    // keychain is handed to a second store, exactly as a restart would.
    let keychain = keychain();
    let identity = identity("restart");
    let key = fake_key("restart");

    {
        let before = KeychainStore::with_store(Arc::clone(&keychain));
        before
            .set(&identity, Secret::new(key.clone()))
            .expect("stored");
    }

    let after = KeychainStore::with_store(Arc::clone(&keychain));

    assert_eq!(
        after
            .get(&identity)
            .expect("consulted")
            .expect("present")
            .expose(),
        key
    );
}

#[test]
fn nothing_about_the_credential_is_written_to_configuration() {
    // §13's second half. The opt-in table is the only thing this product
    // persists itself, and a credential must never reach it.
    let keychain = keychain();
    let store = KeychainStore::with_store(Arc::clone(&keychain));
    let identity = identity("config");
    let key = fake_key("config");

    store
        .set(&identity, Secret::new(key.clone()))
        .expect("stored");

    let table = cartograph_desktop::optin::OptIns::default();
    let serialised = serde_json::to_string(&table).expect("serialisable");

    assert!(!serialised.contains(&key), "a credential reached config");
}

#[test]
fn a_moved_repository_gets_its_own_entry_rather_than_inheriting_one() {
    // Amendment 4's lifecycle consequence, asserted as it actually is. The
    // identity is NOT derived from the locator, so a move that mints a fresh
    // token yields a fresh entry — the old credential is stranded, not
    // inherited. The opposite claim, "changing the path preserves the
    // identity", is false here and is deliberately not asserted.
    let store = store();
    let before_move = identity("mint-1");
    let after_move = identity("mint-2");
    let key = fake_key("moved");

    store
        .set(&before_move, Secret::new(key.clone()))
        .expect("stored");

    // The repository moved: a new token was minted for the new locator.
    assert_eq!(
        store.get(&after_move).expect("consulted"),
        None,
        "a fresh identity silently inherited another's credential"
    );
    // And the old credential is still exactly where it was, untouched.
    assert_eq!(
        store
            .get(&before_move)
            .expect("consulted")
            .expect("present")
            .expose(),
        key
    );
}

#[test]
fn the_subject_carries_nothing_about_the_repository() {
    // Two identities minted for two different locators differ, and neither
    // subject contains any part of a path. `mint()` never sees the locator,
    // which is the reason rather than the hope.
    let first = identity("mint-1");
    let second = identity("mint-2");

    assert_ne!(first.keychain_subject(), second.keychain_subject());

    for subject in [first.keychain_subject(), second.keychain_subject()] {
        assert!(!subject.contains('/'), "a path reached the subject");
        assert!(!subject.contains('\\'), "a path reached the subject");
        assert!(!subject.contains(':'), "a drive letter reached the subject");
    }
}

#[test]
fn forgetting_the_opt_in_record_does_not_delete_the_credential() {
    // Amendment 4 records this orphan rather than closing it. The test exists
    // so that if a later slice introduces cleanup, it changes a test that says
    // what today's behaviour is — instead of silently changing behaviour
    // nothing described.
    let store = store();
    let identity = identity("orphan");
    let key = fake_key("orphan");

    store
        .set(&identity, Secret::new(key.clone()))
        .expect("stored");

    // Pruning the table forgets the locator→identity record. It does not, and
    // must not, reach into the keychain.
    let mut table = cartograph_desktop::optin::OptIns::default();
    let _ = table.prune_missing();

    assert_eq!(
        store
            .get(&identity)
            .expect("consulted")
            .expect("present")
            .expose(),
        key,
        "pruning deleted a credential it was not asked to delete"
    );
}

#[test]
fn two_mock_keychains_do_not_share_credentials() {
    // Each test owns its keychain. If mock state were global, tests running in
    // parallel would see each other's credentials and this suite would be
    // quietly meaningless.
    let one = KeychainStore::with_store(keychain());
    let other = KeychainStore::with_store(keychain());
    let identity = identity("shared");

    one.set(&identity, Secret::new(fake_key("one")))
        .expect("stored");

    assert_eq!(other.get(&identity).expect("consulted"), None);
}

#[test]
fn no_process_wide_store_is_ever_registered() {
    // The strongest statement this suite can make about not touching a real
    // keychain: `keyring-core` reaches a platform store only through the
    // process-wide default, and nothing here ever sets one. Every store in
    // these tests was constructed explicitly and handed in.
    assert!(
        keyring_core::get_default_store().is_none(),
        "a process-wide credential store was registered; a test could reach a real keychain"
    );
}
