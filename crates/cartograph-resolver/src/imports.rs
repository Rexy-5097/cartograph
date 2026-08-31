//! Resolving a name to the declaration it refers to.
//!
//! ```text
//! name at a use site ─► import statement ─► module ─► exported symbol ─► declaration
//! ```
//!
//! # Why this exists
//!
//! M07 measured 286 `OrmAccess` edges whose model name was imported from
//! somewhere other than the model's own module. Matching a name against a
//! project-wide map cannot tell `User` the Flask-AppBuilder model from `User`
//! the diagram node, and a wrong edge is worse than a missing one.
//!
//! # What it will not do
//!
//! - **Guess.** An unresolvable module is [`Resolution::Unresolved`], and two
//!   equally good declarations are [`Resolution::Ambiguous`]. Neither becomes
//!   a relationship.
//! - **Reimplement the language's resolver.** Namespace packages, `sys.path`
//!   manipulation, editable installs and conditional imports are all out of
//!   scope. The supported subset is written down in
//!   `docs/resolver/imports.md` and every entry in it has a fixture.
//! - **Rank candidates.** There is no "best" match. There is a match, or a
//!   refusal.

use std::collections::HashMap;

use cartograph_parser::model::{FileAnalysis, SourceLanguage, Symbol, SymbolKind};

use crate::dependencies::ResolutionContext;

/// Where a name's declaration is, or why that is not known.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Resolution {
    /// The declaration was found, exactly once.
    Declared {
        /// Repository-relative file holding the declaration.
        file: String,
        /// Line the declaration starts on.
        line: u32,
        /// How it was reached, for the edge's evidence.
        route: String,
    },
    /// The name is declared in the file that uses it.
    Local {
        /// Line the declaration starts on.
        line: u32,
    },
    /// More than one declaration is equally plausible.
    ///
    /// Never resolved by picking one: which declaration was meant is an
    /// unanswered question, and an edge is an answer.
    Ambiguous {
        /// The files that each declare the name.
        candidates: Vec<String>,
    },
    /// An import binds the name but its module is not in the project.
    Unresolved {
        /// The specifier as written.
        module: String,
        /// Why, for the record.
        reason: String,
    },
    /// Nothing in this file binds the name.
    ///
    /// Distinct from [`Resolution::Unresolved`]: a star import, a conditional
    /// import or a builtin all land here, and callers decide what that means
    /// for them rather than being forced into a refusal.
    NotBound,
}

impl Resolution {
    /// The declaration's file, when there is exactly one.
    #[must_use]
    pub fn declared_in(&self) -> Option<&str> {
        match self {
            Self::Declared { file, .. } => Some(file),
            _ => None,
        }
    }

    /// Whether this resolution names a single declaration.
    #[must_use]
    pub fn is_definite(&self) -> bool {
        matches!(self, Self::Declared { .. } | Self::Local { .. })
    }

    /// A phrase for an edge's evidence. Never contains source text.
    #[must_use]
    pub fn explain(&self) -> String {
        match self {
            Self::Declared { file, line, route } => {
                format!("declared at {file}:{line} via {route}")
            }
            Self::Local { line } => format!("declared in the same file at line {line}"),
            Self::Ambiguous { candidates } => {
                format!("ambiguous: declared in {}", candidates.join(", "))
            }
            Self::Unresolved { module, reason } => format!("`{module}` {reason}"),
            Self::NotBound => "not bound in this file".to_owned(),
        }
    }
}

/// Every analysed file, addressable by path.
///
/// Built once per analysis. Holds borrows rather than copies: an analysis of a
/// large repository is already the dominant allocation.
pub struct ModuleIndex<'a> {
    by_path: HashMap<&'a str, &'a FileAnalysis>,
    /// Module aliases the project's build configuration declares, each with
    /// the directory its target is relative to.
    aliases: Vec<(String, String, String)>,
}

/// How far a re-export chain is followed.
///
/// One package re-exporting another's symbol is ordinary; a chain longer than
/// this is either generated indirection or a cycle, and stopping is safer than
/// continuing. Each hop is recorded in the resolution's route.
const MAX_REEXPORT_HOPS: usize = 4;

impl<'a> ModuleIndex<'a> {
    /// Indexes an analysed project.
    #[must_use]
    pub fn build(files: &'a [FileAnalysis]) -> Self {
        let mut aliases: Vec<(String, String, String)> = Vec::new();
        for file in files {
            let dir = file
                .path
                .rsplit_once('/')
                .map_or(String::new(), |(d, _)| d.to_owned());
            for alias in &file.module_aliases {
                aliases.push((alias.alias.clone(), alias.target.clone(), dir.clone()));
            }
        }
        // Nearest configuration first, then longest alias. A monorepo has one
        // build configuration per frontend — Airflow has five — and an alias
        // named `src` or `openapi` means something different in each. Scoping
        // by the configuration's own directory is what makes the right one
        // win; a global list picks whichever happened to sort first and
        // resolves imports into a different project entirely.
        aliases.sort_by(|a, b| {
            b.2.len()
                .cmp(&a.2.len())
                .then_with(|| b.0.len().cmp(&a.0.len()))
                .then_with(|| a.0.cmp(&b.0))
        });

        Self {
            by_path: files.iter().map(|f| (f.path.as_str(), f)).collect(),
            aliases,
        }
    }

