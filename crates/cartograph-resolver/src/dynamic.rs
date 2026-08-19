//! Dynamic URL resolution: from a project's files to a canonical client route.
//!
//! ```text
//! project files ─► exported constants ─► per-file Scope
//!                                             │
//! HTTP call URL ──────────────────────────────┴─► SymbolicValue
//!                                                        │
//!                                     partial reconstruction (slice 7)
//!                                                        │
//!                                          UrlObservation ─► M04 matcher
//! ```
//!
//! # Why this feeds the existing matcher rather than replacing it
//!
//! M05 changes what a client URL *is known to be*; it does not change what
//! counts as a match. A resolved URL becomes an ordinary
//! [`UrlObservation`] and flows through M03 canonicalisation and the M04
//! matcher unchanged, so every rule M04 established — static segments compare
//! exactly, ambiguity produces no edge, a route with no discriminating static
//! segment is never accepted — applies to dynamic URLs automatically.
//!
//! That matters more than it sounds. The obvious failure mode for this
//! milestone is that resolving more URLs produces more matches, including
//! wrong ones. Reusing the matcher means a dynamic URL earns an edge under
//! exactly the same evidence a literal one would.
//!
//! # Unknown stays unknown
//!
//! A segment whose value is not determined becomes a template *expression*,
//! which M03 canonicalises to `{*}` and M04 treats as an undetermined
//! position. It never becomes text, and a URL whose *prefix* is unknown is
//! still refused by the matcher, because its segment count is unknown.

use std::collections::HashMap;

use cartograph_parser::model::{
    FileAnalysis, HttpCallObservation, Span, StringFact, TemplatePart, UrlObservation,
};

use crate::evaluator::{Scope, evaluate_expression, evaluate_string_fact};
use crate::symbolic::SymbolicValue;

/// A URL resolved as far as the source determines.
#[derive(Debug, Clone, PartialEq)]
pub struct ResolvedUrl {
    /// The symbolic value the evaluator produced.
    pub value: SymbolicValue,
    /// The value re-expressed for the existing canonicalisation pipeline.
    pub observation: UrlObservation,
    /// How the evaluator got here, for the edge's evidence.
    pub derivation: Vec<String>,
}

impl ResolvedUrl {
    /// Whether every part of the URL is determined.
    #[must_use]
    pub fn is_fully_known(&self) -> bool {
        self.value.is_fully_known()
    }

    /// A one-line account of what was resolved and what was not.
    ///
    /// This is what an edge's evidence carries, so it names the constants that
    /// resolved and the reason each gap remains — never a value that was not
    /// determined.
    #[must_use]
    pub fn explain(&self) -> String {
        let mut parts = vec![format!("derived {}", self.value)];
        parts.extend(self.derivation.iter().cloned());
        if let Some(reason) = self.value.unresolved_reason() {
            parts.push(reason);
        }
        parts.join("; ")
    }
}

/// Constants a project exports, keyed by exporting file then name.
///
/// Built once per analysis and consulted when a file's imports are resolved.
pub type ExportedConstants = HashMap<String, HashMap<String, SymbolicValue>>;

/// Collects every module-scope string constant each file exports.
///
/// Slice 3. Only exported constants are collected: a file's private constants
/// are not visible to its importers, and pretending otherwise would resolve
/// values the importing module cannot actually see.
#[must_use]
pub fn collect_exported_constants(files: &[FileAnalysis]) -> ExportedConstants {
    let mut out = ExportedConstants::new();
    for file in files {
        let scope = Scope::from_file(&file.symbols, &file.strings);
        let mut exported = HashMap::new();
        for symbol in &file.symbols {
            if symbol.enclosing.is_some() {
                continue;
            }
            let is_exported = matches!(
                symbol.export,
                cartograph_parser::model::ExportStatus::Exported
                    | cartograph_parser::model::ExportStatus::DefaultExported
            );
            if !is_exported {
                continue;
            }
            if let Some(value) = scope.get(&symbol.name) {
                exported.insert(symbol.name.clone(), value.clone());
            }
        }
        if !exported.is_empty() {
            out.insert(file.path.clone(), exported);
        }
    }
    out
}

/// Builds a file's evaluation scope, including constants it imports.
///
/// Import specifiers are resolved against the exporting files by a restricted
/// path join — relative specifiers only. A bare specifier (`"axios"`) names a
/// package, not a project file, and is not followed.
#[must_use]
pub fn scope_for_file(file: &FileAnalysis, exported: &ExportedConstants) -> Scope {
    let mut scope = Scope::from_file(&file.symbols, &file.strings);

    for import in &file.imports {
        if !import.specifier.starts_with('.') {
            continue; // a package, not a project file
        }
        let Some(target) = resolve_relative(&file.path, &import.specifier, exported) else {
            continue;
        };
        let Some(available) = exported.get(&target) else {
            continue;
        };
        // Only names this file actually imports are bound.
        let bindings: Vec<(String, SymbolicValue)> = import
            .named
            .iter()
            .filter_map(|named| {
                let local = named
                    .local
                    .clone()
                    .unwrap_or_else(|| named.imported.clone());
                available.get(&named.imported).map(|v| (local, v.clone()))
            })
            .collect();
        scope.extend_with_imports(bindings);
    }
    scope
}

