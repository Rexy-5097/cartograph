//! A credential value that resists becoming text.
//!
//! # Why this lives in the domain model
//!
//! [`Secret`] arrived with M16 Slice 4 in `cartograph_desktop::credential`,
//! beside the `CredentialStore` that produces it. It moved here for one
//! reason, recorded in
//! [ADR-0021](../../../docs/adr/ADR-0021-ask-boundary-and-citation-contract.md)
//! Amendment 3, decision C3: the AI provider crate takes a credential and must
//! **not** depend on the desktop, because the desktop is a client and a client
//! is not something a library may sit below.
//!
//! The store stayed where it was. `CredentialStore::get` is keyed by
//! `cartograph_pipeline::authorization::RepositoryIdentity`, so following the
//! value down here would make this crate depend on another Cartograph crate —
//! the one thing QG-006 checks by name. So the split is not a compromise:
//! **the value comes down to where everything can see it; the store, which
//! needs a repository identity, stays up with the client that has one.**
//!
//! # The secret never becomes text
//!
//! No `Display`, no `Serialize`, no accessor handing out an owned `String`,
//! and a `Debug` that prints a placeholder. It cannot be formatted into a log
//! line, an error, a configuration file or an IPC payload by accident — the
//! type refuses, rather than a reviewer remembering.
//! `cartograph_pipeline::redaction` is the second line for anything that does
//! reach tracing; this is the first.

use std::fmt;

use crate::sensitive::REDACTED;

/// An API credential.
///
/// Deliberately hard to spill. There is no `Display`, no `Serialize` and no
/// accessor that hands out an owned `String`; a caller that genuinely needs the
/// bytes borrows them for as long as the call, through [`Secret::expose`].
#[derive(Clone, PartialEq, Eq)]
pub struct Secret(String);

impl Secret {
    /// Wraps a credential value.
    #[must_use]
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    /// Borrows the credential, for the one place that must send it.
    ///
    /// Named so that a reader of the call site sees what is happening. It is
    /// the only way out of this type, and it yields a borrow rather than an
    /// owned `String` so the value is harder to accumulate somewhere it should
    /// not be.
    #[must_use]
    pub fn expose(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for Secret {
    /// Never the value.
    ///
    /// The same treatment `RepositoryIdentity` gives itself, spelled with
    /// [`REDACTED`] now that both live in this crate, so one vocabulary means
    /// "something was removed here" and there is one place it is spelled.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Secret({REDACTED})")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_secret_never_prints_its_value() {
        let secret = Secret::new("not-a-real-key-0123456789");

        let debugged = format!("{secret:?}");

        assert_eq!(debugged, "Secret(<redacted>)");
        assert!(!debugged.contains("0123456789"));
    }

    #[test]
    fn a_secret_inside_a_larger_structure_still_never_prints() {
        // The realistic leak: nobody formats the secret directly, they format
        // the thing holding it.
        let held = Some(vec![Secret::new("not-a-real-key-0123456789")]);

        let debugged = format!("{held:?}");

        assert!(!debugged.contains("0123456789"), "leaked: {debugged}");
        assert!(debugged.contains(REDACTED));
    }

    #[test]
    fn the_value_is_reachable_only_through_expose() {
        let secret = Secret::new("value");
        assert_eq!(secret.expose(), "value");
    }
}
