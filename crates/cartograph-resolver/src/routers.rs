//! Composing the path a route is actually served on.
//!
//! ```text
//! @pools_router.post("")                     the decorator states ""
//! pools_router = AirflowRouter(prefix="/pools")
//! public_router = AirflowRouter(prefix="/api/v2")
//! public_router.include_router(authenticated_router)
//! authenticated_router.include_router(pools_router)
//!                                            ─► POST /api/v2/pools
//! ```
//!
//! # Why a route's own path is not its path
//!
//! M07 measured 1,458 `HttpCall` edges of which twelve had a TypeScript
//! client. One reason was this: a frontend asks for `/api/v2/pools` and the
//! decorator says `""`. Comparing those two is comparing a served path against
//! a fragment.
//!
//! # What refuses
//!
//! A prefix that is not a string literal, a router included in more than one
//! parent, an inclusion cycle, and a receiver that names no router this
//! project declares. Each produces [`PrefixResolution::Unresolved`] or
//! [`PrefixResolution::Ambiguous`], and a route whose prefix did not resolve
//! keeps the path it states rather than being given a composed one — an
//! unknown prefix must not silently become an empty one.

use std::collections::{HashMap, HashSet};

use cartograph_parser::model::{FileAnalysis, RouteObservation, UrlObservation};

use crate::imports::{ModuleIndex, Resolution};

/// A router's composed prefix, or why there is not one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PrefixResolution {
    /// The full prefix, composed from every enclosing router.
    ///
    /// May be empty: a router with no prefix, included nowhere, composes to
    /// the empty string, which is a fact rather than a failure.
    Known {
        /// The composed prefix, e.g. `/api/v2/pools`.
        prefix: String,
        /// The routers it was composed from, outermost first, for evidence.
        chain: Vec<String>,
    },
    /// A prefix exists somewhere in the chain and could not be read.
    Unresolved {
        /// Why, for the record.
        reason: String,
    },
    /// The router is mounted in more than one place.
    Ambiguous {
        /// How many distinct parents claim it.
        parents: usize,
    },
}

impl PrefixResolution {
    /// The prefix, when it is known.
    #[must_use]
    pub fn known(&self) -> Option<&str> {
        match self {
            Self::Known { prefix, .. } => Some(prefix),
            _ => None,
        }
    }

    /// A phrase for an edge's evidence. Never contains source text.
    #[must_use]
    pub fn explain(&self) -> String {
        match self {
            Self::Known { prefix, chain } if chain.is_empty() => {
                format!("router prefix {prefix:?}")
            }
            Self::Known { prefix, chain } => {
                format!("composed prefix {prefix:?} from {}", chain.join(" → "))
            }
            Self::Unresolved { reason } => format!("prefix not composed: {reason}"),
            Self::Ambiguous { parents } => {
                format!("router is mounted in {parents} places; no single served path")
            }
        }
    }
}

/// A router, identified by the file that declares it and its name.
type RouterId = (String, String);

/// A router's own prefix, and whether one was stated but not readable.
type OwnPrefix = (Option<String>, bool);

/// Where a router is mounted: the parent, the prefix stated at the inclusion
/// site, and whether that prefix was present but not a literal.
type Mount = (RouterId, Option<String>, bool);

/// How deep an inclusion chain may go before it is treated as a cycle.
const MAX_INCLUSION_DEPTH: usize = 12;

/// Every router the project declares, and the prefix each composes to.
pub struct RouterIndex {
    prefixes: HashMap<RouterId, PrefixResolution>,
}

impl RouterIndex {
    /// Builds the index from an analysed project.
    #[must_use]
    pub fn build(files: &[FileAnalysis], modules: &ModuleIndex<'_>) -> Self {
        // Declared routers, and their own prefix.
        let mut declared: HashMap<RouterId, OwnPrefix> = HashMap::new();
        for file in files {
            for router in &file.routers {
                declared.insert(
                    (file.path.clone(), router.name.clone()),
                    (router.prefix.clone(), router.prefix_unresolved),
                );
            }
        }

        // Who includes whom. A name at an inclusion site is resolved through
        // that file's imports, so `authenticated_router.include_router(
        // assets_router)` finds the assets_router declared in another module.
        let mut parents: HashMap<RouterId, Vec<Mount>> = HashMap::new();
        for file in files {
            for inclusion in &file.router_inclusions {
                let (Some(child), Some(parent)) = (
                    resolve_router(modules, file, &inclusion.child, &declared),
                    resolve_router(modules, file, &inclusion.parent, &declared),
                ) else {
                    continue; // a router this project does not declare
                };
                parents.entry(child).or_default().push((
                    parent,
                    inclusion.prefix.clone(),
                    inclusion.prefix_unresolved,
                ));
            }
        }

        let mut prefixes = HashMap::new();
        for id in declared.keys() {
            let mut seen = HashSet::new();
            let resolution = compose(id, &declared, &parents, &mut seen, 0);
            prefixes.insert(id.clone(), resolution);
        }
        Self { prefixes }
    }

    /// The composed prefix for a route's receiver, declared in `file`.
    #[must_use]
    pub fn prefix_for(&self, file: &str, receiver: &str) -> Option<&PrefixResolution> {
        self.prefixes.get(&(file.to_owned(), receiver.to_owned()))
    }

