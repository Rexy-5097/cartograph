//! What a resolution actually read.
//!
//! # Why this exists
//!
//! Resolving a name is not a function of the asking file alone. `from .models
//! import Order` sends the resolver into `models.py`, and if that module
//! re-exports the name it goes further still, up to `MAX_REEXPORT_HOPS`. Any
//! file along that chain can change the answer.
//!
//! Worse, one of the inputs is not a file's *contents* at all.
//! `resolve_python_module` matches a dotted specifier against the repository's
//! file paths as a suffix and answers only when exactly one candidate matches.
//! Adding a file whose path collides — even one containing nothing but
//! `VERSION = 1` — turns a resolved import into an unresolved one and
//! withdraws an edge. That is a dependency on repository *membership*, and no
//! amount of content hashing detects it.
//!
//! So a cache key cannot be predicted from the asking file's hash. It has to
//! be *recorded* while the resolution runs. This module records it. Nothing
//! here caches anything; it only reports what a resolution depended on, so a
//! later slice can decide when that resolution is still valid.
//!
//! # What is deliberately not here
//!
//! No source text, no absolute paths, no timestamps, no environment values.
//! A dependency is a repository-relative path — the same identity the fact
//! model already uses — and paths are all that is retained.

use std::collections::BTreeSet;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

/// Hashes a canonical, already-ordered list.
///
/// The count goes first so no list can hash as a prefix of a longer one.
/// `DefaultHasher` (`SipHash`) — an internal deterministic fingerprint for cache
/// invalidation, **not** a cryptographic hash. No collision resistance is
/// claimed, and none is needed: the inputs are the user's own file paths and a
/// collision costs a stale reuse decision, which the completeness rule and the
/// differential suite exist to catch.
fn fingerprint_of(entries: &[String]) -> u64 {
    let mut hasher = DefaultHasher::new();
    entries.len().hash(&mut hasher);
    for entry in entries {
        entry.hash(&mut hasher);
    }
    hasher.finish()
}

/// Identity of the repository paths Python module resolution can match.
///
/// `resolve_python_module` builds candidates of the form `<base>.py` and
/// `<base>/__init__.py`, then accepts a repository path when it equals a
/// candidate or ends with `/<candidate>`. So only paths ending in `.py` can
/// ever match: `.pyi`, `.ts` and `.tsx` are in the index but are unreachable
/// as module targets, and including them would invalidate Python results for
/// changes that cannot affect them.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct PathSetFingerprint(u64);

impl PathSetFingerprint {
    /// Fingerprints an already-canonical path list.
    #[must_use]
    pub fn of(canonical: &[String]) -> Self {
        Self(fingerprint_of(canonical))
    }

    /// The raw value, for logging and tests.
    #[must_use]
    pub const fn get(self) -> u64 {
        self.0
    }
}

/// Identity of the **ordered** build-alias list.
///
/// `apply_alias` walks the list and returns the first entry whose directory
/// scopes the importing file and whose alias prefixes the specifier, using all
/// three of `(alias, target, dir)`. The list is sorted globally by directory
/// depth, then alias length, then alias — so order is semantic and a
/// membership-only identity would be unsound.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct AliasSetFingerprint(u64);

impl AliasSetFingerprint {
    /// Fingerprints an already-ordered alias list.
    #[must_use]
    pub fn of(ordered: &[String]) -> Self {
        Self(fingerprint_of(ordered))
    }

    /// The raw value, for logging and tests.
    #[must_use]
    pub const fn get(self) -> u64 {
        self.0
    }
}

