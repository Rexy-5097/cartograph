//! The per-file Stage C cache.
//!
//! Caches one thing: a file's ORM access contribution, keyed by the dependency
//! identity the previous M10 slices proved complete. Nothing else is cached —
//! not `OrmAnalysis`, not `ModuleIndex`, not the graph — because nothing else
//! has a dependency set that has been traced and tested.
//!
//! # The rule
//!
//! An entry is reusable only when **every** term still matches:
//!
//! 1. the entry was recorded with complete dependency tracking
//! 2. the file's own content identity
//! 3. the merged model-set fingerprint
//! 4. every recorded read still exists **and** has the same content identity
//! 5. the Python path-set fingerprint, if that resolution consulted it
//! 6. the ordered alias fingerprint, if that resolution consulted it
//!
//! Any failure is a miss. There is no partial reuse: a result derived from a
//! dependency that moved is not partly correct, it is wrong.
//!
//! # File identity comes from the caller
//!
//! Identities are supplied, not computed here. The parse cache already hashes
//! every file's bytes, and a second notion of file identity is a second thing
//! to keep in step. A path whose identity the caller cannot supply is treated
//! as missing, which makes a deleted dependency a miss by construction rather
//! than by a special case.

use std::collections::HashMap;
use std::hash::BuildHasher;

use cartograph_parser::model::FileAnalysis;

use crate::dependencies::ResolutionContext;
use crate::imports::ModuleIndex;
use crate::orm::{OrmAccessSite, OrmModel, discover_accesses_tracked};

/// A Stage C recomputation failed.
///
/// There is no naturally occurring source of this today — every resolver path
/// answers rather than errors. It exists so the publication boundary has a
/// failure path that can be exercised, because "the old generation survives a
/// failure" is a claim about code that must be tested, not inferred from the
/// shape of an assignment.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AccessCacheError {
    /// Repository-relative path being recomputed when the failure occurred.
    pub file: String,
    /// Structural reason. Never source text.
    pub reason: String,
}

impl std::fmt::Display for AccessCacheError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "stage C recomputation failed for `{}`: {}",
            self.file, self.reason
        )
    }
}

impl std::error::Error for AccessCacheError {}

/// A hook consulted before each recomputation.
///
/// **Inert unless installed.** A production `AccessCache` has none, so the
/// production build has no failure behaviour whatsoever; only a test that
/// calls [`AccessCache::inject_failure`] can make one fail. Deliberately not a
/// feature flag or an environment read: QG-008 forbids the resolver's sources
/// from reading the environment, and a flag would put the failure path in a
/// different build than the one under test.
pub type RecomputeHook = Box<dyn Fn(&str, usize) -> Result<(), AccessCacheError> + Send + Sync>;

/// What the cache did, so tests can assert on work avoided rather than time.
///
/// Timing cannot distinguish reuse from a fast recomputation, so every cache
/// assertion in the suite is written against these counters.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct AccessCacheStats {
    /// Entries reused because every dependency term still matched.
    pub hits: usize,
    /// Files whose Stage C result had to be recomputed.
    pub misses: usize,
    /// Files recomputed this run. Equal to `misses`; named for the report.
    pub recomputed_files: usize,
    /// Files whose accesses were reused. Equal to `hits`; named for the report.
    pub reused_files: usize,
    /// Entries dropped because the file is no longer analysed.
    pub evicted: usize,
    /// Results that were computed but **not** stored, because their dependency
    /// tracking was incomplete. A non-zero value means the engine chose to
    /// recompute rather than risk a reuse it could not justify.
    pub refused_to_store: usize,
    /// Updates abandoned because a recomputation failed.
    pub failed_updates: usize,
    /// Whether the most recent update **published a generation**.
    ///
    /// The counters above describe an *attempt*. This says whether that
    /// attempt became the cache's state, so no combination of hit and miss
    /// counts can imply success when the update was abandoned.
    pub published: bool,
}

