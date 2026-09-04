//! Per-repository ASK opt-in, and the desktop's side of the grant.
//!
//! # What the specification requires
//!
//! Frozen Engineering Specification V3 §13: *"AI is opt-in, per repository, off
//! by default."* Off by default is the important half — absence of an entry
//! here means disabled, and nothing in this module can turn ASK on as a side
//! effect of anything else.
//!
//! # The identity is granted, never derived
//!
//! ADR-0020 Amendment 3 made the desktop a **second grant producer** under the
//! one existing opaque identity model. When the human selects a repository, the
//! desktop either recovers the identity that locator was previously recorded
//! against, or **mints a fresh opaque one** and records the association.
//!
//! That association is a **persisted fact, not a derivation**. No function here
//! maps a path to an identity: [`mint`] does not read the locator, and two runs
//! over the same path produce different tokens unless one was written down. The
//! distinction is load-bearing — a derivation would make two spellings of one
//! path agree, and a recorded mapping does not pretend to. The consequences
//! Amendment 3 accepted follow from exactly that: a moved or renamed repository
//! does not carry its opt-in, and one tree reached through two locators may hold
//! two identities.
//!
//! # What this file may hold, and what it may not
//!
//! Locator, opaque identity, opt-in flag. **Never** an API key, a token, source
//! text, `Evidence`, an analysis, or an environment value — §13 puts credentials
//! in the OS keychain, *"never in configuration files"*, and
//! [`crate::credential`] is where they go instead.
//!
//! # Failing closed
//!
//! A configuration that cannot be read or cannot be parsed is refused rather
//! than treated as empty. Empty would read as "no repository has opted in",
//! which is the same answer a *successful* load of a fresh install gives — so a
//! corrupt file would be indistinguishable from a clean one, and the failure
//! would be silent. The caller degrades to disabled on an error; it is told
//! that it is doing so.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use cartograph_pipeline::authorization::RepositoryIdentity;
use serde::{Deserialize, Serialize};

use crate::error::{DesktopError, DesktopErrorKind};

/// One repository's recorded association.
///
/// The identity is held as its opaque token so it can be written down and read
/// back; [`Entry::identity`] hands out the typed form. Nothing interprets the
/// token, here or in the authorization layer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct Entry {
    /// Where the repository was when it was selected. A **locator**, used to
    /// find this entry again — never an identity.
    locator: PathBuf,
    /// The opaque token this repository was granted.
    identity: String,
    /// Whether ASK is enabled. Absent entry means disabled; so does `false`.
    ask_enabled: bool,
}

/// The persisted opt-in table.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct OptIns {
    entries: Vec<Entry>,
}

/// Mints an opaque grant token.
///
/// Not derived from anything about the repository — it never sees the locator.
/// A monotonic counter distinguishes grants minted inside one clock tick; the
/// clock distinguishes them across runs. No cryptographic uniqueness is claimed
/// or needed: the value is compared for equality within one machine's
/// configuration, never used as a secret and never transmitted.
fn mint() -> String {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    // Kept as u128: the nanosecond count is what it is, and narrowing it to
    // make a format string shorter would be a truncation nobody needed.
    let ticks = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0_u128, |d| d.as_nanos());
    let seq = NEXT.fetch_add(1, Ordering::Relaxed);
    format!("desktop-{ticks:x}-{seq:x}")
}

impl OptIns {
    /// Reads the table, or reports why it could not be read.
    ///
    /// A missing file is an empty table, not a failure: that is a fresh
    /// install. A file that exists but cannot be read or parsed **is** a
    /// failure, so a corrupt table cannot masquerade as a clean one.
    ///
    /// # Errors
    ///
    /// [`DesktopErrorKind::Internal`] if the file exists but could not be read
    /// or parsed. The caller must treat that as "everything disabled".
    pub fn load(path: &Path) -> Result<Self, DesktopError> {
        let text = match std::fs::read_to_string(path) {
            Ok(text) => text,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Ok(Self::default());
            }
            Err(_) => {
                // The path is not repeated: an error message is a log line.
                return Err(DesktopError::new(
                    DesktopErrorKind::Internal,
                    "The AI settings file could not be read.",
                )
                .with_hint("AI stays off until it can be read again."));
            }
        };

