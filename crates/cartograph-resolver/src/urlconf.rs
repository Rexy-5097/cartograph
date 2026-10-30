//! Composing the path a Django URL-conf entry is actually served on.
//!
//! ```text
//! path("api/instances/", include("project.license.urls"))   project/urls.py
//! path("admins/", InstanceAdminEndpoint.as_view())          project/license/urls.py
//!                                              ─► api/instances/admins/
//! ```
//!
//! # Why this is separate from [`crate::routers`]
//!
//! A `FastAPI` router is an object, and a route names its receiver. A Django
//! URL conf is a *module*, or a list inside one: an entry names no receiver,
//! and what mounts it is `include("dotted.path")`, `include(some_list)`,
//! `include([...])` or a spread `*other_urlpatterns`. The unit of composition
//! is therefore a file, or a span of one, not a name.
//!
//! Django composes by string concatenation — `"api/"` + `"instances/"` — so no
//! separator is added or removed here. `"api"` + `"x/"` really is `"apix/"`.
//!
//! # What refuses
//!
//! The same cases [`crate::routers`] refuses, for the same reason (RULE 009):
//! a mount whose prefix is not a plain string (including every `re_path`
//! regex), a module or list mounted in more than one place, a cycle, and a
//! target this analysis cannot find. Each leaves the entry's path exactly as
//! written; an unknown prefix never becomes an empty one.

use std::collections::{HashMap, HashSet};

use cartograph_parser::model::{
    FileAnalysis, IncludedUrlConf, RouteDeclarationStyle, RouteObservation, Span, SymbolKind,
    TemplatePart, UrlObservation,
};

use crate::imports::{ModuleIndex, Resolution};
use crate::routers::PrefixResolution;

/// How deep a chain of mounts may go before it is treated as a cycle.
const MAX_MOUNT_DEPTH: usize = 12;

/// One mount: where it is written, and the prefix it adds.
#[derive(Debug, Clone)]
struct Mount {
    /// The file the `include(...)` or spread is written in.
    owner: String,
    /// The mount itself, so the prefixes around it can be found.
    at: Span,
    /// The prefix it adds, when it is a plain string.
    prefix: Option<String>,
    /// Whether a prefix was present and could not be read.
    unresolved: bool,
}

/// A list mounted by name or in place: entries inside `span` are served
/// under `mount`, not under the file's own prefix.
#[derive(Debug, Clone)]
struct Region {
    span: Span,
    mount: Mount,
}

/// What a mount's target turned out to be.
enum Target {
    /// A whole URL-conf module.
    File(String),
    /// A list inside a file.
    Region(String, Span),
    /// Something this analysis could not find. Mounts nothing.
    Unknown,
}

/// Every Django mount in a project, and the prefix each entry composes to.
pub struct UrlConfIndex {
    /// Where each URL-conf module is mounted, keyed by the module's file.
    mounts: HashMap<String, Vec<Mount>>,
    /// Lists mounted on their own, keyed by the file holding the list.
    regions: HashMap<String, Vec<Region>>,
}

impl UrlConfIndex {
    /// Builds the index from an analysed project.
    #[must_use]
    pub fn build(files: &[FileAnalysis], modules: &ModuleIndex<'_>) -> Self {
        let mut mounts: HashMap<String, Vec<Mount>> = HashMap::new();
        let mut regions: HashMap<String, Vec<Region>> = HashMap::new();
        for file in files {
            for inclusion in &file.url_inclusions {
                let mount = Mount {
                    owner: file.path.clone(),
                    at: inclusion.span,
                    prefix: inclusion.prefix.clone(),
                    unresolved: inclusion.prefix_unresolved,
                };
                match resolve_target(modules, file, &inclusion.target) {
                    Target::File(path) => mounts.entry(path).or_default().push(mount),
                    Target::Region(path, span) => {
                        regions
                            .entry(path)
                            .or_default()
                            .push(Region { span, mount });
                    }
                    Target::Unknown => {}
                }
            }
        }
        Self { mounts, regions }
    }