/// Resolves a relative import specifier to a known file path.
///
/// Deliberately restricted: no `node_modules` walk, no `tsconfig` path
/// aliases, no index resolution beyond the common extensions. An unresolved
/// specifier simply contributes no bindings, so its constants stay unknown.
fn resolve_relative(from: &str, specifier: &str, exported: &ExportedConstants) -> Option<String> {
    let dir = from.rsplit_once('/').map_or("", |(d, _)| d);
    let mut segments: Vec<&str> = if dir.is_empty() {
        Vec::new()
    } else {
        dir.split('/').collect()
    };
    for part in specifier.split('/') {
        match part {
            "." | "" => {}
            ".." => {
                segments.pop()?;
            }
            other => segments.push(other),
        }
    }
    let base = segments.join("/");
    [
        format!("{base}.ts"),
        format!("{base}.tsx"),
        format!("{base}.py"),
        format!("{base}/index.ts"),
        format!("{base}/index.tsx"),
        base.clone(),
    ]
    .into_iter()
    .find(|candidate| exported.contains_key(candidate))
}

/// Resolves one HTTP call's URL against a scope.
///
/// Returns `None` when the observation carries nothing to improve on — a URL
/// already literal needs no evaluation, and one the parser could not represent
/// cannot be evaluated.
#[must_use]
pub fn resolve_url(
    call: &HttpCallObservation,
    file: &FileAnalysis,
    scope: &Scope,
) -> Option<ResolvedUrl> {
    let (value, derivation) = match &call.url {
        // Already determined; M05 has nothing to add.
        UrlObservation::Literal { .. } => return None,

        UrlObservation::Template { parts } => {
            let fact = StringFact::Template {
                parts: parts.clone(),
                span: call.span,
            };
            let value = evaluate_string_fact(&fact, scope);
            (value, describe_substitutions(parts, scope))
        }

        // A dynamic URL is a bare expression — an identifier naming a
        // constant, most usefully. Resolving it is exactly slice 3's payoff.
        UrlObservation::Dynamic { source } => {
            let value = evaluate_expression(source, scope);
            let note = match value.as_literal() {
                Some(_) => vec![format!("`{source}` resolved from a module constant")],
                None => Vec::new(),
            };
            (value, note)
        }
    };

    // Find the file's own imports in the evidence when a constant came from
    // elsewhere; cheap and useful when reading an edge later.
    let _ = file;

    Some(ResolvedUrl {
        observation: to_observation(&value, call.span),
        value,
        derivation,
    })
}

/// Notes which substitutions resolved and which did not.
fn describe_substitutions(parts: &[TemplatePart], scope: &Scope) -> Vec<String> {
    parts
        .iter()
        .filter_map(|part| match part {
            TemplatePart::Text { .. } => None,
            TemplatePart::Expression { source, .. } => {
                let value = evaluate_expression(source, scope);
                value
                    .as_literal()
                    .map(|resolved| format!("`{source}` resolved to \"{resolved}\""))
            }
        })
        .collect()
}

/// Re-expresses a symbolic value as an observation the existing pipeline
/// understands.
///
/// Slice 7, and the point at which M05 hands back to M03/M04. A fully
/// determined value becomes a literal; a partially determined one becomes a
/// template whose unresolved parts are expressions, which canonicalisation
/// turns into `{*}` positions. Nothing is flattened into text.
fn to_observation(value: &SymbolicValue, span: Span) -> UrlObservation {
    if let Some(literal) = value.as_literal() {
        return UrlObservation::Literal { value: literal };
    }

    let mut parts = Vec::new();
    for part in value.parts() {
        match part {
            SymbolicValue::Literal { value } => parts.push(TemplatePart::Text {
                value: value.clone(),
            }),
            SymbolicValue::Unknown { name } => parts.push(TemplatePart::Expression {
                source: name.clone(),
                span,
            }),
            SymbolicValue::EnvVar { name } => parts.push(TemplatePart::Expression {
                // Kept distinguishable in evidence; still an unresolved position.
                source: format!("process.env.{name}"),
                span,
            }),
            SymbolicValue::Unsupported { source } => parts.push(TemplatePart::Expression {
                source: source.clone(),
                span,
            }),
            SymbolicValue::Concat { .. } => {
                // `normalized()` removes nesting before this point.
                parts.push(TemplatePart::Expression {
                    source: part.to_string(),
                    span,
                });
            }
        }
    }
    UrlObservation::Template { parts }
}

/// Rewrites an HTTP call observation with its resolved URL.
///
/// The call is otherwise untouched — same callee, same method hint, same span —
/// so downstream stages cannot tell a resolved URL from a literal one except
/// by the evidence, which is the point.
#[must_use]
pub fn with_resolved_url(
    call: &HttpCallObservation,
    resolved: &ResolvedUrl,
) -> HttpCallObservation {
    HttpCallObservation {
        callee: call.callee.clone(),
        method_hint: call.method_hint,
        url: resolved.observation.clone(),
        span: call.span,
    }
}