/// The complete dependency identity of one file's Stage C result.
///
/// **A specification and representation only — nothing looks anything up.** It
/// names every term a future cache would have to agree on before reusing a
/// result, so the terms can be reviewed before any reuse exists.
///
/// `reads` carries paths rather than content hashes: the resolver does not own
/// file contents, and the parse cache already holds their identities. A lookup
/// would pair each path with the identity the parse cache reports.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PerFileDependencyIdentity {
    /// The file whose Stage C result this identifies.
    pub file: String,
    /// Identity of that file's own contents, from the parse cache.
    pub file_content: u64,
    /// Identity of the merged model set the accesses resolved against.
    pub model_set: u64,
    /// Every file whose contents were read, deterministically ordered.
    pub reads: Vec<String>,
    /// Present only when resolution consulted repository membership.
    pub path_set: Option<PathSetFingerprint>,
    /// Present only when resolution consulted the build aliases.
    pub alias_set: Option<AliasSetFingerprint>,
    /// Whether every dependency was tracked.
    pub complete: bool,
}

impl PerFileDependencyIdentity {
    /// Builds the identity from a recorded context.
    ///
    /// The optional fingerprints are attached **only** when the context says
    /// that kind of state was consulted, so a Python result never carries an
    /// alias identity and a relative TypeScript import never carries one
    /// either.
    #[must_use]
    pub fn new(
        file: &str,
        file_content: u64,
        model_set: u64,
        deps: &ResolutionContext,
        path_set: PathSetFingerprint,
        alias_set: AliasSetFingerprint,
    ) -> Self {
        Self {
            file: file.to_owned(),
            file_content,
            model_set,
            reads: deps.files().map(ToOwned::to_owned).collect(),
            path_set: deps.consults_path_set().then_some(path_set),
            alias_set: deps.consults_alias_set().then_some(alias_set),
            complete: deps.is_complete(),
        }
    }

    /// Whether a future cache may reuse the result this identifies.
    ///
    /// Completeness alone decides eligibility; matching the terms decides
    /// validity. An incomplete identity is never reusable, however well its
    /// other terms match.
    #[must_use]
    pub const fn is_reusable(&self) -> bool {
        self.complete
    }
}

/// The repository state one resolution consulted.
///
/// Ordering is deterministic (`BTreeSet`), so two runs over the same
/// repository produce byte-identical dependency sets and a future cache key
/// cannot depend on hash iteration order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolutionContext {
    files: BTreeSet<String>,
    path_set_consulted: bool,
    alias_set_consulted: bool,
    complete: bool,
}

impl Default for ResolutionContext {
    /// A fresh context has read nothing and is *complete*: it has not yet had
    /// the opportunity to lose track of anything.
    fn default() -> Self {
        Self {
            files: BTreeSet::new(),
            path_set_consulted: false,
            alias_set_consulted: false,
            complete: true,
        }
    }
}

impl ResolutionContext {
    /// A context that has recorded nothing.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Records that a file's **contents** were consulted.
    ///
    /// Called for every file whose symbols or imports were read, including
    /// every hop of a re-export chain — not only the file that asked.
    pub fn record_file(&mut self, path: &str) {
        self.files.insert(path.to_owned());
    }

    /// Records that the repository's **set of paths** was consulted.
    ///
    /// Distinct from reading a file: the answer depended on which files exist,
    /// so creating or deleting a file elsewhere can change it even though no
    /// recorded file's contents changed.
    pub fn record_path_set(&mut self) {
        self.path_set_consulted = true;
    }

    /// Records that the answer depended on the project's **build aliases**.
    ///
    /// A third kind of dependency, distinct from both of the others. The alias
    /// list is assembled from every file's `module_aliases` and sorted
    /// globally by directory depth then alias length, so adding a build
    /// configuration anywhere can change which alias wins for an unrelated
    /// file. Membership is not enough; the ordering is semantic.
    ///
    /// Only TypeScript resolution consults it — `resolve_python_module` never
    /// calls `apply_alias` — so a Python-only result must not carry this flag.
    pub fn record_alias_set(&mut self) {
        self.alias_set_consulted = true;
    }