    /// The prefix an entry at `span` in `file` is served under.
    #[must_use]
    pub fn prefix_for(&self, file: &str, span: Span) -> PrefixResolution {
        self.prefix_at(file, span, &mut HashSet::new(), 0)
    }

    /// Restates a URL-conf entry with its composed prefix applied.
    ///
    /// `None` when there is nothing to apply: not a URL-conf entry, an empty,
    /// unresolved or ambiguous prefix, a regex pattern (never interpreted), or
    /// a path this analysis cannot rebuild. The caller then keeps the entry as
    /// written, so composition can only ever make a path more complete.
    #[must_use]
    pub fn served(&self, file: &str, route: &RouteObservation) -> Option<RouteObservation> {
        if route.style != RouteDeclarationStyle::UrlConfEntry {
            return None;
        }
        let resolution = self.prefix_for(file, route.span);
        let prefix = resolution.known()?;
        if prefix.is_empty() {
            return None;
        }
        let path = match &route.path {
            UrlObservation::Literal { value } if looks_like_regex(value) => return None,
            UrlObservation::Literal { value } => UrlObservation::Literal {
                value: format!("{prefix}{value}"),
            },
            UrlObservation::Template { parts } => {
                let mut parts = parts.clone();
                match parts.first_mut() {
                    Some(TemplatePart::Text { value }) => value.insert_str(0, prefix),
                    _ => parts.insert(
                        0,
                        TemplatePart::Text {
                            value: prefix.to_owned(),
                        },
                    ),
                }
                UrlObservation::Template { parts }
            }
            UrlObservation::Dynamic { .. } => return None,
        };
        Some(RouteObservation {
            path,
            ..route.clone()
        })
    }

    /// The prefix at a position: the innermost mounted list around it, or
    /// else the file's own.
    fn prefix_at(
        &self,
        file: &str,
        span: Span,
        seen: &mut HashSet<(String, Span)>,
        depth: usize,
    ) -> PrefixResolution {
        if depth > MAX_MOUNT_DEPTH || !seen.insert((file.to_owned(), span)) {
            return cycle();
        }
        let around: Vec<&Region> = self
            .regions
            .get(file)
            .map(|rs| rs.iter().filter(|r| contains(r.span, span)).collect())
            .unwrap_or_default();
        // Every region around the position nests, so the innermost is the
        // one that starts last (and, from the same start, ends first).
        let Some(innermost) = around.iter().max_by_key(|r| {
            (
                (r.span.start_line, r.span.start_column),
                std::cmp::Reverse((r.span.end_line, r.span.end_column)),
            )
        }) else {
            return self.file_prefix(file, seen, depth + 1);
        };
        // The same list mounted twice serves its entries at two paths.
        let same = around.iter().filter(|r| r.span == innermost.span).count();
        if same > 1 {
            return PrefixResolution::Ambiguous { parents: same };
        }
        self.through(&innermost.mount, seen, depth + 1)
    }

    /// The prefix every entry of a URL-conf module is served under.
    fn file_prefix(
        &self,
        file: &str,
        seen: &mut HashSet<(String, Span)>,
        depth: usize,
    ) -> PrefixResolution {
        match self.mounts.get(file).map(Vec::as_slice).unwrap_or_default() {
            // Mounted nowhere this analysis can see: a root, served as written.
            [] => PrefixResolution::Known {
                prefix: String::new(),
                chain: Vec::new(),
            },
            [mount] => self.through(mount, seen, depth),
            many => PrefixResolution::Ambiguous {
                parents: many.len(),
            },
        }
    }

    /// The prefix a mount composes to: everything around it, then its own.
    fn through(
        &self,
        mount: &Mount,
        seen: &mut HashSet<(String, Span)>,
        depth: usize,
    ) -> PrefixResolution {
        let Some(own) = mount.prefix.as_deref().filter(|_| !mount.unresolved) else {
            return PrefixResolution::Unresolved {
                reason: format!(
                    "mounted at {}:{} under a pattern that is not a plain string",
                    mount.owner, mount.at.start_line
                ),
            };
        };
        match self.prefix_at(&mount.owner, mount.at, seen, depth) {
            PrefixResolution::Known { prefix, mut chain } => {
                chain.push(format!("{}:{}", mount.owner, mount.at.start_line));
                PrefixResolution::Known {
                    prefix: prefix + own,
                    chain,
                }
            }
            other => other,
        }
    }
}

