//! MAP's Tauri shell.
//!
//! # This file should stay boring
//!
//! Its whole job is to open a window and expose two functions. Every decision
//! about what a repository *is* lives in `cartograph-desktop`, and every
//! decision about what a repository *contains* lives further down still, in
//! `cartograph-pipeline` and `cartograph-graph`.
//!
//! That is not tidiness for its own sake. This crate is deliberately outside
//! the cargo workspace — it links a system webview, which the M00–M10 CI jobs
//! have never needed — so it is the one crate `cargo test --workspace` does
//! **not** cover. Anything that can be wrong belongs on the other side of that
//! line, where the gates already run. See ADR-0016.
//!
//! If a command here grows a branch, that branch is in the wrong file.

// The console window is Windows' default for a binary; a desktop application
// showing one behind its window looks broken. Release only, so `println!`
// debugging still works during development.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::sync::Mutex;

use cartograph_desktop::ask::{self, AskAnswer};
use cartograph_desktop::blast::{self, BlastPayload};
use cartograph_desktop::error::{DesktopError, DesktopErrorKind};
use cartograph_desktop::evidence::{AnalysisId, AnalysisSession, EvidenceRecord};
use cartograph_desktop::repository::{self, ValidatedRepository};
use cartograph_desktop::optin::{self, GrantedRepository};
use cartograph_desktop::session::{self, AnalysisPayload};

/// The one piece of state the shell keeps.
///
/// Holds the most recent analysis so its evidence can be queried without
/// re-analysing. A `Mutex` rather than anything cleverer: commands are
/// infrequent and the critical sections are a pointer swap and a graph lookup.
#[derive(Default)]
struct Current(Mutex<Option<AnalysisSession>>);

/// The repository the human selected, and its ASK opt-in.
///
/// Held apart from `Current` because the identity belongs to the
/// *repository*, and one repository is analysed many times. Every decision
/// about it lives in `cartograph_desktop::optin` (ADR-0020 Amendment 3).
#[derive(Default)]
struct Grant(Mutex<Option<GrantedRepository>>);

/// Checks a chosen path without analysing it.
///
/// Exposed separately so the window can report a bad selection immediately
/// rather than after a long analysis that was never going to work.
///
/// Returns the safe-to-show name — never the path itself, which stays on this
/// side of the boundary.
#[tauri::command]
fn validate_repository(path: String) -> Result<String, DesktopError> {
    validated(&path).map(|repository| repository.display_name().to_owned())
}

/// Validates, analyses, and lays out.
///
/// Runs on Tauri's blocking pool rather than the async runtime: the analysis
/// is CPU-bound and synchronous, and holding an async worker for the duration
/// would stall every other command on a large repository.
#[tauri::command(async)]
fn analyze_repository(
    path: String,
    current: tauri::State<'_, Current>,
    grant: tauri::State<'_, Grant>,
) -> Result<AnalysisPayload, DesktopError> {
    let repository = validated(&path)?;

    // Each analysis gets the next id, so a selection made against the previous
    // graph cannot be answered by this one.
    let id = {
        let held = current.0.lock().map_err(|_| poisoned())?;
        held.as_ref().map_or(AnalysisId::first(), |s| s.id().next())
    };

    // The desktop acting as grant producer, after an explicit human
    // selection (ADR-0020 Amendment 3). Settings that cannot be read leave the
    // grant unset, so ASK reads as off rather than as anything else.
    *grant.0.lock().map_err(|_| poisoned())? =
        GrantedRepository::establish(repository.path(), optin::settings_path()).ok();

    let (payload, session) = session::analyze_as(&repository, id)?;

    // Published only after the analysis succeeded: a failed run must not
    // replace a good graph with nothing.
    *current.0.lock().map_err(|_| poisoned())? = Some(session);
    Ok(payload)
}

/// The evidence behind one edge.
///
/// Takes the analysis id the window was looking at, not just the edge. That is
/// the whole safety property: `EdgeId` restarts at zero per analysis, so an id
/// from a replaced graph would otherwise resolve to a different relationship
/// and return confident, wrong evidence.
#[tauri::command]
fn edge_evidence(
    analysis: AnalysisId,
    edge: u64,
    current: tauri::State<'_, Current>,
) -> Result<EvidenceRecord, DesktopError> {
    let held = current.0.lock().map_err(|_| poisoned())?;
    let session = held.as_ref().ok_or_else(|| {
        DesktopError::new(
            DesktopErrorKind::NoAnalysis,
            "No repository has been analysed yet.",
        )
    })?;
    session.evidence(analysis, cartograph_core::EdgeId::from_raw(edge))
}