/// One cached Stage C result and the identity it is valid under.
#[derive(Debug, Clone)]
struct Entry {
    file_content: u64,
    model_set: u64,
    /// Recorded reads with the content identity each had when recorded. A path
    /// alone is not enough: the file may still exist with different contents.
    reads: Vec<(String, u64)>,
    path_set: Option<u64>,
    alias_set: Option<u64>,
    accesses: Vec<OrmAccessSite>,
    dependencies: ResolutionContext,
}

/// The whole-project state a Stage C result is valid against.
#[derive(Debug, Clone, Copy)]
pub struct GlobalFingerprints {
    /// Identity of the merged model set.
    pub model_set: u64,
    /// Identity of the repository's Python module path set.
    pub path_set: u64,
    /// Identity of the ordered build-alias list.
    pub alias_set: u64,
}

/// Per-file Stage C results, reusable while their dependencies hold.
#[derive(Default)]
pub struct AccessCache {
    entries: HashMap<String, Entry>,
    stats: AccessCacheStats,
    /// `None` in every production build. See [`RecomputeHook`].
    hook: Option<RecomputeHook>,
}

impl std::fmt::Debug for AccessCache {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AccessCache")
            .field("entries", &self.entries.len())
            .field("stats", &self.stats)
            .field("failure_hook_installed", &self.hook.is_some())
            .finish()
    }
}

impl AccessCache {
    /// Installs a failure hook. **Tests only.**
    ///
    /// Called with the file being recomputed and how many recomputations have
    /// already completed this run, so a test can fail before the first, after
    /// one, or after any number.
    pub fn inject_failure(&mut self, hook: RecomputeHook) {
        self.hook = Some(hook);
    }

    /// Removes any installed failure hook.
    pub fn clear_failure(&mut self) {
        self.hook = None;
    }

    /// An empty cache. Every lookup through it misses.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// What the most recent [`analyze`](Self::analyze) call did.
    #[must_use]
    pub fn stats(&self) -> AccessCacheStats {
        self.stats
    }

    /// Entries currently held.
    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether the cache holds nothing.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Whether `entry` may be reused for the current repository state.
    ///
    /// Every term, in the order they are cheapest to reject. A missing
    /// identity for a recorded read means the file is gone, which is a miss —
    /// the old hash must never be trusted in its place.
    fn reusable<S: BuildHasher>(
        entry: &Entry,
        file: &str,
        identities: &HashMap<String, u64, S>,
        global: GlobalFingerprints,
    ) -> bool {
        if !entry.dependencies.is_complete() {
            return false;
        }
        if identities.get(file) != Some(&entry.file_content) {
            return false;
        }
        if entry.model_set != global.model_set {
            return false;
        }
        if entry.path_set.is_some_and(|f| f != global.path_set) {
            return false;
        }
        if entry.alias_set.is_some_and(|f| f != global.alias_set) {
            return false;
        }
        entry
            .reads
            .iter()
            .all(|(path, recorded)| identities.get(path) == Some(recorded))
    }

