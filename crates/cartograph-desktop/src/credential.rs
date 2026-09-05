//! The credential boundary: where an API key would live, and how it is reached.
//!
//! # What the specification requires
//!
//! Frozen Engineering Specification V3 §13 is unambiguous: *"Credentials live
//! in the OS keychain, never in configuration files"*, and *"AI is opt-in, per
//! repository, off by default. The product is fully useful with no API key and
//! no network."*
//!
//! Two things follow, and this module is both of them. A credential is reached
//! through **one narrow interface** rather than read wherever it is wanted, and
//! **having no credential at all is an ordinary, supported state** — not an
//! error path bolted on afterwards.
//!
//! # Why there is no OS keychain backend here yet
//!
//! §10 — the frozen tech stack — names thirteen crates for the Rust core, and
//! **none of them stores credentials**. §13 states the requirement; §10 does not
//! name a crate that satisfies it. QG-006 requires that *"new dependencies not
//! named in the frozen stack need an ADR reference in the PR"*, so choosing one
//! is a decision with its own evidence, not something to settle inside an
//! implementation slice.
//!
//! So this slice ships the **boundary** and no backend. That is not a stub for
//! its own sake: with no backend, [`NoCredentials`] reports the credential as
//! absent, ASK degrades to raw evidence, and the property §13 actually asks for
//! — *"fully useful with no API key"* — is the behaviour that ships and is
//! tested. A backend can be added behind this trait without any caller changing.
//!
//! # The secret never becomes text — and it no longer lives here
//!
//! [`Secret`] has no `Display`, no `Serialize`, and a `Debug` that prints a
//! placeholder. It cannot be formatted into a log line, an error, a
//! configuration file or an IPC payload by accident — the type refuses, rather
//! than a reviewer remembering. `cartograph_pipeline::redaction` is the second
//! line for anything that does reach tracing; this is the first.
//!
//! The type itself now lives in [`cartograph_core::secret`] and is re-exported
//! below, so `cartograph_desktop::credential::Secret` still resolves and every
//! caller here compiles unchanged. It moved because the AI provider crate
//! takes a credential and may not depend on the desktop (ADR-0021 Amendment 3,
//! decision C3).
//!
//! **The store did not follow it.** [`CredentialStore::get`] is keyed by
//! [`RepositoryIdentity`], which lives in `cartograph-pipeline`; moving the
//! trait into `cartograph-core` would make the domain model depend on another
//! Cartograph crate, which is the dependency direction QG-006 checks by name.
//! So the value went down to where every crate can see it, and the store —
//! which needs a repository identity — stayed up here with the client that has
//! one.

use cartograph_pipeline::authorization::RepositoryIdentity;

pub use cartograph_core::secret::Secret;

/// Why a credential could not be reached.
///
/// "Absent" is deliberately **not** one of these: a repository with no
/// credential is the default state of the product, and calling it an error
/// would make the ordinary path look like a failure.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum CredentialError {
    /// The store exists but refused access — a locked keychain, or a denied
    /// prompt. The user can plausibly fix this.
    PermissionDenied,
    /// No credential store is available on this machine or in this build.
    Unavailable,
    /// The store failed for a reason the caller cannot act on.
    Backend,
}

impl std::fmt::Display for CredentialError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::PermissionDenied => "the credential store refused access",
            Self::Unavailable => "no credential store is available",
            Self::Backend => "the credential store failed",
        })
    }
}

impl std::error::Error for CredentialError {}

/// Where a repository's API credential is kept.
///
/// Keyed by [`RepositoryIdentity`] — the same opaque identity the authorization
/// boundary uses (ADR-0020 Amendment 3), so a credential cannot be looked up by
/// a path and one repository's key cannot be reached with another's handle.
pub trait CredentialStore {
    /// The credential for this repository, or `None` if there is none.
    ///
    /// # Errors
    ///
    /// [`CredentialError`] when the store could not be consulted. Absence is
    /// `Ok(None)`, not an error.
    fn get(&self, identity: &RepositoryIdentity) -> Result<Option<Secret>, CredentialError>;

    /// Stores or replaces the credential for this repository.
    ///
    /// # Errors
    ///
    /// [`CredentialError`] when the store refused or failed.
    fn set(&self, identity: &RepositoryIdentity, secret: Secret) -> Result<(), CredentialError>;

    /// Removes the credential for this repository, if there is one.
    ///
    /// # Errors
    ///
    /// [`CredentialError`] when the store refused or failed. Deleting an
    /// absent credential succeeds.
    fn delete(&self, identity: &RepositoryIdentity) -> Result<(), CredentialError>;
}

/// The store this build ships with: there is no credential store.
///
/// Every repository reads as having no credential, which is exactly the state
/// §13 requires the product to be fully useful in. When a keychain crate is
/// authorised, it replaces this implementation and nothing else changes.
#[derive(Debug, Clone, Copy, Default)]
pub struct NoCredentials;

impl CredentialStore for NoCredentials {
    fn get(&self, _identity: &RepositoryIdentity) -> Result<Option<Secret>, CredentialError> {
        Ok(None)
    }

    fn set(&self, _identity: &RepositoryIdentity, _secret: Secret) -> Result<(), CredentialError> {
        Err(CredentialError::Unavailable)
    }

    fn delete(&self, _identity: &RepositoryIdentity) -> Result<(), CredentialError> {
        Err(CredentialError::Unavailable)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn identity(value: &str) -> RepositoryIdentity {
        RepositoryIdentity::from_grant(value).expect("non-empty")
    }

    /// The type's own behaviour is tested where it now lives, in
    /// `cartograph_core::secret`. What is tested here is what this module still
    /// owns: that the re-export resolves, and that the store built on it keeps
    /// its guarantees.
    #[test]
    fn the_re_export_resolves_and_still_refuses_to_print_its_value() {
        let secret = Secret::new("not-a-real-key-0123456789");

        let debugged = format!("{secret:?}");

        assert_eq!(debugged, "Secret(<redacted>)");
        assert_eq!(secret.expose(), "not-a-real-key-0123456789");
    }

    #[test]
    fn with_no_store_every_repository_reads_as_having_no_credential() {
        // The state §13 requires the product to be fully useful in.
        let store = NoCredentials;

        assert_eq!(store.get(&identity("a")).expect("consulted"), None);
        assert_eq!(store.get(&identity("b")).expect("consulted"), None);
    }

    #[test]
    fn with_no_store_writing_is_refused_rather_than_silently_dropped() {
        let store = NoCredentials;

        assert_eq!(
            store.set(&identity("a"), Secret::new("k")),
            Err(CredentialError::Unavailable)
        );
        assert_eq!(
            store.delete(&identity("a")),
            Err(CredentialError::Unavailable)
        );
    }

    #[test]
    fn a_credential_error_never_carries_a_value() {
        for error in [
            CredentialError::PermissionDenied,
            CredentialError::Unavailable,
            CredentialError::Backend,
        ] {
            let rendered = format!("{error} {error:?}");
            assert!(!rendered.contains("key"), "error text leaks: {rendered}");
        }
    }
}
