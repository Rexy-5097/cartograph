//! The OS keychain, behind the credential boundary Slice 4 already shipped.
//!
//! # What this module is
//!
//! [`NoCredentials`] is the store that answers "there is none" to everything.
//! This is the one that answers truthfully, by asking the operating system:
//! the Windows Credential Manager, the macOS Keychain, or the Secret Service
//! that GNOME Keyring and `KWallet` implement on Linux. Which one is decided at
//! compile time and never at runtime, so a build links exactly one backend and
//! never three (ADR-0021 Amendment 2).
//!
//! Nothing else changes. [`CredentialStore`] is the same trait with the same
//! three methods, absence is still `Ok(None)` rather than an error, and a
//! machine with no working keychain still reads as "no credential" — which is
//! the state §13 requires the product to be fully useful in.
//!
//! # How an entry is addressed
//!
//! `keyring_core::Entry` is named by two strings, and ADR-0021 Amendment 4
//! decides which two:
//!
//! | | |
//! |---|---|
//! | service | [`SERVICE`] — a constant, the same everywhere |
//! | account | [`RepositoryIdentity::keychain_subject`] — the grant token, verbatim |
//!
//! Nothing is hashed, derived from a path, derived from Git, canonicalised, or
//! persisted a second time. Two repositories hold two different tokens, so they
//! name two different accounts, so neither can read the other's key. That is
//! the whole isolation argument, and it is the mapping rather than a check.
//!
//! # Why this store is not the process-global one
//!
//! `keyring-core` offers a process-wide default store, set once at startup.
//! This module does not use it. [`KeychainStore`] holds its own
//! `Arc<CredentialStore>` and builds entries from it directly, for two reasons:
//! a test can then hand in the `mock` store without touching global state that
//! another test running in parallel would see, and the substitutability the
//! [`CredentialStore`] trait exists for keeps working. Production behaviour is
//! identical — the platform store is the same object either way.
//!
//! # What is deliberately not here
//!
//! No deletion during pruning. `OptIns::prune_missing` can forget a
//! locator→identity record while the secret stays in the keychain, and
//! Amendment 4 records that orphan rather than closing it: automatic cleanup is
//! a lifecycle decision for the slice that wires the UI, not something to
//! introduce quietly underneath a storage backend.
//!
//! No credential ever reaches a log, an error, the frontend, IPC or MCP.
//! [`Secret`] has no `Display` and no `Serialize`, and [`CredentialError`]
//! carries no data at all.
//!
//! [`NoCredentials`]: crate::credential::NoCredentials
//! [`RepositoryIdentity::keychain_subject`]: cartograph_pipeline::authorization::RepositoryIdentity::keychain_subject

use std::sync::Arc;

use cartograph_pipeline::authorization::RepositoryIdentity;
use keyring_core::api::CredentialStore as KeyringStore;
use keyring_core::{Entry, Error as KeyringError};

use crate::credential::{CredentialError, CredentialStore, Secret};

/// The keychain service every Cartograph credential is filed under.
///
/// Fixed by ADR-0021 Amendment 4. It is the same on every machine and for every
/// repository; the repository is distinguished by the account, never by this.
pub const SERVICE: &str = "cartograph-ask";

/// A credential store backed by the operating system's keychain.
///
/// Construct with [`KeychainStore::open`] in production, or
/// [`KeychainStore::with_store`] in a test that supplies `keyring-core`'s
/// `mock` store.
pub struct KeychainStore {
    store: Arc<KeyringStore>,
}

impl std::fmt::Debug for KeychainStore {
    /// Names the store and nothing else.
    ///
    /// The inner store's own `Debug` may describe entries, and an entry is
    /// named by a subject. Amendment 4 forbids a subject reaching `Debug`, so
    /// this does not delegate.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("KeychainStore")
    }
}

impl KeychainStore {
    /// Opens this platform's keychain.
    ///
    /// # Errors
    ///
    /// [`CredentialError::Unavailable`] when the platform store cannot be
    /// created — no Secret Service on the session bus, for instance. A caller
    /// that would rather degrade than fail should fall back to
    /// [`NoCredentials`](crate::credential::NoCredentials), which is the shape
    /// §13 asks for.
    pub fn open() -> Result<Self, CredentialError> {
        platform_store()
            .map(|store| Self { store })
            .map_err(|_| CredentialError::Unavailable)
    }