        serde_json::from_str(&text).map_err(|_| {
            DesktopError::new(
                DesktopErrorKind::Internal,
                "The AI settings file could not be understood.",
            )
            .with_hint("AI stays off. Removing the file restores the defaults.")
        })
    }

    /// Writes the table, creating the directory if it is missing.
    ///
    /// # Errors
    ///
    /// [`DesktopErrorKind::Internal`] if the file could not be written.
    pub fn save(&self, path: &Path) -> Result<(), DesktopError> {
        let failed = || {
            DesktopError::new(
                DesktopErrorKind::Internal,
                "The AI settings file could not be saved.",
            )
        };

        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|_| failed())?;
        }
        let text = serde_json::to_string_pretty(self).map_err(|_| failed())?;
        std::fs::write(path, text).map_err(|_| failed())
    }

    /// Saves, when there is somewhere to save to.
    ///
    /// A platform that tells us nothing about where configuration belongs is
    /// not an error: there is simply nowhere to remember an opt-in, so it is
    /// not remembered and ASK stays off next time.
    ///
    /// # Errors
    ///
    /// Whatever [`OptIns::save`] could not do.
    pub fn save_if(&self, path: Option<&Path>) -> Result<(), DesktopError> {
        path.map_or(Ok(()), |p| self.save(p))
    }

    /// The identity for a selected repository: recovered, or freshly granted.
    ///
    /// This is the desktop acting as grant producer (ADR-0020 Amendment 3). It
    /// runs only after a human has chosen the repository.
    ///
    /// # Errors
    ///
    /// [`DesktopErrorKind::Internal`] only if a recorded token is empty, which
    /// would mean the file was edited into an invalid state.
    pub fn grant(&mut self, locator: &Path) -> Result<RepositoryIdentity, DesktopError> {
        if let Some(entry) = self.entries.iter().find(|e| e.locator == locator) {
            return entry.identity();
        }

        let token = mint();
        self.entries.push(Entry {
            locator: locator.to_path_buf(),
            identity: token.clone(),
            ask_enabled: false, // off by default, §13
        });
        RepositoryIdentity::from_grant(token).map_err(|_| {
            DesktopError::new(DesktopErrorKind::Internal, "A grant could not be created.")
        })
    }

    /// Whether ASK is enabled for this repository.
    ///
    /// Unknown identity means disabled. There is no path by which an
    /// unrecognised handle reads as enabled.
    #[must_use]
    pub fn is_enabled(&self, identity: &RepositoryIdentity) -> bool {
        self.entries
            .iter()
            .any(|e| e.matches(identity) && e.ask_enabled)
    }

    /// Turns ASK on or off for one repository.
    ///
    /// Returns whether an entry was found. An unknown identity changes nothing
    /// — a handle that was never granted cannot enable anything.
    pub fn set_enabled(&mut self, identity: &RepositoryIdentity, enabled: bool) -> bool {
        for entry in &mut self.entries {
            if entry.matches(identity) {
                entry.ask_enabled = enabled;
                return true;
            }
        }
        false
    }

    /// How many repositories are recorded.
    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether nothing is recorded.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Drops entries whose locator no longer exists.
    ///
    /// Bookkeeping, not authorization: a stale entry is harmless, and removing
    /// one only reclaims a line. Returns how many were removed.
    pub fn prune_missing(&mut self) -> usize {
        let before = self.entries.len();
        self.entries.retain(|e| e.locator.exists());
        before - self.entries.len()
    }
}

impl Entry {
    /// The typed identity for this entry.
    fn identity(&self) -> Result<RepositoryIdentity, DesktopError> {
        RepositoryIdentity::from_grant(self.identity.clone()).map_err(|_| {
            DesktopError::new(
                DesktopErrorKind::Internal,
                "A recorded grant was not usable.",
            )
            .with_hint("AI stays off for this repository.")
        })
    }

    /// Whether this entry is the one that identity names.
    ///
    /// Compared through `RepositoryIdentity`'s own equality rather than by
    /// string, so the comparison the authorization layer performs is the
    /// comparison performed here.
    fn matches(&self, identity: &RepositoryIdentity) -> bool {
        self.identity().is_ok_and(|recorded| &recorded == identity)
    }
}

/// The repository the human selected, and what it has opted into.
///
/// This is the desktop's grant, held as one value so the Tauri shell holds no
/// logic of its own (ADR-0016: everything that can be wrong lives on this side
/// of the line, where the gates run). The identity never leaves this type — the
/// window is told only whether ASK is on.
#[derive(Debug)]
pub struct GrantedRepository {
    identity: RepositoryIdentity,
    table: OptIns,
    settings: Option<PathBuf>,
}

impl GrantedRepository {
    /// Establishes the grant for a repository the human has just selected.
    ///
    /// Recovers the identity that locator was recorded against, or mints one.
    /// A settings file that cannot be read is **not** treated as empty: the
    /// error is returned, and the caller leaves ASK off.
    ///
    /// # Errors
    ///
    /// [`DesktopErrorKind::Internal`] if the settings could not be read or the
    /// grant could not be created.
    pub fn establish(locator: &Path, settings: Option<PathBuf>) -> Result<Self, DesktopError> {
        let mut table = settings
            .as_deref()
            .map_or_else(|| Ok(OptIns::default()), OptIns::load)?;
        let identity = table.grant(locator)?;
        table.save_if(settings.as_deref())?;
        Ok(Self {
            identity,
            table,
            settings,
        })
    }

    /// The identity this repository was granted.
    ///
    /// Crate-visible on purpose: it is the authorization handle, and no client
    /// surface has a reason to render it.
    #[must_use]
    pub fn identity(&self) -> &RepositoryIdentity {
        &self.identity
    }

    /// Whether ASK is enabled here. The one fact the window is told.
    #[must_use]
    pub fn ask_enabled(&self) -> bool {
        self.table.is_enabled(&self.identity)
    }

    /// Turns ASK on or off and remembers it.
    ///
    /// Returns what is now true rather than what was asked for.
    ///
    /// # Errors
    ///
    /// [`DesktopErrorKind::Internal`] if the settings could not be saved.
    pub fn set_ask_enabled(&mut self, enabled: bool) -> Result<bool, DesktopError> {
        self.table.set_enabled(&self.identity, enabled);
        self.table.save_if(self.settings.as_deref())?;
        Ok(self.ask_enabled())
    }
}

/// Where the opt-in table lives on this platform.
///
/// Resolved from the environment rather than through a crate: the frozen stack
/// (§10) names no configuration-directory dependency, and the three variables
/// below are the platform conventions themselves. Returns `None` when the
/// environment says nothing, in which case there is nowhere to persist and ASK
/// stays off.
#[must_use]
pub fn settings_path() -> Option<PathBuf> {
    let base = if cfg!(windows) {
        std::env::var_os("APPDATA").map(PathBuf::from)
    } else if cfg!(target_os = "macos") {
        std::env::var_os("HOME")
            .map(PathBuf::from)
            .map(|home| home.join("Library/Application Support"))
    } else {
        std::env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .or_else(|| {
                std::env::var_os("HOME")
                    .map(PathBuf::from)
                    .map(|home| home.join(".config"))
            })
    }?;

    Some(base.join("Cartograph").join("ask-optin.json"))
}
