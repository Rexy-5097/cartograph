//! The desktop application's logic, without the desktop.
//!
//! # Why this crate has no Tauri dependency
//!
//! Everything a Tauri command needs to do — validate a chosen folder, run the
//! analysis, shape the result, classify a failure — is ordinary Rust that can
//! be called from a test. Everything Tauri *itself* provides — a window, a
//! webview, an IPC bridge — cannot.
//!
//! Putting the first group here and only the second in `desktop/src-tauri`
//! means the part that can be wrong is the part the workspace gates already
//! run. `cargo test --workspace` covers this crate; the shell above it is
//! thin enough to read.
//!
//! It also keeps the existing gates working. Tauri's Rust crate links a system
//! webview — `WebKitGTK` on Linux — so making it a workspace member would make
//! `cargo check --workspace` on CI depend on system packages that the M00–M10
//! jobs have never needed. See ADR-0016.
//!
//! # The layering, and the rule that governs it
//!
//! ```text
//! React            draws, and owns interaction state
//!   │
//! Tauri command    marshals arguments, calls one function here
//!   │
//! cartograph-desktop   validates, orchestrates, classifies   ← this crate
//!   │
//! cartograph-pipeline  discovery, parsing, caching, resolution
//!   │
//! cartograph-graph     the graph, and the layout (ADR-0015)
//! ```
//!
//! **Analysis only ever moves upward.** No layer above `cartograph-pipeline`
//! decides what connects to what, and no layer above `cartograph-graph`
//! decides where anything is drawn. That is RULE 002 and ADR-0001, and it is
//! the reason the renderer stays swappable.
//!
//! # State lives in the frontend
//!
//! [`Phase`] names the states a session moves through, but this crate does not
//! hold one. A session is a sequence of two pure calls — validate, then
//! analyse — and the window is what remembers which of them has happened.
//! Keeping the machine in the frontend means there is one copy of it rather
//! than two that can disagree; keeping the *names* here means both sides use
//! the same words.

pub mod error;
pub mod evidence;
pub mod repository;
pub mod scene;
pub mod session;

use serde::{Deserialize, Serialize};

/// Where a session has got to.
///
/// Serialised in `camelCase` and mirrored by the TypeScript union in
/// `desktop/src/session.ts`, which a test in this crate pins.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Phase {
    /// Nothing chosen. The empty state, and where the application starts.
    Idle,
    /// A folder dialog is open. Distinguished from `Idle` so the window can
    /// disable the button that opened it.
    Selecting,
    /// A folder was chosen and analysis is running.
    Analyzing,
    /// Analysis finished and there is something to draw.
    Ready,
    /// Analysis or validation failed. Carries a
    /// [`error::DesktopError`] alongside, in the frontend's store.
    Failed,
    /// The user abandoned the operation.
    ///
    /// Recoverable: the next selection starts from `Idle`. See
    /// [`session`] for what "cancelled" does and does not stop.
    Cancelled,
}

impl Phase {
    /// Whether a new repository may be chosen from here.
    ///
    /// `Analyzing` is the only state that refuses, because starting a second
    /// analysis before the first returns is how two results race to populate
    /// one window.
    #[must_use]
    pub const fn accepts_selection(self) -> bool {
        !matches!(self, Self::Analyzing)
    }

    /// Whether this state is showing something the user can look at.
    #[must_use]
    pub const fn has_result(self) -> bool {
        matches!(self, Self::Ready)
    }
}
