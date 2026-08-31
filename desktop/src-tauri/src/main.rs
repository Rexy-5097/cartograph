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

use cartograph_desktop::error::DesktopError;
use cartograph_desktop::repository::{self, ValidatedRepository};
use cartograph_desktop::session::{self, AnalysisPayload};

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
fn analyze_repository(path: String) -> Result<AnalysisPayload, DesktopError> {
    session::analyze(&validated(&path)?)
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
        .invoke_handler(tauri::generate_handler![
            validate_repository,
            analyze_repository
        ])
        .run(tauri::generate_context!())
        .expect("the Tauri runtime failed to start");
}
