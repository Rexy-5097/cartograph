//! Language-specific extraction: the first tier of Cartograph's two-tier
//! analysis (ADR-0002).
//!
//! This crate answers **"what does this file say?"** — declared symbols,
//! imports, call sites, string and template structure, HTTP-looking call
//! shapes — as [`model::FileAnalysis`] facts with precise, repository-relative
//! source locations.
//!
//! It deliberately does *not* answer "what does this name refer to?". Nothing
//! here resolves a symbol, matches a route, or produces a graph edge: facts
//! are observations for the resolver (M03–M06) to verify. The distinction
//! between observed syntax and verified semantic relationship is central to
//! Cartograph — see [`model`].
//!
//! # Boundary
//!
//! ```text
//! source text ──► Analyzer ──► FileAnalysis (facts + diagnostics)
//! ```
//!
//! tree-sitter is an implementation detail of [`typescript`] (private) and
//! appears nowhere in this crate's public API, so adding Python at M02 — or
//! changing the parsing runtime — cannot ripple outward.
//!
//! # Error tolerance
//!
//! A malformed file yields a [`model::ParseStatus::CompleteWithErrors`]
//! analysis with facts from the well-formed regions plus structured
//! [`diagnostics`]; an unreadable or non-UTF-8 file yields
//! [`model::ParseStatus::Failed`] with a diagnostic. Neither aborts a
//! repository walk. [`ParserError`] is reserved for caller mistakes (bad path,
//! unsupported extension) and environment failures (grammar load).
//!
//! Languages at M01: TypeScript and TSX. Python arrives at M02.

pub mod diagnostics;
pub mod error;
pub mod model;

mod typescript;

use std::path::Path;

pub use error::ParserError;

use diagnostics::{Diagnostic, DiagnosticKind, Severity};
use model::{FileAnalysis, ParseStatus, SourceLanguage};

/// Reusable analyzer holding the loaded grammars.
///
/// Construction loads the grammars once; `analyze_*` calls reuse them. The
/// analyzer is deliberately `!Sync` cheap state — for parallel walks, create
/// one per worker (planned for the milestone that measures the need).
pub struct Analyzer {
    typescript: typescript::TypeScriptExtractor,
}

impl Analyzer {
    /// Creates an analyzer with the TypeScript and TSX grammars loaded.
    ///
    /// # Errors
    ///
    /// [`ParserError::Grammar`] if a grammar cannot be loaded into the
    /// runtime — an installation problem, not a source problem.
    pub fn new() -> Result<Self, ParserError> {
        Ok(Self {
            typescript: typescript::TypeScriptExtractor::new()?,
        })
    }

    /// Analyses in-memory source as `rel_path`.
    ///
    /// `rel_path` must be repository-relative; it is validated with the same
    /// rules as the M00 domain model (no absolute paths, no `..`) and
    /// normalised to forward slashes.
    ///
    /// # Errors
    ///
    /// [`ParserError::InvalidPath`] for a path violating those rules, or
    /// [`ParserError::UnsupportedLanguage`] for an extension no extractor
    /// claims. Problems *inside* the source never error — they are
    /// diagnostics on the returned analysis.
    pub fn analyze_source(
        &mut self,
        rel_path: &str,
        source: &str,
    ) -> Result<FileAnalysis, ParserError> {
        let path = normalize_rel_path(rel_path)?;
        let language = SourceLanguage::from_path(&path)
            .ok_or(ParserError::UnsupportedLanguage { path: path.clone() })?;
        Ok(self.typescript.extract(&path, language, source))
    }

    /// Reads and analyses `repo_root`/`rel_path`.
    ///
    /// An unreadable or non-UTF-8 file returns a
    /// [`ParseStatus::Failed`] analysis with a diagnostic rather than an
    /// error, so a repository walk survives individual bad files.
    ///
    /// # Errors
    ///
    /// Same as [`Analyzer::analyze_source`]: only path and language problems.
    pub fn analyze_file(
        &mut self,
        repo_root: &Path,
        rel_path: &str,
    ) -> Result<FileAnalysis, ParserError> {
        let path = normalize_rel_path(rel_path)?;
        let language = SourceLanguage::from_path(&path)
            .ok_or(ParserError::UnsupportedLanguage { path: path.clone() })?;

        let bytes = match std::fs::read(repo_root.join(&path)) {
            Ok(bytes) => bytes,
            Err(e) => {
                return Ok(failed_analysis(
                    &path,
                    language,
                    DiagnosticKind::Unreadable,
                    // io::ErrorKind only - no OS message that could embed a path.
                    format!("file could not be read: {}", e.kind()),
                ));
            }
        };
        let Ok(source) = String::from_utf8(bytes) else {
            return Ok(failed_analysis(
                &path,
                language,
                DiagnosticKind::NotUtf8,
                "file is not valid UTF-8".to_owned(),
            ));
        };
        Ok(self.typescript.extract(&path, language, &source))
    }
}

/// Validates and normalises a repository-relative path.
///
/// Reuses the domain model's rules (`SourceLocation` rejects absolute paths
/// and `..` traversal) so the parser cannot leak a machine layout into
/// persisted analysis results, then normalises separators to `/`.
fn normalize_rel_path(rel_path: &str) -> Result<String, ParserError> {
    // Validation only; the location itself is discarded.
    cartograph_core::SourceLocation::new(rel_path, 1)?;
    Ok(rel_path.replace('\\', "/"))
}

fn failed_analysis(
    path: &str,
    language: SourceLanguage,
    kind: DiagnosticKind,
    message: String,
) -> FileAnalysis {
    FileAnalysis {
        path: path.to_owned(),
        language,
        status: ParseStatus::Failed,
        imports: Vec::new(),
        symbols: Vec::new(),
        calls: Vec::new(),
        strings: Vec::new(),
        http_calls: Vec::new(),
        diagnostics: vec![Diagnostic {
            severity: Severity::Error,
            kind,
            message,
            span: None,
        }],
    }
}