    /// Stage C for every file, reusing what is still valid.
    ///
    /// `identities` maps repository-relative paths to content identities — the
    /// parse cache's hashes. A path absent from the map is treated as a file
    /// that no longer exists.
    ///
    /// Returns the accesses per file, in the order `files` gives them, so the
    /// flattened list a caller builds is identical to a clean run's.
    ///
    /// # Errors
    ///
    /// Returns [`AccessCacheError`] when a recomputation fails. Nothing is
    /// published in that case: the previous generation is left exactly as it
    /// was, and the partially built generation is dropped.
    pub fn analyze<S: BuildHasher, M: BuildHasher>(
        &mut self,
        files: &[&FileAnalysis],
        models: &HashMap<String, OrmModel, M>,
        index: &ModuleIndex<'_>,
        identities: &HashMap<String, u64, S>,
        global: GlobalFingerprints,
    ) -> Result<Vec<(Vec<OrmAccessSite>, ResolutionContext)>, AccessCacheError> {
        let mut stats = AccessCacheStats::default();
        let mut fresh: HashMap<String, Entry> = HashMap::with_capacity(files.len());
        let mut out = Vec::with_capacity(files.len());

        for file in files {
            let path = file.path.as_str();

            if let Some(entry) = self
                .entries
                .get(path)
                .filter(|e| Self::reusable(e, path, identities, global))
            {
                stats.hits += 1;
                stats.reused_files += 1;
                out.push((entry.accesses.clone(), entry.dependencies.clone()));
                fresh.insert(path.to_owned(), entry.clone());
                continue;
            }

            // The failure boundary. Consulted *before* the work, and an error
            // returns immediately — so `self.entries` is never assigned and
            // the previous generation stays exactly as it was. `fresh` is a
            // local that is simply dropped.
            if let Some(hook) = self.hook.as_ref() {
                if let Err(error) = hook(path, stats.recomputed_files) {
                    self.stats = AccessCacheStats {
                        failed_updates: 1,
                        published: false,
                        ..AccessCacheStats::default()
                    };
                    return Err(error);
                }
            }

            stats.misses += 1;
            stats.recomputed_files += 1;
            let mut dependencies = ResolutionContext::new();
            let accesses = discover_accesses_tracked(file, models, index, &mut dependencies);

            // The entry is built completely before it is published, so a
            // half-written entry can never be observed. An incomplete
            // dependency context is not stored at all: keeping it would invite
            // a later reuse that nothing justifies.
            if dependencies.is_complete() {
                if let Some(file_content) = identities.get(path).copied() {
                    let reads: Option<Vec<(String, u64)>> = dependencies
                        .files()
                        .map(|read| identities.get(read).map(|id| (read.to_owned(), *id)))
                        .collect();
                    if let Some(reads) = reads {
                        fresh.insert(
                            path.to_owned(),
                            Entry {
                                file_content,
                                model_set: global.model_set,
                                reads,
                                path_set: dependencies
                                    .consults_path_set()
                                    .then_some(global.path_set),
                                alias_set: dependencies
                                    .consults_alias_set()
                                    .then_some(global.alias_set),
                                accesses: accesses.clone(),
                                dependencies: dependencies.clone(),
                            },
                        );
                    } else {
                        // A read whose identity the caller cannot supply: the
                        // entry could never be validated, so it is not stored.
                        stats.refused_to_store += 1;
                    }
                } else {
                    stats.refused_to_store += 1;
                }
            } else {
                stats.refused_to_store += 1;
            }

            out.push((accesses, dependencies));
        }

        stats.evicted = self
            .entries
            .keys()
            .filter(|k| !fresh.contains_key(*k))
            .count();
        // Publication. One assignment, after every entry is built, so no
        // half-updated generation is ever observable.
        stats.published = true;
        self.entries = fresh;
        self.stats = stats;
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ctx(reads: &[&str], path_set: bool, complete: bool) -> ResolutionContext {
        let mut c = ResolutionContext::new();
        for r in reads {
            c.record_file(r);
        }
        if path_set {
            c.record_path_set();
        }
        if !complete {
            c.mark_incomplete();
        }
        c
    }

    fn entry(reads: &[(&str, u64)], complete: bool) -> Entry {
        Entry {
            file_content: 1,
            model_set: 2,
            reads: reads.iter().map(|(p, h)| ((*p).to_owned(), *h)).collect(),
            path_set: Some(3),
            alias_set: None,
            accesses: Vec::new(),
            dependencies: ctx(
                &reads.iter().map(|(p, _)| *p).collect::<Vec<_>>(),
                true,
                complete,
            ),
        }
    }

    fn ids(pairs: &[(&str, u64)]) -> HashMap<String, u64> {
        pairs.iter().map(|(p, h)| ((*p).to_owned(), *h)).collect()
    }

    const GLOBAL: GlobalFingerprints = GlobalFingerprints {
        model_set: 2,
        path_set: 3,
        alias_set: 4,
    };

    #[test]
    fn an_entry_with_every_term_matching_is_reusable() {
        let e = entry(&[("b.py", 9)], true);
        let identities = ids(&[("a.py", 1), ("b.py", 9)]);
        assert!(AccessCache::reusable(&e, "a.py", &identities, GLOBAL));
    }

    #[test]
    fn a_changed_own_file_is_not_reusable() {
        let e = entry(&[("b.py", 9)], true);
        let identities = ids(&[("a.py", 999), ("b.py", 9)]);
        assert!(!AccessCache::reusable(&e, "a.py", &identities, GLOBAL));
    }

    #[test]
    fn a_changed_dependency_is_not_reusable() {
        let e = entry(&[("b.py", 9)], true);
        let identities = ids(&[("a.py", 1), ("b.py", 999)]);
        assert!(!AccessCache::reusable(&e, "a.py", &identities, GLOBAL));
    }

    /// The deleted-dependency case: the old hash must never stand in for a
    /// file that is gone.
    #[test]
    fn a_deleted_dependency_is_not_reusable() {
        let e = entry(&[("b.py", 9)], true);
        let identities = ids(&[("a.py", 1)]);
        assert!(!AccessCache::reusable(&e, "a.py", &identities, GLOBAL));
    }

    #[test]
    fn a_changed_model_set_is_not_reusable() {
        let e = entry(&[("b.py", 9)], true);
        let identities = ids(&[("a.py", 1), ("b.py", 9)]);
        let moved = GlobalFingerprints {
            model_set: 22,
            ..GLOBAL
        };
        assert!(!AccessCache::reusable(&e, "a.py", &identities, moved));
    }

    #[test]
    fn a_changed_path_set_is_not_reusable_when_it_was_consulted() {
        let e = entry(&[("b.py", 9)], true);
        let identities = ids(&[("a.py", 1), ("b.py", 9)]);
        let moved = GlobalFingerprints {
            path_set: 33,
            ..GLOBAL
        };
        assert!(!AccessCache::reusable(&e, "a.py", &identities, moved));
    }

    /// ...and must not invalidate an entry that never consulted it.
    #[test]
    fn a_changed_path_set_is_ignored_when_it_was_not_consulted() {
        let mut e = entry(&[("b.py", 9)], true);
        e.path_set = None;
        e.dependencies = ctx(&["b.py"], false, true);
        let identities = ids(&[("a.py", 1), ("b.py", 9)]);
        let moved = GlobalFingerprints {
            path_set: 33,
            ..GLOBAL
        };
        assert!(AccessCache::reusable(&e, "a.py", &identities, moved));
    }

    /// A Python entry never records an alias dependency, so moving the alias
    /// list must not invalidate it.
    #[test]
    fn a_changed_alias_set_is_ignored_when_it_was_not_consulted() {
        let e = entry(&[("b.py", 9)], true);
        assert!(e.alias_set.is_none());
        let identities = ids(&[("a.py", 1), ("b.py", 9)]);
        let moved = GlobalFingerprints {
            alias_set: 44,
            ..GLOBAL
        };
        assert!(AccessCache::reusable(&e, "a.py", &identities, moved));
    }

    #[test]
    fn a_changed_alias_set_is_not_reusable_when_it_was_consulted() {
        let mut e = entry(&[("b.ts", 9)], true);
        e.alias_set = Some(4);
        let identities = ids(&[("a.ts", 1), ("b.ts", 9)]);
        let moved = GlobalFingerprints {
            alias_set: 44,
            ..GLOBAL
        };
        assert!(!AccessCache::reusable(&e, "a.ts", &identities, moved));
    }

    /// The rule that keeps the whole design honest.
    #[test]
    fn an_incomplete_entry_is_never_reusable() {
        let e = entry(&[("b.py", 9)], false);
        let identities = ids(&[("a.py", 1), ("b.py", 9)]);
        assert!(
            !AccessCache::reusable(&e, "a.py", &identities, GLOBAL),
            "incomplete tracking must disqualify reuse even when every other term matches"
        );
    }

    #[test]
    fn a_new_cache_is_empty() {
        let cache = AccessCache::new();
        assert!(cache.is_empty());
        assert_eq!(cache.stats(), AccessCacheStats::default());
    }
}