    /// Builds a store on a caller-supplied `keyring-core` store.
    ///
    /// This is how a test uses the `mock` store: no global state, so tests
    /// running in parallel cannot see each other's credentials.
    #[must_use]
    pub fn with_store(store: Arc<KeyringStore>) -> Self {
        Self { store }
    }

    /// The entry this repository's credential lives in.
    ///
    /// The one place the Amendment 4 mapping is written down in code.
    fn entry(&self, identity: &RepositoryIdentity) -> Result<Entry, KeyringError> {
        self.store.build(SERVICE, identity.keychain_subject(), None)
    }
}

/// Maps a keychain failure onto the boundary's four outcomes.
///
/// [`KeychainError::NoEntry`](KeyringError::NoEntry) is **absent**, not a
/// failure, and is handled by the callers rather than here — absence is an
/// ordinary state and mapping it to an error is what would break degraded ASK.
///
/// The rest keep the distinction Slice 4 drew. Collapsing them would tell a
/// user with a locked keychain that they have no key, which is a different
/// problem with a different fix.
fn classify(error: &KeyringError) -> CredentialError {
    match error {
        // The store exists and refused. A locked keychain, or a denied prompt.
        KeyringError::NoStorageAccess(_) => CredentialError::PermissionDenied,
        // There is no store to ask: none registered, or one this build cannot
        // read. Both are "no credential store is available".
        KeyringError::NoDefaultStore | KeyringError::BadStoreFormat(_) => {
            CredentialError::Unavailable
        }
        // Everything else is the store failing in a way the caller cannot act
        // on, including `NoEntry` if it ever reaches here — a caller that has
        // not already treated absence as absence has a bug, and reporting a
        // backend failure is the conservative reading.
        _ => CredentialError::Backend,
    }
}

impl CredentialStore for KeychainStore {
    fn get(&self, identity: &RepositoryIdentity) -> Result<Option<Secret>, CredentialError> {
        match self.entry(identity).and_then(|entry| entry.get_password()) {
            Ok(password) => Ok(Some(Secret::new(password))),
            Err(KeyringError::NoEntry) => Ok(None),
            Err(error) => Err(classify(&error)),
        }
    }

    fn set(&self, identity: &RepositoryIdentity, secret: Secret) -> Result<(), CredentialError> {
        self.entry(identity)
            .and_then(|entry| entry.set_password(secret.expose()))
            .map_err(|error| classify(&error))
    }

    fn delete(&self, identity: &RepositoryIdentity) -> Result<(), CredentialError> {
        match self
            .entry(identity)
            .and_then(|entry| entry.delete_credential())
        {
            // Deleting an absent credential succeeds: the trait says so, and
            // the state the caller wanted is the state that now holds.
            Ok(()) | Err(KeyringError::NoEntry) => Ok(()),
            Err(error) => Err(classify(&error)),
        }
    }
}

/// The store for the target being compiled.
///
/// Compile-time selection, so a binary contains one backend and the choice
/// cannot be wrong at runtime. A target with no arm here has no keychain
/// backend and [`KeychainStore::open`] reports it unavailable, which is the
/// degraded state the product is required to remain useful in.
#[cfg(windows)]
fn platform_store() -> Result<Arc<KeyringStore>, KeyringError> {
    windows_native_keyring_store::Store::new().map(|store| store as Arc<KeyringStore>)
}

#[cfg(target_os = "macos")]
fn platform_store() -> Result<Arc<KeyringStore>, KeyringError> {
    // `keychain` and `protected` are two different stores in two modules, not
    // one store with a switch, so the path names the module (ADR-0021
    // Amendment 5 chose `keychain`).
    apple_native_keyring_store::keychain::Store::new().map(|store| store as Arc<KeyringStore>)
}

#[cfg(target_os = "linux")]
fn platform_store() -> Result<Arc<KeyringStore>, KeyringError> {
    zbus_secret_service_keyring_store::Store::new().map(|store| store as Arc<KeyringStore>)
}

#[cfg(not(any(windows, target_os = "macos", target_os = "linux")))]
fn platform_store() -> Result<Arc<KeyringStore>, KeyringError> {
    Err(KeyringError::NoDefaultStore)
}