/// What depends on one artefact.
///
/// Takes the analysis id the window was looking at for the same reason
/// `edge_evidence` does: `NodeId` restarts at zero per analysis, so an id from
/// a replaced graph would otherwise resolve to a different artefact and
/// highlight a confident, wrong impact set.
///
/// Runs on the blocking pool: a blast radius over a large graph is CPU-bound,
/// and holding an async worker for it would stall other commands.
#[tauri::command(async)]
fn blast_radius(
    analysis: AnalysisId,
    node: u64,
    current: tauri::State<'_, Current>,
) -> Result<BlastPayload, DesktopError> {
    let held = current.0.lock().map_err(|_| poisoned())?;
    let session = held.as_ref().ok_or_else(|| {
        DesktopError::new(
            DesktopErrorKind::NoAnalysis,
            "No repository has been analysed yet.",
        )
    })?;
    blast::radius(session, analysis, cartograph_core::NodeId::from_raw(node))
}

/// The derived evidence behind one artefact, with no model involved.
///
/// M16's degraded path: no key, no network, no provider. What comes back is
/// the evidence the analysis already produced, in the bundle's deterministic
/// order, and the payload says `ai: "disabled"` so the window states that from
/// the data rather than assuming it.
///
/// Takes the analysis id for the same reason its neighbours do: `NodeId`
/// restarts at zero per analysis, so an id from a replaced graph would explain
/// a different artefact confidently and wrongly.
///
/// Runs on the blocking pool: walking a large graph is CPU-bound, and holding
/// an async worker for it would stall other commands.
#[tauri::command(async)]
fn ask_evidence(
    analysis: AnalysisId,
    node: u64,
    current: tauri::State<'_, Current>,
) -> Result<AskAnswer, DesktopError> {
    let held = current.0.lock().map_err(|_| poisoned())?;
    let session = held.as_ref().ok_or_else(|| {
        DesktopError::new(
            DesktopErrorKind::NoAnalysis,
            "No repository has been analysed yet.",
        )
    })?;
    ask::answer(session, analysis, cartograph_core::NodeId::from_raw(node))
}

/// Whether ASK is enabled for the selected repository.
///
/// The only thing about the grant that crosses to the window: a boolean. The
/// identity never leaves Rust (ADR-0020 Amendment 3).
#[tauri::command]
fn ask_enabled(grant: tauri::State<'_, Grant>) -> Result<bool, DesktopError> {
    let held = grant.0.lock().map_err(|_| poisoned())?;
    Ok(held.as_ref().is_some_and(GrantedRepository::ask_enabled))
}

/// Turns ASK on or off for the selected repository, and remembers it.
///
/// Returns what is now true rather than what was asked for, so a save that
/// failed cannot leave the window showing an opt-in that was not recorded.
#[tauri::command]
fn set_ask_enabled(
    enabled: bool,
    grant: tauri::State<'_, Grant>,
) -> Result<bool, DesktopError> {
    let mut held = grant.0.lock().map_err(|_| poisoned())?;
    held.as_mut()
        .map_or(Ok(false), |granted| granted.set_ask_enabled(enabled))
}

/// A poisoned lock means a command panicked while holding it. That is a defect
/// here, not a user error, and it is reported as one rather than papered over.
fn poisoned() -> DesktopError {
    DesktopError::new(
        DesktopErrorKind::Internal,
        "The analysis session is unavailable after an earlier failure.",
    )
    .with_hint("Analyse the repository again.")
}

/// The single place a string becomes a repository.
///
/// Both commands funnel through here so neither can accidentally skip
/// validation — the type system then carries the guarantee, because
/// `session::analyze` accepts nothing else.
fn validated(path: &str) -> Result<ValidatedRepository, DesktopError> {
    repository::validate(std::path::Path::new(path))
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(Current::default())
        .manage(Grant::default())
        .invoke_handler(tauri::generate_handler![
            validate_repository,
            analyze_repository,
            edge_evidence,
            blast_radius,
            ask_evidence,
            ask_enabled,
            set_ask_enabled
        ])
        .run(tauri::generate_context!())
        .expect("the Tauri runtime failed to start");
}