    /// Rewrites a bare specifier through a declared build alias.
    ///
    /// Returns the repository-relative path the specifier stands for, before
    /// extension resolution. A target beginning `/` is relative to the
    /// configuration file's own directory, which is how Vite reads it.
    fn apply_alias(&self, specifier: &str, importing_file: &str) -> Option<String> {
        for (alias, target, dir) in &self.aliases {
            // The configuration only governs files beneath it.
            if !dir.is_empty() && !importing_file.starts_with(&format!("{dir}/")) {
                continue;
            }
            let rest = if specifier == alias {
                ""
            } else if let Some(rest) = specifier.strip_prefix(&format!("{alias}/")) {
                rest
            } else {
                continue;
            };
            let base = target.strip_prefix('/').map_or_else(
                || target.clone(),
                |relative| {
                    if dir.is_empty() {
                        relative.to_owned()
                    } else {
                        format!("{dir}/{relative}")
                    }
                },
            );
            return Some(if rest.is_empty() {
                base
            } else {
                format!("{base}/{rest}")
            });
        }
        None
    }

    /// The analysis for a repository-relative path.
    #[must_use]
    pub fn file(&self, path: &str) -> Option<&'a FileAnalysis> {
        self.by_path.get(path).copied()
    }

    /// Resolves a Python name used in `from` to its declaration.
    #[must_use]
    pub fn resolve_python(&self, from: &FileAnalysis, name: &str) -> Resolution {
        self.resolve_python_tracked(from, name, &mut ResolutionContext::new())
    }

    /// [`resolve_python`](Self::resolve_python), recording what it read.
    ///
    /// The untracked form delegates here, so there is exactly one
    /// implementation of Python resolution and the recorded dependencies
    /// cannot drift away from the behaviour they describe.
    pub fn resolve_python_tracked(
        &self,
        from: &FileAnalysis,
        name: &str,
        deps: &mut ResolutionContext,
    ) -> Resolution {
        // The asking file's own symbols decide whether the name is local, so
        // its contents are a dependency of every answer.
        deps.record_file(&from.path);
        if let Some(symbol) = declaration_in(from, name) {
            return Resolution::Local {
                line: symbol.span.start_line,
            };
        }
        self.follow_python(from, name, 0, deps)
    }

    fn follow_python(
        &self,
        from: &FileAnalysis,
        name: &str,
        hop: usize,
        deps: &mut ResolutionContext,
    ) -> Resolution {
        if hop >= MAX_REEXPORT_HOPS {
            return Resolution::Unresolved {
                module: from.path.clone(),
                reason: format!("re-export chain exceeded {MAX_REEXPORT_HOPS} hops"),
            };
        }

        // This hop reads `from`'s import statements.
        deps.record_file(&from.path);
        let Some(import) = python_import_binding(from, name) else {
            return Resolution::NotBound;
        };
        // The name the exporting module knows it by, which an alias changes.
        let exported = import
            .named
            .iter()
            .find(|n| n.local.as_deref().unwrap_or(n.imported.as_str()) == name)
            .map_or(name, |n| n.imported.as_str());

        // Matching a dotted specifier scans the repository's file paths and
        // answers only when exactly one matches, so the answer depends on
        // which files exist — not on any file's contents.
        deps.record_path_set();
        let Some(target) = self.resolve_python_module(&import.specifier, &from.path) else {
            return Resolution::Unresolved {
                module: import.specifier.clone(),
                reason: "is not a module this project contains".to_owned(),
            };
        };
        deps.record_file(&target);
        let Some(module) = self.file(&target) else {
            return Resolution::Unresolved {
                module: import.specifier.clone(),
                reason: "resolved to a file that was not analysed".to_owned(),
            };
        };

        if let Some(symbol) = declaration_in(module, exported) {
            return Resolution::Declared {
                file: module.path.clone(),
                line: symbol.span.start_line,
                route: format!("`from {} import {exported}`", import.specifier),
            };
        }
        // The module re-exports it; follow one hop further. Each hop is kept
        // in the route so the evidence shows the whole path a reader would
        // have to walk by hand, not just its last step.
        let hop_taken = format!("`from {} import {exported}`", import.specifier);
        match self.follow_python(module, exported, hop + 1, deps) {
            Resolution::Local { line } => Resolution::Declared {
                file: module.path.clone(),
                line,
                route: hop_taken,
            },
            Resolution::Declared { file, line, route } => Resolution::Declared {
                file,
                line,
                route: format!("{hop_taken} then {route}"),
            },
            Resolution::NotBound => Resolution::Unresolved {
                module: import.specifier.clone(),
                reason: format!("does not declare or re-export `{exported}`"),
            },
            other => other,
        }
    }

    /// Resolves a `TypeScript` name used in `from` to its declaration.
    ///
    /// Supports the forms the corpus uses: named, default and namespace
    /// imports, and `export { X } from "./m"` re-exports. Only **relative**
    /// specifiers are followed. A bare specifier is either a package or a
    /// build-tool alias, and resolving an alias means reading a bundler
    /// configuration — a different mechanism, declared out of scope, and
    /// reported as [`Resolution::Unresolved`] rather than guessed at.
    #[must_use]
    pub fn resolve_typescript(&self, from: &FileAnalysis, name: &str) -> Resolution {
        if let Some(symbol) = declaration_in(from, name) {
            return Resolution::Local {
                line: symbol.span.start_line,
            };
        }
        self.follow_typescript(from, name, 0)
    }

    fn follow_typescript(&self, from: &FileAnalysis, name: &str, hop: usize) -> Resolution {
        if hop >= MAX_REEXPORT_HOPS {
            return Resolution::Unresolved {
                module: from.path.clone(),
                reason: format!("re-export chain exceeded {MAX_REEXPORT_HOPS} hops"),
            };
        }

        for import in &from.imports {
            let exported = if import
                .named
                .iter()
                .any(|n| n.local.as_deref().unwrap_or(n.imported.as_str()) == name)
            {
                import
                    .named
                    .iter()
                    .find(|n| n.local.as_deref().unwrap_or(n.imported.as_str()) == name)
                    .map_or(name, |n| n.imported.as_str())
            } else if import.default_name.as_deref() == Some(name) {
                "default"
            } else if import.namespace_name.as_deref() == Some(name) {
                // `import * as m from "./m"` binds the module, not a symbol.
                name
            } else {
                continue;
            };

            // A bare specifier is a package unless the build configuration
            // says otherwise. `openapi/queries` is only followable because
            // vite.config.ts declares `resolve: { alias: { openapi: … } }`.
            let specifier = if import.specifier.starts_with('.') {
                import.specifier.clone()
            } else if let Some(aliased) = self.apply_alias(&import.specifier, &from.path) {
                aliased
            } else {
                return Resolution::Unresolved {
                    module: import.specifier.clone(),
                    reason: "is a package, and no build configuration aliases it".to_owned(),
                };
            };
            let Some(target) = self.resolve_relative_module(&specifier, &from.path) else {
                return Resolution::Unresolved {
                    module: import.specifier.clone(),
                    reason: "does not resolve to a file this project contains".to_owned(),
                };
            };
            let Some(module) = self.file(&target) else {
                return Resolution::Unresolved {
                    module: import.specifier.clone(),
                    reason: "resolved to a file that was not analysed".to_owned(),
                };
            };
            if let Some(symbol) = declaration_in(module, exported) {
                return Resolution::Declared {
                    file: module.path.clone(),
                    line: symbol.span.start_line,
                    route: format!("`import {{ {exported} }} from \"{}\"`", import.specifier),
                };
            }
            let hop_taken = format!("`from \"{}\"`", import.specifier);
            return match self.follow_typescript(module, exported, hop + 1) {
                Resolution::Local { line } => Resolution::Declared {
                    file: module.path.clone(),
                    line,
                    route: hop_taken,
                },
                Resolution::Declared { file, line, route } => Resolution::Declared {
                    file,
                    line,
                    route: format!("{hop_taken} then {route}"),
                },
                other => other,
            };
        }

        // No named binding. A barrel file forwards everything it re-exports,
        // so each wildcard is tried and the answer must be unique: two barrels
        // exporting the same name says nothing about which was meant.
        self.follow_typescript_wildcards(from, name, hop)
    }

    fn follow_typescript_wildcards(
        &self,
        from: &FileAnalysis,
        name: &str,
        hop: usize,
    ) -> Resolution {
        let mut found: Vec<Resolution> = Vec::new();
        for import in &from.imports {
            if import.namespace_name.as_deref() != Some("*") {
                continue;
            }
            let specifier = if import.specifier.starts_with('.') {
                import.specifier.clone()
            } else if let Some(aliased) = self.apply_alias(&import.specifier, &from.path) {
                aliased
            } else {
                continue;
            };
            let Some(target) = self.resolve_relative_module(&specifier, &from.path) else {
                continue;
            };
            let Some(module) = self.file(&target) else {
                continue;
            };
            if let Some(symbol) = declaration_in(module, name) {
                found.push(Resolution::Declared {
                    file: module.path.clone(),
                    line: symbol.span.start_line,
                    route: format!("`export * from \"{}\"`", import.specifier),
                });
                continue;
            }
            if hop + 1 < MAX_REEXPORT_HOPS {
                match self.follow_typescript(module, name, hop + 1) {
                    r @ Resolution::Declared { .. } => found.push(r),
                    Resolution::Local { line } => found.push(Resolution::Declared {
                        file: module.path.clone(),
                        line,
                        route: format!("`export * from \"{}\"`", import.specifier),
                    }),
                    _ => {}
                }
            }
        }
        match found.len() {
            0 => Resolution::NotBound,
            1 => found.remove(0),
            _ => {
                let mut candidates: Vec<String> = found
                    .iter()
                    .filter_map(|r| r.declared_in().map(ToOwned::to_owned))
                    .collect();
                candidates.sort();
                candidates.dedup();
                if candidates.len() == 1 {
                    found.remove(0)
                } else {
                    Resolution::Ambiguous { candidates }
                }
            }
        }
    }

    /// Resolves a relative `TypeScript` specifier to an analysed file.
    ///
    /// Extension and index resolution only — no `node_modules` walk, no
    /// `tsconfig` path mapping, no conditional exports.
    #[must_use]
    pub fn resolve_relative_module(&self, specifier: &str, from: &str) -> Option<String> {
        // An aliased specifier is already repository-relative; only a `.`-led
        // one is relative to the importing file.
        let dir = if specifier.starts_with('.') {
            from.rsplit_once('/').map_or("", |(d, _)| d)
        } else {
            ""
        };
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
            format!("{base}/index.ts"),
            format!("{base}/index.tsx"),
            base.clone(),
        ]
        .into_iter()
        .find(|candidate| self.by_path.contains_key(candidate.as_str()))
    }

    /// Resolves a Python module specifier to an analysed file.
    ///
    /// Handles the three forms the corpus uses: an absolute dotted path, a
    /// relative path with leading dots, and a package resolved through its
    /// `__init__.py`. Anything else is unresolved rather than approximated.
    #[must_use]
    pub fn resolve_python_module(&self, specifier: &str, from: &str) -> Option<String> {
        let dots = specifier.len() - specifier.trim_start_matches('.').len();
        let rest = specifier.trim_start_matches('.');

        let candidates: Vec<String> = if dots > 0 {
            // Relative: climb `dots - 1` package levels from this file's own.
            let mut segments: Vec<&str> = from.split('/').collect();
            segments.pop(); // the file itself
            for _ in 0..dots.saturating_sub(1) {
                segments.pop()?;
            }
            let mut base = segments.join("/");
            if !rest.is_empty() {
                if !base.is_empty() {
                    base.push('/');
                }
                base.push_str(&rest.replace('.', "/"));
            }
            vec![format!("{base}.py"), format!("{base}/__init__.py")]
        } else {
            // Absolute: the project may be rooted under any number of source
            // directories (`backend/`, `src/`, `airflow-core/src/`), so the
            // dotted path is matched as a path *suffix* anchored at a
            // separator — never as a substring, which would let `ics.models`
            // be satisfied by `analytics/models.py`.
            let base = rest.replace('.', "/");
            vec![format!("{base}.py"), format!("{base}/__init__.py")]
        };

        let mut found: Vec<&str> = Vec::new();
        for candidate in &candidates {
            for path in self.by_path.keys() {
                if *path == candidate.as_str() || path.ends_with(&format!("/{candidate}")) {
                    found.push(path);
                }
            }
            if !found.is_empty() {
                break;
            }
        }
        match found.len() {
            1 => Some(found[0].to_owned()),
            // A dotted path that matches several files in a monorepo says
            // nothing about which one was imported.
            _ => None,
        }
    }
}

/// The import statement in `file` that binds `name`, if any.
fn python_import_binding<'f>(
    file: &'f FileAnalysis,
    name: &str,
) -> Option<&'f cartograph_parser::model::Import> {
    file.imports.iter().find(|import| {
        import
            .named
            .iter()
            .any(|n| n.local.as_deref().unwrap_or(n.imported.as_str()) == name)
    })
}

/// A module-scope declaration of `name` in one file.
fn declaration_in<'f>(file: &'f FileAnalysis, name: &str) -> Option<&'f Symbol> {
    file.symbols.iter().find(|s| {
        s.name == name
            && s.enclosing.is_none()
            && matches!(
                s.kind,
                SymbolKind::Class | SymbolKind::Function | SymbolKind::Variable
            )
    })
}

/// Whether a file is Python, for callers that must not cross languages.
#[must_use]
pub fn is_python(file: &FileAnalysis) -> bool {
    file.language == SourceLanguage::Python
}