/// Finds what an inclusion in `file` mounts.
fn resolve_target(
    modules: &ModuleIndex<'_>,
    file: &FileAnalysis,
    target: &IncludedUrlConf,
) -> Target {
    match target {
        IncludedUrlConf::Module { specifier } => modules
            .resolve_python_module(specifier, &file.path)
            .map_or(Target::Unknown, Target::File),
        IncludedUrlConf::Inline { span } => Target::Region(file.path.clone(), *span),
        // `include(router.urls)`, `include(views.patterns)`: an attribute of
        // an object this analysis does not model.
        IncludedUrlConf::Name { name } if name.contains('.') => Target::Unknown,
        IncludedUrlConf::Name { name } => resolve_name(modules, file, name),
        IncludedUrlConf::Other { .. } => Target::Unknown,
    }
}

/// A bare name in a mount: a list in this file, an imported list, or an
/// imported module.
fn resolve_name(modules: &ModuleIndex<'_>, file: &FileAnalysis, name: &str) -> Target {
    if let Some(span) = module_list(file, name) {
        return Target::Region(file.path.clone(), span);
    }
    match modules.resolve_python(file, name) {
        // `from .analytic import urlpatterns as analytic_urls`: the module's
        // own patterns — every entry in it, including any appended later.
        Resolution::Declared {
            file: at,
            name: declared,
            ..
        } if declared == "urlpatterns" => Target::File(at),
        // `from .api import extra_patterns`: only that list.
        Resolution::Declared {
            file: at,
            name: declared,
            ..
        } => modules
            .file(&at)
            .and_then(|f| module_list(f, &declared))
            .map_or(Target::Unknown, |span| Target::Region(at, span)),
        // `from project.license import urls`: the name may be a submodule,
        // which the symbol lookup does not see.
        Resolution::Unresolved { .. } | Resolution::NotBound => {
            imported_module(modules, file, name).map_or(Target::Unknown, Target::File)
        }
        _ => Target::Unknown,
    }
}

/// A module-level variable named `name` in `file`, by the span of its
/// assignment.
fn module_list(file: &FileAnalysis, name: &str) -> Option<Span> {
    file.symbols
        .iter()
        .find(|s| s.name == name && s.enclosing.is_none() && s.kind == SymbolKind::Variable)
        .map(|s| s.span)
}

/// The module bound to `name` by `from package import name`.
fn imported_module(modules: &ModuleIndex<'_>, file: &FileAnalysis, name: &str) -> Option<String> {
    file.imports.iter().find_map(|import| {
        let named = import
            .named
            .iter()
            .find(|n| n.local.as_deref().unwrap_or(n.imported.as_str()) == name)?;
        let specifier = if import.specifier.ends_with('.') {
            format!("{}{}", import.specifier, named.imported)
        } else {
            format!("{}.{}", import.specifier, named.imported)
        };
        modules.resolve_python_module(&specifier, &file.path)
    })
}

/// Whether `outer` encloses `inner` (or is it).
fn contains(outer: Span, inner: Span) -> bool {
    (outer.start_line, outer.start_column) <= (inner.start_line, inner.start_column)
        && (inner.end_line, inner.end_column) <= (outer.end_line, outer.end_column)
}

/// The pattern characters `normalize` treats as a regex: such a path is
/// never composed, because it is never interpreted.
fn looks_like_regex(text: &str) -> bool {
    text.contains(['^', '$', '\\', '[', ']', '+', '?', '(', ')'])
}

fn cycle() -> PrefixResolution {
    PrefixResolution::Unresolved {
        reason: "mounts cycle or nest deeper than this analysis follows".to_owned(),
    }
}