    /// How many routers were indexed.
    #[must_use]
    pub fn len(&self) -> usize {
        self.prefixes.len()
    }

    /// Whether the project declares any router.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.prefixes.is_empty()
    }
}

/// Resolves a router name used in `file` to the router it refers to.
fn resolve_router(
    modules: &ModuleIndex<'_>,
    file: &FileAnalysis,
    name: &str,
    declared: &HashMap<RouterId, OwnPrefix>,
) -> Option<RouterId> {
    let here = (file.path.clone(), name.to_owned());
    if declared.contains_key(&here) {
        return Some(here);
    }
    match modules.resolve_python(file, name) {
        Resolution::Declared { file: at, .. } => {
            let there = (at, name.to_owned());
            declared.contains_key(&there).then_some(there)
        }
        _ => None,
    }
}

/// Walks a router's inclusion chain outward, accumulating prefixes.
fn compose(
    id: &RouterId,
    declared: &HashMap<RouterId, OwnPrefix>,
    parents: &HashMap<RouterId, Vec<Mount>>,
    seen: &mut HashSet<RouterId>,
    depth: usize,
) -> PrefixResolution {
    if depth > MAX_INCLUSION_DEPTH || !seen.insert(id.clone()) {
        return PrefixResolution::Unresolved {
            reason: "inclusion chain cycles or is deeper than this analysis follows".to_owned(),
        };
    }

    let Some((own_prefix, own_unresolved)) = declared.get(id) else {
        return PrefixResolution::Unresolved {
            reason: "not a router this project declares".to_owned(),
        };
    };
    if *own_unresolved {
        return PrefixResolution::Unresolved {
            reason: format!("`{}` has a prefix that is not a string literal", id.1),
        };
    }

    let own = own_prefix.clone().unwrap_or_default();
    let mounts = parents.get(id).map(Vec::as_slice).unwrap_or_default();

    match mounts.len() {
        // A root router: its own prefix is the whole prefix.
        0 => PrefixResolution::Known {
            prefix: own,
            chain: vec![id.1.clone()],
        },
        1 => {
            let (parent, at_inclusion, inclusion_unresolved) = &mounts[0];
            if *inclusion_unresolved {
                return PrefixResolution::Unresolved {
                    reason: format!(
                        "`{}` is included with a prefix that is not a string literal",
                        id.1
                    ),
                };
            }
            match compose(parent, declared, parents, seen, depth + 1) {
                PrefixResolution::Known {
                    prefix: outer,
                    mut chain,
                } => {
                    chain.push(id.1.clone());
                    let mut composed = outer;
                    composed.push_str(at_inclusion.as_deref().unwrap_or(""));
                    composed.push_str(&own);
                    PrefixResolution::Known {
                        prefix: composed,
                        chain,
                    }
                }
                other => other,
            }
        }
        // Mounted twice under different parents: the same decorator serves two
        // paths, and choosing one would invent an endpoint.
        n => PrefixResolution::Ambiguous { parents: n },
    }
}

/// Restates a route with its composed prefix applied.
///
/// Returns `None` when there is nothing to apply — no receiver, no router, an
/// unresolved or ambiguous prefix, an empty prefix, or a path this analysis
/// cannot rebuild. In every one of those the caller keeps the original
/// observation, so composition can only ever make a path more complete.
#[must_use]
pub fn with_composed_prefix(
    route: &RouteObservation,
    resolution: &PrefixResolution,
) -> Option<RouteObservation> {
    let prefix = resolution.known()?;
    if prefix.is_empty() {
        return None;
    }
    let composed = match &route.path {
        UrlObservation::Literal { value } => UrlObservation::Literal {
            value: join_paths(prefix, value),
        },
        // A templated path keeps its parts; only the leading text moves.
        UrlObservation::Template { parts } => {
            let mut parts = parts.clone();
            match parts.first_mut() {
                Some(cartograph_parser::model::TemplatePart::Text { value }) => {
                    *value = join_paths(prefix, value);
                }
                _ => parts.insert(
                    0,
                    cartograph_parser::model::TemplatePart::Text {
                        value: prefix.to_owned(),
                    },
                ),
            }
            UrlObservation::Template { parts }
        }
        // A dynamic path is unknown; prefixing an unknown yields an unknown.
        UrlObservation::Dynamic { .. } => return None,
    };
    Some(RouteObservation {
        path: composed,
        ..route.clone()
    })
}

/// Joins a prefix and a route path without inventing or losing a separator.
fn join_paths(prefix: &str, path: &str) -> String {
    let left = prefix.strip_suffix('/').unwrap_or(prefix);
    if path.is_empty() || path == "/" {
        // `@router.post("")` under prefix `/pools` serves `/pools`, and
        // `@router.get("/")` serves `/pools/`. The trailing slash is a
        // distinction M03 preserves, so it is preserved here too.
        return if path == "/" {
            format!("{left}/")
        } else {
            left.to_owned()
        };
    }
    if path.starts_with('/') {
        format!("{left}{path}")
    } else {
        format!("{left}/{path}")
    }
}