    /// Records that the dependency set is **not** known to be complete.
    ///
    /// The one-way switch that keeps a later cache honest: a result whose
    /// dependencies could not be fully tracked must never be reused on the
    /// strength of the dependencies that *were* seen. Nothing clears it.
    pub fn mark_incomplete(&mut self) {
        self.complete = false;
    }

    /// The files whose contents were read, in deterministic order.
    pub fn files(&self) -> impl Iterator<Item = &str> {
        self.files.iter().map(String::as_str)
    }

    /// How many files were read.
    #[must_use]
    pub fn file_count(&self) -> usize {
        self.files.len()
    }

    /// Whether a given file was read.
    #[must_use]
    pub fn reads(&self, path: &str) -> bool {
        self.files.contains(path)
    }

    /// Whether the answer depended on which files exist in the repository.
    #[must_use]
    pub fn consults_path_set(&self) -> bool {
        self.path_set_consulted
    }

    /// Whether the answer depended on the project's build aliases.
    #[must_use]
    pub fn consults_alias_set(&self) -> bool {
        self.alias_set_consulted
    }

    /// Whether every dependency was tracked.
    ///
    /// A future cache may only reuse a result whose context reports `true`.
    #[must_use]
    pub fn is_complete(&self) -> bool {
        self.complete
    }

    /// Absorbs another context's dependencies.
    ///
    /// Incompleteness is infectious: merging an incomplete context makes the
    /// result incomplete, because the combined answer now rests on something
    /// that was not tracked.
    pub fn absorb(&mut self, other: &Self) {
        self.files.extend(other.files.iter().cloned());
        self.path_set_consulted |= other.path_set_consulted;
        self.alias_set_consulted |= other.alias_set_consulted;
        self.complete &= other.complete;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_new_context_is_empty_and_complete() {
        let ctx = ResolutionContext::new();
        assert_eq!(ctx.file_count(), 0);
        assert!(!ctx.consults_path_set());
        assert!(ctx.is_complete());
    }

    #[test]
    fn reads_are_deduplicated_and_ordered() {
        let mut ctx = ResolutionContext::new();
        ctx.record_file("b/second.py");
        ctx.record_file("a/first.py");
        ctx.record_file("b/second.py");
        assert_eq!(ctx.file_count(), 2, "the repeat must not be counted twice");
        assert_eq!(
            ctx.files().collect::<Vec<_>>(),
            vec!["a/first.py", "b/second.py"],
            "order must be deterministic, not insertion order"
        );
    }

    #[test]
    fn the_path_set_is_a_separate_kind_of_dependency() {
        let mut ctx = ResolutionContext::new();
        ctx.record_file("a.py");
        assert!(
            !ctx.consults_path_set(),
            "reading a file is not consulting the path set"
        );
        ctx.record_path_set();
        assert!(ctx.consults_path_set());
    }

    #[test]
    fn incompleteness_is_one_way() {
        let mut ctx = ResolutionContext::new();
        ctx.mark_incomplete();
        ctx.record_file("a.py");
        ctx.record_path_set();
        assert!(
            !ctx.is_complete(),
            "recording more must never restore completeness"
        );
    }

    #[test]
    fn absorbing_unions_the_reads_and_propagates_incompleteness() {
        let mut left = ResolutionContext::new();
        left.record_file("a.py");
        let mut right = ResolutionContext::new();
        right.record_file("b.py");
        right.record_path_set();
        right.mark_incomplete();

        left.absorb(&right);
        assert_eq!(left.files().collect::<Vec<_>>(), vec!["a.py", "b.py"]);
        assert!(left.consults_path_set(), "path-set use must propagate");
        assert!(!left.is_complete(), "incompleteness must propagate");
    }

    #[test]
    fn absorbing_a_complete_context_does_not_clear_incompleteness() {
        let mut left = ResolutionContext::new();
        left.mark_incomplete();
        left.absorb(&ResolutionContext::new());
        assert!(!left.is_complete());
    }
}
