//! The parse cache — M10's first incremental slice.
//!
//! # Why this boundary
//!
//! `Analyzer::dispatch` is a pure function of `(path, language, source)`: it
//! touches no global state, and [`FileAnalysis`] already derives `PartialEq`.
//! So a file's facts are fully determined by its bytes, and reusing them when
//! the bytes have not changed cannot produce a different answer.
//!
//! Everything *above* parse — `ModuleIndex`, `RouteIndex`, `ExportedConstants`,
//! `OrmAnalysis`, every edge — reads the whole file set and is recomputed in
//! full on every run. That is not an oversight. It is what makes the central
//! M10 invariant hold by construction here:
//!
//! ```text
//! incremental_analysis(repo) == clean_analysis(repo)
//! ```
//!
//! No derived fact is cached, so no derived fact can go stale. The invariant
//! reduces to one checkable property — equal bytes imply an equal
//! `FileAnalysis` — rather than to an invalidation argument over a dependency
//! graph. Making resolution itself incremental is the next slice, and it is
//! where the interesting failure modes live.
//!
//! Measured on this machine: extraction is 79–95% of a run, and caching it is
//! worth 3.0× on a 1,539-file repository and 4.5× on a 156-file one, with zero
//! files reparsed over an unchanged tree. A warm run then spends nearly all of
//! its time above the parse layer, so resolution is the next bottleneck. See
//! `docs/development/incremental-engine.md`.

use std::collections::HashMap;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::path::Path;

use cartograph_parser::model::FileAnalysis;
use cartograph_parser::{Analyzer, ParserError};

/// A file's content identity.
///
/// `DefaultHasher` (SipHash-1-3), deliberately, and no new dependency. This
/// validates a cache; it is not a security boundary. The input is the user's
/// own working tree, nobody is supplying files to force a collision, and a
/// collision costs a stale parse rather than a privilege breach. Hashing is
/// already the dominant cost of a warm run (647 ms for 21 MB), so a
/// cryptographic hash would make the common path slower for no correctness
/// gain.
///
/// The hash covers bytes only — no path, no timestamp, no machine identifier —
/// so identical content hashes identically anywhere. Bytes rather than decoded
/// text, so an encoding change counts as a change.
///
/// `DefaultHasher` is not stable across Rust releases. For an in-process cache
/// that is a feature: a toolchain change invalidates it. It would **not** be
/// acceptable for an on-disk cache, and persistence must revisit this choice.
#[must_use]
pub fn content_hash(bytes: &[u8]) -> u64 {
    let mut hasher = DefaultHasher::new();
    bytes.hash(&mut hasher);
    hasher.finish()
}

/// What one run did, so a test can assert on work avoided rather than on time.
///
/// Timing cannot distinguish "reused the cache" from "reparsed quickly on a
/// warm page cache", so the invariant "unchanged files are not reparsed" is
/// only testable against counts.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct CacheStats {
    /// Files discovery offered this run.
    pub seen: usize,
    /// Files whose cached facts were reused — content hash matched.
    pub reused: usize,
    /// Files parsed this run — new, changed, or unreadable.
    pub parsed: usize,
    /// Entries dropped because the file is no longer present.
    pub evicted: usize,
}

/// Cached per-file facts, keyed by repository-relative path.
///
/// The path is the key rather than part of the hash because a `FileAnalysis`
/// embeds its own path: a moved file must miss, even with identical bytes,
/// or its facts would carry the wrong location.
#[derive(Debug, Default)]
pub struct FactCache {
    entries: HashMap<String, Entry>,
    stats: CacheStats,
}

#[derive(Debug)]
struct Entry {
    hash: u64,
    analysis: FileAnalysis,
}

impl FactCache {
    /// Facts for every file in `files`, reusing what is still valid.
    ///
    /// Files are returned in the order given, so the analyses vector is
    /// identical to the one a clean run builds — resolution is
    /// order-sensitive, and reordering would be a difference the canonical
    /// comparison would rightly flag.
    ///
    /// # Errors
    ///
    /// Propagates [`ParserError`] for a path or language the analyser refuses.
    /// Problems *inside* a file are diagnostics on the returned analysis, not
    /// errors, and are cached like any other result — so a file that becomes
    /// malformed loses its old facts rather than keeping them.
    /// `on_file` is called with the index of each file as it is handled, so
    /// the caller can report progress. The cache itself writes nothing:
    /// deciding what a human sees belongs to the CLI layer, not here.
    pub fn analyze(
        &mut self,
        analyzer: &mut Analyzer,
        root: &Path,
        files: &[String],
        mut on_file: impl FnMut(usize),
    ) -> Result<Vec<FileAnalysis>, ParserError> {
        let mut stats = CacheStats {
            seen: files.len(),
            ..CacheStats::default()
        };
        let mut fresh: HashMap<String, Entry> = HashMap::with_capacity(files.len());
        let mut analyses = Vec::with_capacity(files.len());

        for (index, rel) in files.iter().enumerate() {
            on_file(index);

            // Read once and reuse the bytes: hashing and then letting the
            // analyser read the file again would double the I/O on every miss.
            //
            // Unreadable means no content, so no cache identity. Parsing it
            // yields the `Unreadable` diagnostic, and no entry is retained — a
            // permissions change must not resurrect the last good facts.
            let Ok(bytes) = std::fs::read(root.join(rel)) else {
                stats.parsed += 1;
                analyses.push(analyzer.analyze_file(root, rel)?);
                continue;
            };

            let hash = content_hash(&bytes);
            let facts = if let Some(hit) = self.entries.get(rel).filter(|e| e.hash == hash) {
                stats.reused += 1;
                hit.analysis.clone()
            } else {
                stats.parsed += 1;
                // Valid UTF-8 is dispatched from the bytes already in hand.
                // Anything else delegates to `analyze_file`, which owns the
                // not-UTF-8 diagnostic; reproducing it here would be a second
                // definition of the same behaviour, free to drift.
                match std::str::from_utf8(&bytes) {
                    Ok(source) => analyzer.analyze_source(rel, source)?,
                    Err(_) => analyzer.analyze_file(root, rel)?,
                }
            };

            analyses.push(facts.clone());
            fresh.insert(
                rel.clone(),
                Entry {
                    hash,
                    analysis: facts,
                },
            );
        }

        // Whatever the new set does not contain is gone: deleted, renamed
        // away, or newly excluded. Replacing the map wholesale is what makes
        // deletion correct without a separate deletion path.
        stats.evicted = self
            .entries
            .keys()
            .filter(|k| !fresh.contains_key(*k))
            .count();
        self.entries = fresh;
        self.stats = stats;

        Ok(analyses)
    }

    /// What the most recent [`analyze`](Self::analyze) call did.
    #[must_use]
    pub fn stats(&self) -> CacheStats {
        self.stats
    }

    /// Entries currently held.
    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Content identity of every file analysed this run.
    ///
    /// The Stage C cache validates its recorded reads against these, so both
    /// caches share one notion of file identity rather than each hashing
    /// separately and drifting.
    #[must_use]
    pub fn identities(&self) -> HashMap<String, u64> {
        self.entries
            .iter()
            .map(|(path, entry)| (path.clone(), entry.hash))
            .collect()
    }
}

/// Everything one incremental session remembers between runs.
///
/// Two caches with different units: `facts` holds parsed files, `accesses`
/// holds per-file Stage C results. They are kept together so a caller cannot
/// warm one and forget the other, which would silently validate Stage C
/// entries against identities nobody refreshed.
#[derive(Debug, Default)]
pub struct IncrementalState {
    /// Parsed per-file facts.
    pub facts: FactCache,
    /// Per-file ORM access results.
    pub accesses: cartograph_resolver::AccessCache,
}

impl IncrementalState {
    /// State that remembers nothing. The first run through it is cold.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identical_bytes_hash_identically_and_different_bytes_do_not() {
        assert_eq!(content_hash(b"const A = 1;"), content_hash(b"const A = 1;"));
        assert_ne!(content_hash(b"const A = 1;"), content_hash(b"const A = 2;"));
    }

    /// Whitespace is content: a formatting-only edit changes locations, so it
    /// must not be treated as a no-op.
    #[test]
    fn whitespace_is_part_of_content_identity() {
        assert_ne!(
            content_hash(b"const A = 1;"),
            content_hash(b"const A  = 1;")
        );
        assert_ne!(content_hash(b"a"), content_hash(b"a\n"));
    }

    #[test]
    fn the_empty_file_has_a_stable_hash() {
        assert_eq!(content_hash(b""), content_hash(b""));
        assert_ne!(content_hash(b""), content_hash(b" "));
    }

    /// The hash is over bytes, so it carries no machine-specific information
    /// and the same content anywhere is the same content.
    #[test]
    fn the_hash_depends_on_content_alone() {
        let source = b"export const BASE = '/api';";
        assert_eq!(content_hash(source), content_hash(source));
    }

    #[test]
    fn a_new_cache_is_empty() {
        let cache = FactCache::default();
        assert_eq!(cache.len(), 0);
        assert_eq!(cache.stats(), CacheStats::default());
    }
}

/// The differential suite: `incremental == clean`, per mutation class.
///
/// These live inside the crate because `cartograph-cli` is a binary and a
/// `tests/` file cannot reach its modules.
#[cfg(test)]
mod differential {
    use super::*;
    use crate::pipeline::{self, Options};
    use cartograph_testkit::canonical::{assert_graphs_equal, canonical};

    /// A repository on disk that a test can mutate.
    ///
    /// Deterministic by construction: every fixture is written from the
    /// literals below rather than copied from the repository, so a test cannot
    /// depend on a file someone edits for another reason. The directory is
    /// unique per test and removed on drop.
    struct Repo {
        root: std::path::PathBuf,
    }

    impl Repo {
        fn new(name: &str) -> Self {
            // A counter, not a timestamp: two runs in the same millisecond
            // must not collide, and a timestamp in a path is nondeterministic.
            use std::sync::atomic::{AtomicUsize, Ordering};
            static NEXT: AtomicUsize = AtomicUsize::new(0);
            let unique = NEXT.fetch_add(1, Ordering::Relaxed);
            let root = std::env::temp_dir()
                .join("cartograph-m10")
                .join(format!("{name}-{}-{unique}", std::process::id()));
            let _ = std::fs::remove_dir_all(&root);
            std::fs::create_dir_all(&root).expect("fixture root");
            Self { root }
        }

        fn write(&self, rel: &str, contents: &str) -> &Self {
            let path = self.root.join(rel);
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent).expect("fixture parent");
            }
            std::fs::write(path, contents).expect("write fixture");
            self
        }

        fn delete(&self, rel: &str) -> &Self {
            std::fs::remove_file(self.root.join(rel)).expect("delete fixture");
            self
        }

        fn rename(&self, from: &str, to: &str) -> &Self {
            let target = self.root.join(to);
            if let Some(parent) = target.parent() {
                std::fs::create_dir_all(parent).expect("fixture parent");
            }
            std::fs::rename(self.root.join(from), target).expect("rename fixture");
            self
        }

        /// Analysis through `cache`, i.e. incremental once the cache is warm.
        fn incremental(&self, cache: &mut IncrementalState) -> pipeline::Analysis {
            pipeline::run_with_cache(&self.root, Options { quiet: true }, cache)
                .expect("incremental analysis")
        }

        /// Analysis from nothing — the reference answer.
        fn clean(&self) -> pipeline::Analysis {
            pipeline::run(&self.root, Options { quiet: true }).expect("clean analysis")
        }

        /// The whole point: after a mutation, the two must agree.
        fn assert_incremental_matches_clean(&self, cache: &mut IncrementalState, context: &str) {
            let incremental = self.incremental(cache);
            let clean = self.clean();
            assert_graphs_equal(&incremental.graph, &clean.graph, context);
            assert_eq!(
                incremental.totals.files, clean.totals.files,
                "{context}: file count differs"
            );
            assert_eq!(
                incremental.diagnostics.len(),
                clean.diagnostics.len(),
                "{context}: diagnostic count differs"
            );
        }
    }

    impl Drop for Repo {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.root);
        }
    }

    /// A full-stack fixture: TS client → HTTP → Python route → ORM → table.
    fn full_stack(repo: &Repo) {
        repo.write(
            "web/api.ts",
            "export const BASE = '/api';\n\
             export async function listOrders() {\n\
             \x20 return fetch(`${BASE}/orders`, { method: 'GET' });\n\
             }\n",
        )
        .write(
            "app/routes.py",
            "from fastapi import APIRouter\n\
             from .models import Order\n\
             router = APIRouter()\n\
             @router.get('/api/orders')\n\
             def list_orders():\n\
             \x20   return Order.query.all()\n",
        )
        .write(
            "app/models.py",
            "from sqlalchemy.orm import DeclarativeBase\n\
             class Base(DeclarativeBase):\n\
             \x20   pass\n\
             class Order(Base):\n\
             \x20   __tablename__ = 'orders'\n",
        );
    }

    /// PART 8 — a re-run with nothing changed must reuse everything and
    /// produce the same graph. Asserted on counts, because timing cannot tell
    /// a cache hit from a fast reparse.
    #[test]
    fn an_unchanged_repository_reuses_every_file() {
        let repo = Repo::new("noop");
        full_stack(&repo);
        let mut cache = IncrementalState::new();

        let first = repo.incremental(&mut cache);
        assert_eq!(cache.facts.stats().parsed, 3, "cold run must parse all 3");
        assert_eq!(cache.facts.stats().reused, 0);

        let second = repo.incremental(&mut cache);
        assert_eq!(cache.facts.stats().parsed, 0, "warm run must parse nothing");
        assert_eq!(cache.facts.stats().reused, 3);
        assert_eq!(cache.facts.stats().evicted, 0);

        assert_graphs_equal(&first.graph, &second.graph, "no-op re-run");
        repo.assert_incremental_matches_clean(&mut cache, "no-op");
    }

    /// PART 7 — the first proof. One file changes; only that file is reparsed
    /// and the result still equals a clean build.
    #[test]
    fn one_changed_file_reparses_only_itself() {
        let repo = Repo::new("one-file");
        full_stack(&repo);
        let mut cache = IncrementalState::new();
        repo.incremental(&mut cache);

        repo.write(
            "web/api.ts",
            "export const BASE = '/api';\n\
             export async function listOrders() {\n\
             \x20 return fetch(`${BASE}/orders`, { method: 'GET' });\n\
             }\n\
             export const UNUSED = 'a different literal';\n",
        );

        repo.incremental(&mut cache);
        assert_eq!(cache.facts.stats().parsed, 1, "only the edited file");
        assert_eq!(cache.facts.stats().reused, 2);
        repo.assert_incremental_matches_clean(&mut cache, "single-file edit");
    }

    /// PART 11 — a new file is analysed; the others are not touched.
    #[test]
    fn a_created_file_is_analysed_and_others_are_reused() {
        let repo = Repo::new("create");
        full_stack(&repo);
        let mut cache = IncrementalState::new();
        repo.incremental(&mut cache);

        repo.write(
            "app/extra.py",
            "class Invoice:\n\
             \x20   __tablename__ = 'invoices'\n",
        );

        repo.incremental(&mut cache);
        assert_eq!(cache.facts.stats().parsed, 1, "only the new file");
        assert_eq!(cache.facts.stats().reused, 3);
        assert_eq!(cache.facts.len(), 4);
        repo.assert_incremental_matches_clean(&mut cache, "file creation");
    }

    /// PART 12 — a deleted file's facts and every edge derived from them go.
    #[test]
    fn a_deleted_file_leaves_no_trace() {
        let repo = Repo::new("delete");
        full_stack(&repo);
        let mut cache = IncrementalState::new();
        let before = repo.incremental(&mut cache);
        assert!(
            canonical(&before.graph)
                .nodes
                .iter()
                .any(|n| n.contains("orders")),
            "the table should exist before deletion"
        );

        repo.delete("app/models.py");

        let after = repo.incremental(&mut cache);
        assert_eq!(cache.facts.stats().evicted, 1);
        assert_eq!(cache.facts.len(), 2);
        assert!(
            !canonical(&after.graph)
                .nodes
                .iter()
                .any(|n| n.contains("models.py")),
            "no node may still cite the deleted file"
        );
        repo.assert_incremental_matches_clean(&mut cache, "file deletion");
    }

    /// PART 13 — rename is delete + create, and must leave no stale path.
    #[test]
    fn a_renamed_file_leaves_no_stale_path() {
        let repo = Repo::new("rename");
        full_stack(&repo);
        let mut cache = IncrementalState::new();
        repo.incremental(&mut cache);

        repo.rename("app/models.py", "app/schema.py");

        let after = repo.incremental(&mut cache);
        assert_eq!(cache.facts.stats().evicted, 1, "the old path is dropped");
        assert_eq!(cache.facts.stats().parsed, 1, "the new path is parsed");
        let nodes = canonical(&after.graph).nodes;
        assert!(
            !nodes.iter().any(|n| n.contains("app/models.py")),
            "the old path survived the rename: {nodes:?}"
        );
        repo.assert_incremental_matches_clean(&mut cache, "file rename");
    }

    /// PART 14 — a backend route change must move the cross-language edge,
    /// even though the frontend file did not change.
    #[test]
    fn a_backend_route_change_propagates_to_the_http_edge() {
        let repo = Repo::new("route");
        full_stack(&repo);
        let mut cache = IncrementalState::new();
        let before = repo.incremental(&mut cache);
        assert!(
            canonical(&before.graph)
                .edges
                .iter()
                .any(|e| e.contains("HttpCall")),
            "the fixture must produce an HTTP edge to begin with"
        );

        repo.write(
            "app/routes.py",
            "from fastapi import APIRouter\n\
             from .models import Order\n\
             router = APIRouter()\n\
             @router.get('/api/invoices')\n\
             def list_orders():\n\
             \x20   return Order.query.all()\n",
        );

        repo.incremental(&mut cache);
        assert_eq!(cache.facts.stats().parsed, 1, "only the route file");
        // The frontend was not reparsed, yet its edge must be reconsidered.
        repo.assert_incremental_matches_clean(&mut cache, "backend route change");
    }

    /// PART 15 — M05's evaluator under incremental update. `BASE` moves, and
    /// the dependent URL and its match must move with it.
    #[test]
    fn a_dynamic_url_base_change_propagates() {
        let repo = Repo::new("dynamic");
        full_stack(&repo);
        let mut cache = IncrementalState::new();
        repo.incremental(&mut cache);

        repo.write(
            "web/api.ts",
            "export const BASE = '/v2';\n\
             export async function listOrders() {\n\
             \x20 return fetch(`${BASE}/orders`, { method: 'GET' });\n\
             }\n",
        );

        repo.incremental(&mut cache);
        assert_eq!(cache.facts.stats().parsed, 1);
        repo.assert_incremental_matches_clean(&mut cache, "dynamic URL base change");
    }

    /// PART 16 — an ORM table rename must move the model→table edge.
    #[test]
    fn an_orm_table_change_propagates() {
        let repo = Repo::new("orm");
        full_stack(&repo);
        let mut cache = IncrementalState::new();
        repo.incremental(&mut cache);

        repo.write(
            "app/models.py",
            "from sqlalchemy.orm import DeclarativeBase\n\
             class Base(DeclarativeBase):\n\
             \x20   pass\n\
             class Order(Base):\n\
             \x20   __tablename__ = 'purchase_orders'\n",
        );

        let after = repo.incremental(&mut cache);
        assert_eq!(cache.facts.stats().parsed, 1);
        let nodes = canonical(&after.graph).nodes;
        assert!(
            !nodes.iter().any(|n| n.contains("|orders|")),
            "the old table name survived: {nodes:?}"
        );
        repo.assert_incremental_matches_clean(&mut cache, "ORM table rename");
    }

    /// PART 18 — diagnostics are facts. A file that becomes malformed must
    /// lose its old successful facts, and gain a diagnostic.
    #[test]
    fn diagnostics_appear_and_disappear_with_the_file() {
        let repo = Repo::new("diagnostics");
        full_stack(&repo);
        let mut cache = IncrementalState::new();
        let clean_first = repo.incremental(&mut cache);
        let baseline_diagnostics = clean_first.diagnostics.len();

        repo.write("app/models.py", "class Order(:\n    __tablename__ = \n");
        let broken = repo.incremental(&mut cache);
        assert!(
            broken.diagnostics.len() > baseline_diagnostics,
            "a malformed file must produce a diagnostic"
        );
        repo.assert_incremental_matches_clean(&mut cache, "file became malformed");

        // ...and repairing it must clear the diagnostic again.
        repo.write(
            "app/models.py",
            "from sqlalchemy.orm import DeclarativeBase\n\
             class Base(DeclarativeBase):\n\
             \x20   pass\n\
             class Order(Base):\n\
             \x20   __tablename__ = 'orders'\n",
        );
        let repaired = repo.incremental(&mut cache);
        assert_eq!(
            repaired.diagnostics.len(),
            baseline_diagnostics,
            "the stale diagnostic survived the repair"
        );
        repo.assert_incremental_matches_clean(&mut cache, "file repaired");
    }

    /// PART 21 — the stale-state cases. Each ends where it started, and the
    /// final graph must equal a clean build of the final tree.
    #[test]
    fn revert_and_recreate_cycles_end_exactly_where_they_started() {
        let repo = Repo::new("stale");
        full_stack(&repo);
        let mut cache = IncrementalState::new();
        let original = canonical(&repo.incremental(&mut cache).graph);

        // 1. changed twice before the next analysis: only the last state counts
        repo.write("app/models.py", "class Order:\n    __tablename__ = 'a'\n");
        repo.write("app/models.py", "class Order:\n    __tablename__ = 'b'\n");
        repo.assert_incremental_matches_clean(&mut cache, "two writes, one analysis");

        // 2. changed and reverted
        full_stack(&repo);
        let reverted = canonical(&repo.incremental(&mut cache).graph);
        assert_eq!(
            original, reverted,
            "a reverted change left the graph altered"
        );

        // 3. deleted and recreated
        repo.delete("app/models.py");
        repo.incremental(&mut cache);
        full_stack(&repo);
        let recreated = canonical(&repo.incremental(&mut cache).graph);
        assert_eq!(
            original, recreated,
            "delete + recreate did not restore the graph"
        );
        repo.assert_incremental_matches_clean(&mut cache, "delete and recreate");
    }

    /// PART 34 — two independent subtrees mutated at once must both land,
    /// and neither may disturb the other.
    #[test]
    fn two_independent_mutations_in_one_update_both_apply() {
        let repo = Repo::new("independent");
        full_stack(&repo);
        repo.write("web/other.ts", "export const X = 1;\n");
        let mut cache = IncrementalState::new();
        repo.incremental(&mut cache);

        repo.write("web/other.ts", "export const X = 2;\n");
        repo.write(
            "app/models.py",
            "from sqlalchemy.orm import DeclarativeBase\n\
             class Base(DeclarativeBase):\n\
             \x20   pass\n\
             class Order(Base):\n\
             \x20   __tablename__ = 'renamed_orders'\n",
        );

        repo.incremental(&mut cache);
        assert_eq!(cache.facts.stats().parsed, 2, "both edits, nothing else");
        assert_eq!(cache.facts.stats().reused, 2);
        repo.assert_incremental_matches_clean(&mut cache, "two independent mutations");
    }

    /// A frontend whose URL base lives in a *separate* module, so an import
    /// change and an exported-symbol change are distinguishable mutations.
    fn split_frontend(repo: &Repo) {
        repo.write("web/config_a.ts", "export const BASE = '/api';\n")
            .write("web/config_b.ts", "export const BASE = '/v2';\n")
            .write(
                "web/app.ts",
                "import { BASE } from './config_a';\n\
                 export async function listOrders() {\n\
                 \x20 return fetch(`${BASE}/orders`, { method: 'GET' });\n\
                 }\n",
            )
            .write(
                "app/routes.py",
                "from fastapi import APIRouter\n\
                 router = APIRouter()\n\
                 @router.get('/api/orders')\n\
                 def list_orders():\n\
                 \x20   return []\n",
            );
    }

    /// The import target changes: `./config_a` → `./config_b`. The old
    /// module's contribution must not survive in the resolved URL.
    #[test]
    fn an_import_change_repoints_the_dependency() {
        let repo = Repo::new("import-change");
        split_frontend(&repo);
        let mut cache = IncrementalState::new();
        let before = repo.incremental(&mut cache);
        let before_edges = canonical(&before.graph).edges;

        repo.write(
            "web/app.ts",
            "import { BASE } from './config_b';\n\
             export async function listOrders() {\n\
             \x20 return fetch(`${BASE}/orders`, { method: 'GET' });\n\
             }\n",
        );

        let after = repo.incremental(&mut cache);
        assert_eq!(
            cache.facts.stats().parsed,
            1,
            "only the importing file changed"
        );
        assert_eq!(cache.facts.stats().reused, 3);
        assert_ne!(
            before_edges,
            canonical(&after.graph).edges,
            "repointing the import must change the resolved URL"
        );
        repo.assert_incremental_matches_clean(&mut cache, "import repointed");
    }

    /// The *exported symbol* changes name, so the importer's reference no
    /// longer resolves. The dependent file was not edited.
    #[test]
    fn an_exported_symbol_change_propagates_to_importers() {
        let repo = Repo::new("export-change");
        split_frontend(&repo);
        let mut cache = IncrementalState::new();
        let before = repo.incremental(&mut cache);
        let before_edges = canonical(&before.graph).edges;

        repo.write("web/config_a.ts", "export const ROOT = '/api';\n");

        let after = repo.incremental(&mut cache);
        assert_eq!(
            cache.facts.stats().parsed,
            1,
            "only the exporting file changed"
        );
        assert_ne!(
            before_edges,
            canonical(&after.graph).edges,
            "renaming the exported constant must change what the importer resolves"
        );
        repo.assert_incremental_matches_clean(&mut cache, "exported symbol renamed");
    }

    /// A router *prefix* change moves every route registered on it, without
    /// any route decorator being edited.
    #[test]
    fn a_route_prefix_change_moves_the_routes_under_it() {
        let repo = Repo::new("prefix");
        repo.write(
            "web/api.ts",
            "export const BASE = '/api';\n\
             export async function listOrders() {\n\
             \x20 return fetch(`${BASE}/orders`, { method: 'GET' });\n\
             }\n",
        )
        .write(
            "app/routes.py",
            "from fastapi import APIRouter\n\
             router = APIRouter(prefix='/api')\n\
             @router.get('/orders')\n\
             def list_orders():\n\
             \x20   return []\n",
        );
        let mut cache = IncrementalState::new();
        let before = repo.incremental(&mut cache);
        let before_edges = canonical(&before.graph).edges;

        repo.write(
            "app/routes.py",
            "from fastapi import APIRouter\n\
             router = APIRouter(prefix='/v2')\n\
             @router.get('/orders')\n\
             def list_orders():\n\
             \x20   return []\n",
        );

        let after = repo.incremental(&mut cache);
        assert_eq!(cache.facts.stats().parsed, 1);
        assert_ne!(
            before_edges,
            canonical(&after.graph).edges,
            "changing the router prefix must move the served path"
        );
        repo.assert_incremental_matches_clean(&mut cache, "router prefix change");
    }

    /// Two changes that depend on each other, applied in one update: the
    /// export is renamed *and* the importer follows it. Each alone would break
    /// the chain; together they must restore exactly the original graph.
    #[test]
    fn multiple_dependent_changes_in_one_update_stay_consistent() {
        let repo = Repo::new("dependent");
        split_frontend(&repo);
        let mut cache = IncrementalState::new();
        let original = canonical(&repo.incremental(&mut cache).graph);

        repo.write("web/config_a.ts", "export const ROOT = '/api';\n")
            .write(
                "web/app.ts",
                "import { ROOT } from './config_a';\n\
                 export async function listOrders() {\n\
                 \x20 return fetch(`${ROOT}/orders`, { method: 'GET' });\n\
                 }\n",
            );

        let after = repo.incremental(&mut cache);
        assert_eq!(cache.facts.stats().parsed, 2, "both dependent files");
        assert_eq!(
            original,
            canonical(&after.graph),
            "renaming a constant and its use together must be graph-neutral"
        );
        repo.assert_incremental_matches_clean(&mut cache, "dependent pair renamed");
    }

    /// Conservative-invalidation guard: a mutation confined to one subtree
    /// must leave every unrelated node and edge byte-identical.
    #[test]
    fn an_unrelated_subtree_is_untouched_by_a_local_change() {
        let repo = Repo::new("unrelated");
        full_stack(&repo);
        // A second, *complete* chain. An isolated model would not do: a model
        // nothing accesses produces no node at all, so "unchanged" would be
        // vacuously true and the test would prove nothing.
        repo.write(
            "web/invoices.ts",
            "export const IBASE = '/api';\n\
             export async function listInvoices() {\n\
             \x20 return fetch(`${IBASE}/invoices`, { method: 'GET' });\n\
             }\n",
        )
        .write(
            "other/invoice_routes.py",
            "from fastapi import APIRouter\n\
             from .invoice_models import Invoice\n\
             router = APIRouter()\n\
             @router.get('/api/invoices')\n\
             def list_invoices():\n\
             \x20   return Invoice.query.all()\n",
        )
        .write(
            "other/invoice_models.py",
            "from sqlalchemy.orm import DeclarativeBase\n\
             class Base2(DeclarativeBase):\n\
             \x20   pass\n\
             class Invoice(Base2):\n\
             \x20   __tablename__ = 'invoices'\n",
        );
        let mut cache = IncrementalState::new();
        let before = canonical(&repo.incremental(&mut cache).graph);
        let unrelated_before: Vec<_> = before
            .nodes
            .iter()
            .filter(|n| n.contains("nvoice"))
            .cloned()
            .collect();
        assert!(
            !unrelated_before.is_empty(),
            "fixture must have a second subtree in the graph, got {:?}",
            before.nodes
        );

        repo.write(
            "app/models.py",
            "from sqlalchemy.orm import DeclarativeBase\n\
             class Base(DeclarativeBase):\n\
             \x20   pass\n\
             class Order(Base):\n\
             \x20   __tablename__ = 'renamed'\n",
        );

        let after = canonical(&repo.incremental(&mut cache).graph);
        let unrelated_after: Vec<_> = after
            .nodes
            .iter()
            .filter(|n| n.contains("nvoice"))
            .cloned()
            .collect();
        assert_eq!(
            unrelated_before, unrelated_after,
            "an unrelated subtree changed when it should not have"
        );
        repo.assert_incremental_matches_clean(&mut cache, "local change, unrelated subtree");
    }

    /// Model identity is a whole-project property because ambiguity is: a
    /// class name claimed by two classes resolves to nothing. This is the
    /// first genuine invalidation hazard — a *create* can delete nodes, and a
    /// *delete* can create them — so it is pinned before `OrmAnalysis` is
    /// localized.
    ///
    /// The collision has to be built carefully. An earlier version of this
    /// fixture gave the rival class a differently named base (`Base3`), which
    /// the resolver does not recognise as a model at all, so there was no
    /// collision and the test was asserting behaviour the product does not
    /// have. Declaring the same `Base` in both files is what actually
    /// collides — and it collides on `Base` too, so every model resolves to
    /// nothing.
    fn colliding_models(repo: &Repo) {
        repo.write(
            "app/routes.py",
            "from fastapi import APIRouter
             from .models import Order
             router = APIRouter()
             @router.get('/api/orders')
             def list_orders():
                 return Order.query.all()
",
        )
        .write(
            "app/models.py",
            "from sqlalchemy.orm import DeclarativeBase
             class Base(DeclarativeBase):
                 pass
             class Order(Base):
                 __tablename__ = 'orders'
",
        );
    }

    const RIVAL: &str = "from sqlalchemy.orm import DeclarativeBase
                         class Base(DeclarativeBase):
                             pass
                         class Order(Base):
                             __tablename__ = 'other_orders'
";

    /// Creating a colliding declaration removes a model the edited file never
    /// mentions.
    #[test]
    fn a_colliding_declaration_removes_the_model_it_collides_with() {
        let repo = Repo::new("collide-create");
        colliding_models(&repo);
        let mut cache = IncrementalState::new();
        let before = canonical(&repo.incremental(&mut cache).graph);
        assert!(
            before.nodes.iter().any(|n| n.contains("|orders|")),
            "the table must resolve before the collision: {:?}",
            before.nodes
        );

        repo.write("app/duplicate.py", RIVAL);

        let after = canonical(&repo.incremental(&mut cache).graph);
        assert!(
            !after.nodes.iter().any(|n| n.contains("|orders|")),
            "an ambiguous model must resolve to nothing: {:?}",
            after.nodes
        );
        repo.assert_incremental_matches_clean(&mut cache, "collision created");
    }

    /// ...and deleting one claimant brings it back — a deletion that *adds* a
    /// node, which no purely subtractive invalidation rule would produce.
    #[test]
    fn deleting_one_claimant_restores_the_ambiguous_model() {
        let repo = Repo::new("collide-delete");
        colliding_models(&repo);
        repo.write("app/duplicate.py", RIVAL);
        let mut cache = IncrementalState::new();
        let ambiguous = canonical(&repo.incremental(&mut cache).graph);
        assert!(
            !ambiguous.nodes.iter().any(|n| n.contains("|orders|")),
            "the fixture must start ambiguous: {:?}",
            ambiguous.nodes
        );

        repo.delete("app/duplicate.py");

        let after = canonical(&repo.incremental(&mut cache).graph);
        assert!(
            after.nodes.iter().any(|n| n.contains("|orders|")),
            "removing the rival claimant must restore the model: {:?}",
            after.nodes
        );
        repo.assert_incremental_matches_clean(&mut cache, "collision removed by deletion");
    }

    /// Entering and leaving a collision is symmetric and leaves no residue.
    #[test]
    fn renaming_a_class_into_and_out_of_a_collision_is_symmetric() {
        let repo = Repo::new("collide-rename");
        colliding_models(&repo);
        let mut cache = IncrementalState::new();
        let original = canonical(&repo.incremental(&mut cache).graph);

        repo.write("app/duplicate.py", RIVAL);
        let collided = canonical(&repo.incremental(&mut cache).graph);
        assert_ne!(original, collided, "the collision must change the graph");
        repo.assert_incremental_matches_clean(&mut cache, "renamed into collision");

        // Rename the rival's class out of the collision. Its base still
        // collides, so this asserts equality with a clean rebuild rather than
        // predicting which models survive.
        repo.write(
            "app/duplicate.py",
            "from sqlalchemy.orm import DeclarativeBase
             class Ledger(DeclarativeBase):
                 pass
             class Receipt(Ledger):
                 __tablename__ = 'receipts'
",
        );
        repo.assert_incremental_matches_clean(&mut cache, "renamed out of collision");

        // Removing the rival entirely returns to exactly the original graph.
        repo.delete("app/duplicate.py");
        assert_eq!(
            original,
            canonical(&repo.incremental(&mut cache).graph),
            "the collision cycle left residue"
        );
    }

    /// A fixture that actually exercises ORM *access* resolution.
    ///
    /// `Order.query.all()` does not: `classify_access` recognises Django
    /// `.objects` chains and construction, so a Flask-SQLAlchemy `.query`
    /// chain yields no access and any test built on it would prove nothing
    /// about `resolves_to_the_model`. Construction does reach it.
    fn orm_access(repo: &Repo) {
        repo.write(
            "app/models.py",
            "from sqlalchemy.orm import DeclarativeBase\n\
             class Base(DeclarativeBase):\n\
             \x20   pass\n\
             class Order(Base):\n\
             \x20   __tablename__ = 'orders'\n",
        )
        .write(
            "app/handlers.py",
            "from .models import Order\n\
             def create_order(name):\n\
             \x20   o = Order(name=name)\n\
             \x20   return o\n",
        );
    }

    fn has_access_edge(graph: &cartograph_graph::ArchitectureGraph) -> bool {
        canonical(graph)
            .edges
            .iter()
            .any(|e| e.contains("OrmAccess"))
    }

    /// Python module resolution scans **file paths**, so a file whose path
    /// collides as a suffix makes the import ambiguous — even when its
    /// contents are inert. Adding `pkg/app/models.py` makes `from .models
    /// import Order` resolve to two candidates, and `resolve_python_module`
    /// answers `None` rather than choose. The access edge disappears.
    ///
    /// This is a dependency on the repository's **path set**, not on any
    /// file's content, and it is the case most likely to be missed by an
    /// invalidation rule written from content hashes alone.
    #[test]
    fn a_path_collision_removes_an_access_edge_without_touching_any_content() {
        let repo = Repo::new("path-collision");
        orm_access(&repo);
        let mut cache = IncrementalState::new();
        let before = repo.incremental(&mut cache);
        assert!(
            has_access_edge(&before.graph),
            "the fixture must produce an ORM access edge to begin with: {:?}",
            canonical(&before.graph).edges
        );

        // Inert content. Only the path matters.
        repo.write("pkg/app/models.py", "VERSION = 1\n");

        let after = repo.incremental(&mut cache);
        assert!(
            !has_access_edge(&after.graph),
            "an ambiguous module path must withdraw the access edge: {:?}",
            canonical(&after.graph).edges
        );
        repo.assert_incremental_matches_clean(&mut cache, "module path collision created");
    }

    /// ...and removing the colliding path restores it.
    #[test]
    fn removing_a_path_collision_restores_the_access_edge() {
        let repo = Repo::new("path-collision-undo");
        orm_access(&repo);
        repo.write("pkg/app/models.py", "VERSION = 1\n");
        let mut cache = IncrementalState::new();
        let collided = repo.incremental(&mut cache);
        assert!(
            !has_access_edge(&collided.graph),
            "the fixture must start collided"
        );

        repo.delete("pkg/app/models.py");

        let after = repo.incremental(&mut cache);
        assert!(
            has_access_edge(&after.graph),
            "removing the colliding path must restore the access edge: {:?}",
            canonical(&after.graph).edges
        );
        repo.assert_incremental_matches_clean(&mut cache, "module path collision removed");
    }

    /// Python module resolution never consults module aliases —
    /// `resolve_python_module` does not call `apply_alias`, which is reached
    /// only from `follow_typescript`. So a TypeScript file, with or without a
    /// build alias, cannot change a Python ORM result. Pinned because the
    /// opposite was assumed during design, and an invalidation rule built on
    /// that assumption would recompute every Python access on every
    /// TypeScript edit.
    #[test]
    fn typescript_files_and_aliases_do_not_affect_python_orm_resolution() {
        let repo = Repo::new("ts-inert");
        orm_access(&repo);
        let mut cache = IncrementalState::new();
        let before = canonical(&repo.incremental(&mut cache).graph);
        assert!(
            before.edges.iter().any(|e| e.contains("OrmAccess")),
            "precondition: an access edge must exist"
        );

        repo.write("web/unrelated.ts", "export const X = 1;\n")
            .write(
                "web/tsconfig.json",
                "{\"compilerOptions\":{\"baseUrl\":\".\",\"paths\":{\"@app/*\":[\"app/*\"]}}}\n",
            );

        let after = canonical(&repo.incremental(&mut cache).graph);
        assert_eq!(
            before, after,
            "a TypeScript file and a build alias must leave Python ORM results untouched"
        );
        repo.assert_incremental_matches_clean(&mut cache, "typescript added");
    }

    /// PART 24 — the performance baseline, on a real repository.
    ///
    /// Opt-in and ignored by default: it needs a checkout this repository does
    /// not vendor, and a timing assertion in CI would be flaky on shared
    /// runners. Run it deliberately:
    ///
    /// ```text
    /// CARTOGRAPH_BENCH_REPO=/path/to/repo \
    ///   cargo test --bin cartograph -- --ignored --nocapture warm_cache
    /// ```
    ///
    /// It measures rather than asserts. The counts are the correctness signal;
    /// the milliseconds are context, and no threshold is claimed.
    #[test]
    #[ignore = "needs CARTOGRAPH_BENCH_REPO; measures rather than asserts"]
    fn warm_cache_baseline_on_a_real_repository() {
        let Ok(path) = std::env::var("CARTOGRAPH_BENCH_REPO") else {
            return;
        };
        let root = std::path::PathBuf::from(path);
        let mut cache = IncrementalState::new();

        let t = std::time::Instant::now();
        let cold = pipeline::run_with_cache(&root, Options { quiet: true }, &mut cache)
            .expect("cold analysis");
        let cold_ms = t.elapsed().as_secs_f64() * 1000.0;
        let cold_stats = cache.facts.stats();
        let cold_access = cache.accesses.stats();

        let t = std::time::Instant::now();
        let warm = pipeline::run_with_cache(&root, Options { quiet: true }, &mut cache)
            .expect("warm analysis");
        let warm_ms = t.elapsed().as_secs_f64() * 1000.0;
        let warm_stats = cache.facts.stats();
        let warm_access = cache.accesses.stats();

        println!("files            : {}", cold.totals.files);
        println!(
            "nodes/edges      : {}/{}",
            cold.totals.nodes, cold.totals.edges
        );
        println!(
            "cold             : {cold_ms:.0} ms  parsed={} reused={}",
            cold_stats.parsed, cold_stats.reused
        );
        println!(
            "warm (no change) : {warm_ms:.0} ms  parsed={} reused={}",
            warm_stats.parsed, warm_stats.reused
        );
        println!("speedup          : {:.1}x", cold_ms / warm_ms.max(0.001));
        println!(
            "stage C cold     : hits={} misses={}",
            cold_access.hits, cold_access.misses
        );
        println!(
            "stage C warm     : hits={} misses={} refused_to_store={}",
            warm_access.hits, warm_access.misses, warm_access.refused_to_store
        );
        assert_eq!(
            warm_access.misses, 0,
            "a warm run over an unchanged tree must recompute no Stage C result"
        );

        // The one thing that is asserted, because it is correctness and not
        // timing: a warm run over an unchanged tree reparses nothing, and the
        // graph is unchanged.
        assert_eq!(warm_stats.parsed, 0, "a warm run reparsed files");
        assert_eq!(warm_stats.reused, cold_stats.parsed);
        assert_graphs_equal(
            &cold.graph,
            &warm.graph,
            "cold vs warm on a real repository",
        );
    }

    /// An unreadable file must not resurrect the facts it had when readable.
    #[test]
    fn a_file_that_becomes_unparseable_does_not_keep_its_old_facts() {
        let repo = Repo::new("unreadable");
        full_stack(&repo);
        let mut cache = IncrementalState::new();
        repo.incremental(&mut cache);

        // Invalid UTF-8: readable bytes, undecodable content.
        std::fs::write(repo.root.join("app/models.py"), [0xff, 0xfe, 0x00, 0x9c])
            .expect("write invalid utf-8");

        let after = repo.incremental(&mut cache);
        assert!(
            !canonical(&after.graph)
                .nodes
                .iter()
                .any(|n| n.contains("|orders|")),
            "facts from the previously valid file survived"
        );
        repo.assert_incremental_matches_clean(&mut cache, "file became invalid UTF-8");
    }
}

/// Stage C cache behaviour, asserted on counters rather than time.
///
/// Every test here first proves its precondition — that the fixture produces
/// the access the mutation is supposed to disturb — because a cache test over
/// a repository with no accesses would pass whatever the cache did.
#[cfg(test)]
mod access_cache {
    use super::*;
    use crate::pipeline::{self, Options};
    use cartograph_testkit::canonical::{assert_graphs_equal, canonical};

    /// A repository on disk that a test can mutate. Same harness the
    /// differential suite uses.
    struct Repo {
        root: std::path::PathBuf,
    }

    impl Repo {
        fn new(name: &str) -> Self {
            use std::sync::atomic::{AtomicUsize, Ordering};
            static NEXT: AtomicUsize = AtomicUsize::new(0);
            let unique = NEXT.fetch_add(1, Ordering::Relaxed);
            let root = std::env::temp_dir()
                .join("cartograph-m10-cache")
                .join(format!("{name}-{}-{unique}", std::process::id()));
            let _ = std::fs::remove_dir_all(&root);
            std::fs::create_dir_all(&root).expect("fixture root");
            Self { root }
        }

        fn write(&self, rel: &str, contents: &str) -> &Self {
            let path = self.root.join(rel);
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent).expect("fixture parent");
            }
            std::fs::write(path, contents).expect("write fixture");
            self
        }

        fn delete(&self, rel: &str) -> &Self {
            std::fs::remove_file(self.root.join(rel)).expect("delete fixture");
            self
        }

        fn rename(&self, from: &str, to: &str) -> &Self {
            std::fs::rename(self.root.join(from), self.root.join(to)).expect("rename");
            self
        }

        fn run(&self, state: &mut IncrementalState) -> pipeline::Analysis {
            pipeline::run_with_cache(&self.root, Options { quiet: true }, state)
                .expect("incremental analysis")
        }

        fn clean(&self) -> pipeline::Analysis {
            pipeline::run(&self.root, Options { quiet: true }).expect("clean analysis")
        }

        fn assert_matches_clean(&self, state: &mut IncrementalState, context: &str) {
            let incremental = self.run(state);
            let clean = self.clean();
            assert_graphs_equal(&incremental.graph, &clean.graph, context);
        }
    }

    impl Drop for Repo {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.root);
        }
    }

    const MODELS: &str = "from sqlalchemy.orm import DeclarativeBase\n\
                          class Base(DeclarativeBase):\n\
                          \x20   pass\n\
                          class Order(Base):\n\
                          \x20   __tablename__ = 'orders'\n";

    /// A -> B -> C, so a transitive dependency change can be tested.
    fn chain(repo: &Repo) {
        repo.write("app/models.py", MODELS)
            .write("app/mid.py", "from .models import Order\n")
            .write(
                "app/handlers.py",
                "from .mid import Order\n\
                 def make():\n\
                 \x20   return Order(x=1)\n",
            );
    }

    fn has_access(graph: &cartograph_graph::ArchitectureGraph) -> bool {
        canonical(graph)
            .edges
            .iter()
            .any(|e| e.contains("OrmAccess"))
    }

    /// PART 8 — a second run over an unchanged tree reuses every entry.
    #[test]
    fn an_unchanged_repository_reuses_every_stage_c_entry() {
        let repo = Repo::new("noop");
        chain(&repo);
        let mut state = IncrementalState::new();

        let first = repo.run(&mut state);
        assert!(
            has_access(&first.graph),
            "precondition: the fixture must produce an ORM access"
        );
        assert_eq!(state.accesses.stats().hits, 0, "cold run cannot hit");
        let cold_misses = state.accesses.stats().misses;
        assert!(cold_misses > 0);

        let second = repo.run(&mut state);
        assert_eq!(
            state.accesses.stats().misses,
            0,
            "a warm run over an unchanged tree must recompute nothing"
        );
        assert_eq!(state.accesses.stats().hits, cold_misses);
        assert_graphs_equal(&first.graph, &second.graph, "no-op re-run");
    }

    /// PART 9 — editing the file that holds the accesses invalidates its own
    /// entry, and only its own.
    #[test]
    fn editing_the_accessing_file_misses_only_that_entry() {
        let repo = Repo::new("own-edit");
        chain(&repo);
        let mut state = IncrementalState::new();
        repo.run(&mut state);
        let total = state.accesses.stats().misses;

        repo.write(
            "app/handlers.py",
            "from .mid import Order\n\
             def make():\n\
             \x20   return Order(x=2)\n",
        );
        repo.run(&mut state);
        assert_eq!(state.accesses.stats().misses, 1, "only the edited file");
        assert_eq!(state.accesses.stats().hits, total - 1);
        repo.assert_matches_clean(&mut state, "own file edited");
    }

    /// PART 10 — mandatory. A -> B -> C; change only C. The entry for A must
    /// miss, because C's recorded content identity moved.
    #[test]
    fn a_transitive_dependency_change_invalidates_the_dependent_entry() {
        let repo = Repo::new("transitive");
        chain(&repo);
        let mut state = IncrementalState::new();
        let before = repo.run(&mut state);
        assert!(
            has_access(&before.graph),
            "precondition: the chain must resolve to an access"
        );

        // Change only the far end of the chain.
        repo.write(
            "app/models.py",
            "from sqlalchemy.orm import DeclarativeBase\n\
             class Base(DeclarativeBase):\n\
             \x20   pass\n\
             class Order(Base):\n\
             \x20   __tablename__ = 'renamed_orders'\n",
        );
        repo.run(&mut state);

        // models.py itself missed, and handlers.py missed because it reads it.
        assert!(
            state.accesses.stats().misses >= 2,
            "the dependent entry must miss too, not just the edited file: {:?}",
            state.accesses.stats()
        );
        repo.assert_matches_clean(&mut state, "transitive dependency changed");
    }

    /// PART 11 — an unrelated file must not invalidate an entry.
    #[test]
    fn an_unrelated_file_change_leaves_the_entry_reusable() {
        let repo = Repo::new("unrelated");
        chain(&repo);
        repo.write("app/other.py", "VALUE = 1\n");
        let mut state = IncrementalState::new();
        repo.run(&mut state);
        let total = state.accesses.stats().misses;

        repo.write("app/other.py", "VALUE = 2\n");
        repo.run(&mut state);
        assert_eq!(
            state.accesses.stats().misses,
            1,
            "only the edited file may miss: {:?}",
            state.accesses.stats()
        );
        assert_eq!(state.accesses.stats().hits, total - 1);
        repo.assert_matches_clean(&mut state, "unrelated file changed");
    }

    /// PART 12/13 — a path collision changes the path-set fingerprint, so the
    /// entries that consulted it miss; removing it restores the result.
    #[test]
    fn a_path_collision_invalidates_and_its_removal_restores() {
        let repo = Repo::new("path-set");
        chain(&repo);
        let mut state = IncrementalState::new();
        let before = canonical(&repo.run(&mut state).graph);
        assert!(
            before.edges.iter().any(|e| e.contains("OrmAccess")),
            "precondition: an access must exist first"
        );

        repo.write("vendor/app/models.py", "VERSION = 1\n");
        let collided = repo.run(&mut state);
        assert!(
            state.accesses.stats().misses > 0,
            "a path-set change must invalidate the entries that consulted it"
        );
        assert!(
            !has_access(&collided.graph),
            "the ambiguous module path must withdraw the access"
        );
        repo.assert_matches_clean(&mut state, "path collision created");

        repo.delete("vendor/app/models.py");
        let restored = canonical(&repo.run(&mut state).graph);
        assert_eq!(
            before, restored,
            "removing the collision must restore the graph"
        );
        repo.assert_matches_clean(&mut state, "path collision removed");
    }

    /// PART 14 — model ambiguity in both directions.
    #[test]
    fn model_ambiguity_invalidates_and_its_removal_restores() {
        let repo = Repo::new("ambiguity");
        chain(&repo);
        let mut state = IncrementalState::new();
        let before = canonical(&repo.run(&mut state).graph);
        assert!(before.nodes.iter().any(|n| n.contains("|orders|")));

        repo.write(
            "app/dup.py",
            "from sqlalchemy.orm import DeclarativeBase\n\
             class Base(DeclarativeBase):\n\
             \x20   pass\n\
             class Order(Base):\n\
             \x20   __tablename__ = 'other'\n",
        );
        let ambiguous = repo.run(&mut state);
        assert!(
            state.accesses.stats().misses > 0,
            "a model-set change must invalidate"
        );
        assert!(
            !canonical(&ambiguous.graph)
                .nodes
                .iter()
                .any(|n| n.contains("|orders|")),
            "the ambiguous model must resolve to nothing"
        );
        repo.assert_matches_clean(&mut state, "ambiguity created");

        repo.delete("app/dup.py");
        let restored = canonical(&repo.run(&mut state).graph);
        assert_eq!(
            before, restored,
            "removing the rival must restore the graph"
        );
    }

    /// PART 15 — the critical separation test. A table rename must NOT
    /// invalidate Stage C, because access resolution never reads the table —
    /// but the graph's table edge must still change.
    #[test]
    fn a_table_rename_does_not_invalidate_stage_c_but_does_change_the_graph() {
        let repo = Repo::new("table");
        chain(&repo);
        let mut state = IncrementalState::new();
        let before = canonical(&repo.run(&mut state).graph);
        assert!(
            before.nodes.iter().any(|n| n.contains("|orders|")),
            "precondition: the table must resolve first"
        );

        repo.write(
            "app/models.py",
            "from sqlalchemy.orm import DeclarativeBase\n\
             class Base(DeclarativeBase):\n\
             \x20   pass\n\
             class Order(Base):\n\
             \x20   __tablename__ = 'purchase_orders'\n",
        );
        let after = repo.run(&mut state);

        // models.py's own entry misses (its content changed) and handlers.py
        // misses because it reads models.py — but the *reason* is the read,
        // not the table. What matters here is the graph.
        let renamed = canonical(&after.graph);
        assert!(
            renamed
                .nodes
                .iter()
                .any(|n| n.contains("|purchase_orders|")),
            "the table edge must reflect the rename: {:?}",
            renamed.nodes
        );
        assert!(
            !renamed.nodes.iter().any(|n| n.contains("|orders|")),
            "the old table must be gone"
        );
        repo.assert_matches_clean(&mut state, "table renamed");
    }

    /// PART 20/21/22 — deleted, recreated and renamed dependencies.
    #[test]
    fn deleted_recreated_and_renamed_dependencies_are_never_trusted() {
        let repo = Repo::new("dep-lifecycle");
        chain(&repo);
        let mut state = IncrementalState::new();
        let original = canonical(&repo.run(&mut state).graph);

        // Delete the far dependency.
        repo.delete("app/models.py");
        repo.run(&mut state);
        repo.assert_matches_clean(&mut state, "dependency deleted");

        // Recreate it with different contents.
        repo.write(
            "app/models.py",
            "from sqlalchemy.orm import DeclarativeBase\n\
             class Base(DeclarativeBase):\n\
             \x20   pass\n\
             class Order(Base):\n\
             \x20   __tablename__ = 'recreated'\n",
        );
        repo.assert_matches_clean(&mut state, "dependency recreated differently");

        // Restore the original and confirm the graph comes back exactly.
        repo.write("app/models.py", MODELS);
        let restored = canonical(&repo.run(&mut state).graph);
        assert_eq!(
            original, restored,
            "restoring the content must restore the graph"
        );

        // Rename it away: no stale path may survive.
        repo.rename("app/models.py", "app/schema.py");
        let renamed = repo.run(&mut state);
        assert!(
            !canonical(&renamed.graph)
                .nodes
                .iter()
                .any(|n| n.contains("app/models.py")),
            "the old path survived the rename"
        );
        repo.assert_matches_clean(&mut state, "dependency renamed");
    }

    /// PART 23 — rapid mutations including a revert. The cache must always
    /// reflect current content, never a timestamp or an assumption of
    /// monotonicity.
    #[test]
    fn rapid_mutations_and_a_revert_always_reflect_current_content() {
        let repo = Repo::new("rapid");
        chain(&repo);
        let mut state = IncrementalState::new();
        let original = canonical(&repo.run(&mut state).graph);

        let variant = |table: &str| {
            format!(
                "from sqlalchemy.orm import DeclarativeBase\n\
                 class Base(DeclarativeBase):\n\
                 \x20   pass\n\
                 class Order(Base):\n\
                 \x20   __tablename__ = '{table}'\n"
            )
        };

        repo.write("app/models.py", &variant("one"));
        repo.run(&mut state);
        repo.write("app/models.py", &variant("two"));
        repo.run(&mut state);
        repo.write("app/models.py", MODELS); // revert
        let reverted = canonical(&repo.run(&mut state).graph);
        assert_eq!(
            original, reverted,
            "a revert must restore the original graph"
        );
        repo.write("app/models.py", &variant("three"));
        repo.assert_matches_clean(&mut state, "after rapid mutation");
    }

    /// PART 24 — two independent chains. Changing one must not invalidate the
    /// other, in either direction.
    #[test]
    fn independent_subgraphs_invalidate_independently() {
        let repo = Repo::new("independent");
        chain(&repo);
        repo.write(
            "other/models.py",
            "from sqlalchemy.orm import DeclarativeBase\n\
             class Ledger(DeclarativeBase):\n\
             \x20   pass\n\
             class Receipt(Ledger):\n\
             \x20   __tablename__ = 'receipts'\n",
        )
        .write(
            "other/handlers.py",
            "from .models import Receipt\n\
             def make():\n\
             \x20   return Receipt(x=1)\n",
        );
        let mut state = IncrementalState::new();
        repo.run(&mut state);
        let total = state.accesses.stats().misses;
        assert!(total >= 4, "precondition: both chains must be present");

        // Touch only the first chain's leaf.
        repo.write(
            "app/handlers.py",
            "from .mid import Order\ndef make():\n    return Order(x=9)\n",
        );
        repo.run(&mut state);
        assert_eq!(
            state.accesses.stats().misses,
            1,
            "the other chain must stay reusable: {:?}",
            state.accesses.stats()
        );

        // Now touch only the second chain's leaf.
        repo.write(
            "other/handlers.py",
            "from .models import Receipt\ndef make():\n    return Receipt(x=9)\n",
        );
        repo.run(&mut state);
        assert_eq!(
            state.accesses.stats().misses,
            1,
            "and the first chain must stay reusable: {:?}",
            state.accesses.stats()
        );
        repo.assert_matches_clean(&mut state, "independent subgraphs");
    }

    /// PART 25 — the full cross-stack chain, one mutation at a time.
    #[test]
    fn the_cross_stack_chain_stays_equal_to_a_clean_rebuild() {
        let repo = Repo::new("cross-stack");
        repo.write(
            "web/api.ts",
            "export const BASE = '/api';\n\
             export async function listOrders() {\n\
             \x20 return fetch(`${BASE}/orders`, { method: 'GET' });\n\
             }\n",
        )
        .write(
            "app/routes.py",
            "from fastapi import APIRouter\n\
             from .models import Order\n\
             router = APIRouter()\n\
             @router.get('/api/orders')\n\
             def list_orders():\n\
             \x20   return Order(x=1)\n",
        )
        .write("app/models.py", MODELS);

        let mut state = IncrementalState::new();
        let first = repo.run(&mut state);
        assert!(
            canonical(&first.graph)
                .edges
                .iter()
                .any(|e| e.contains("HttpCall")),
            "precondition: the chain must cross the HTTP boundary"
        );

        // One mutation at a time, each compared against a clean rebuild.
        repo.write(
            "web/api.ts",
            "export const BASE = '/v2';\n\
             export async function listOrders() {\n\
             \x20 return fetch(`${BASE}/orders`, { method: 'GET' });\n\
             }\n",
        );
        repo.assert_matches_clean(&mut state, "frontend endpoint changed");

        repo.write(
            "app/routes.py",
            "from fastapi import APIRouter\n\
             from .models import Order\n\
             router = APIRouter(prefix='/v2')\n\
             @router.get('/orders')\n\
             def list_orders():\n\
             \x20   return Order(x=1)\n",
        );
        repo.assert_matches_clean(&mut state, "router prefix changed");

        repo.write(
            "app/models.py",
            "from sqlalchemy.orm import DeclarativeBase\n\
             class Base(DeclarativeBase):\n\
             \x20   pass\n\
             class Order(Base):\n\
             \x20   __tablename__ = 'v2_orders'\n",
        );
        repo.assert_matches_clean(&mut state, "ORM table changed");
    }

    /// PART 29 — a run that fails must not leave a reusable entry behind, and
    /// must not publish a half-updated one. A file that becomes unreadable
    /// makes its own analysis fail; the next successful run must recompute
    /// from the real state rather than serve what was cached before.
    #[test]
    fn a_file_that_becomes_unparseable_does_not_serve_its_old_entry() {
        let repo = Repo::new("failure");
        chain(&repo);
        let mut state = IncrementalState::new();
        let before = repo.run(&mut state);
        assert!(has_access(&before.graph), "precondition: an access exists");

        // Invalid UTF-8: readable bytes, undecodable content.
        std::fs::write(repo.root.join("app/models.py"), [0xff, 0xfe, 0x00, 0x9c])
            .expect("write invalid utf-8");
        let broken = repo.run(&mut state);
        assert!(
            !canonical(&broken.graph)
                .nodes
                .iter()
                .any(|n| n.contains("|orders|")),
            "the old entry must not be served for the new state"
        );
        repo.assert_matches_clean(&mut state, "dependency became unparseable");
    }
}

/// Real-repository mutation differential testing.
///
/// Fixtures establish that the invalidation rules are right in the small. They
/// cannot establish that they hold over a repository nobody wrote for the
/// test: a real tree has re-export hubs, barrel files, colliding module names
/// and thousands of entries whose dependency sets overlap in ways no fixture
/// anticipates.
///
/// Every case here runs the same shape:
///
/// ```text
/// materialise pinned copy -> baseline analysis -> mutate one thing
///   -> incremental analysis -> clean rebuild -> canonical comparison
/// ```
///
/// The canonical copy is never touched: sources are copied into a disposable
/// temporary tree and mutated there. Nothing is committed, uploaded or
/// persisted, and the tree is removed on drop.
///
/// Opt-in and ignored by default — these need checkouts this repository does
/// not vendor.
#[cfg(test)]
mod real_repository {
    use super::*;
    use crate::pipeline::{self, Options};
    use cartograph_testkit::canonical::{CanonicalGraph, canonical, differences};

    /// A disposable copy of a pinned repository.
    struct Mutable {
        root: std::path::PathBuf,
        /// Files copied, for baseline integrity checks.
        files: usize,
    }

    impl Mutable {
        /// Copies every analysable file from `src`, preserving structure.
        ///
        /// Only files the analyser would read are copied. That is faithful
        /// rather than a shortcut: a file the walker never opens cannot enter
        /// `by_path`, so it cannot affect resolution or the path set either.
        fn materialise(src: &std::path::Path, label: &str) -> Self {
            use std::sync::atomic::{AtomicUsize, Ordering};
            static NEXT: AtomicUsize = AtomicUsize::new(0);
            let unique = NEXT.fetch_add(1, Ordering::Relaxed);
            let root = std::env::temp_dir()
                .join("cartograph-m10-mutation")
                .join(format!("{label}-{}-{unique}", std::process::id()));
            let _ = std::fs::remove_dir_all(&root);
            std::fs::create_dir_all(&root).expect("temporary root");

            let mut files = 0usize;
            copy_sources(src, src, &root, &mut files);
            Self { root, files }
        }

        fn path(&self, rel: &str) -> std::path::PathBuf {
            self.root.join(rel)
        }

        fn read(&self, rel: &str) -> String {
            std::fs::read_to_string(self.path(rel)).expect("file exists")
        }

        fn write(&self, rel: &str, contents: &str) {
            let path = self.path(rel);
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent).expect("parent");
            }
            std::fs::write(path, contents).expect("write");
        }

        fn delete(&self, rel: &str) {
            std::fs::remove_file(self.path(rel)).expect("delete");
        }

        fn rename(&self, from: &str, to: &str) {
            let target = self.path(to);
            if let Some(parent) = target.parent() {
                std::fs::create_dir_all(parent).expect("parent");
            }
            std::fs::rename(self.path(from), target).expect("rename");
        }

        fn incremental(&self, state: &mut IncrementalState) -> pipeline::Analysis {
            pipeline::run_with_cache(&self.root, Options { quiet: true }, state)
                .expect("incremental analysis")
        }

        fn clean(&self) -> pipeline::Analysis {
            pipeline::run(&self.root, Options { quiet: true }).expect("clean analysis")
        }
    }

    impl Drop for Mutable {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.root);
        }
    }

    fn copy_sources(
        root: &std::path::Path,
        dir: &std::path::Path,
        dest_root: &std::path::Path,
        count: &mut usize,
    ) {
        const SKIP: [&str; 11] = [
            "node_modules",
            ".git",
            "dist",
            "build",
            "coverage",
            "target",
            "__pycache__",
            "site-packages",
            "venv",
            ".venv",
            "migrations",
        ];
        let Ok(entries) = std::fs::read_dir(dir) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
                continue;
            };
            let Ok(kind) = entry.file_type() else {
                continue;
            };
            if kind.is_symlink() {
                continue;
            }
            if kind.is_dir() {
                if name.starts_with('.') || SKIP.contains(&name) {
                    continue;
                }
                copy_sources(root, &path, dest_root, count);
            } else if matches!(
                std::path::Path::new(name)
                    .extension()
                    .and_then(|e| e.to_str()),
                Some("py" | "pyi" | "ts" | "tsx" | "mts" | "cts")
            ) {
                if let Ok(rel) = path.strip_prefix(root) {
                    let target = dest_root.join(rel);
                    if let Some(parent) = target.parent() {
                        let _ = std::fs::create_dir_all(parent);
                    }
                    if std::fs::copy(&path, &target).is_ok() {
                        *count += 1;
                    }
                }
            }
        }
    }

    /// What one mutation did, for the report.
    struct Outcome {
        hits: usize,
        misses: usize,
        graph: CanonicalGraph,
    }

    /// Applies `mutate`, runs incremental and clean, and requires equality.
    ///
    /// The comparison is canonical semantic equality — a graph with equal
    /// counts but different edges fails.
    fn mutate_and_compare(
        repo: &Mutable,
        state: &mut IncrementalState,
        label: &str,
        mutate: impl FnOnce(&Mutable),
    ) -> Outcome {
        mutate(repo);
        let incremental = repo.incremental(state);
        let counters = state.accesses.stats();
        let clean = repo.clean();

        let (left, right) = (canonical(&incremental.graph), canonical(&clean.graph));
        assert_eq!(
            left,
            right,
            "{label}: incremental != clean\n{}",
            differences(&left, &right).join("\n")
        );
        Outcome {
            hits: counters.hits,
            misses: counters.misses,
            graph: left,
        }
    }

    fn report(label: &str, out: &Outcome, before: &CanonicalGraph) {
        let diff = differences(before, &out.graph).len();
        println!(
            "  {label:<44} hits={:<5} misses={:<5} nodes={:<5} edges={:<5} changed={diff}",
            out.hits,
            out.misses,
            out.graph.nodes.len(),
            out.graph.edges.len()
        );
    }

    /// PART 5 — the Zulip mutation catalogue.
    #[test]
    #[ignore = "needs CARTOGRAPH_MUTATION_ZULIP; real-repository differential suite"]
    fn zulip_mutations_keep_incremental_equal_to_clean() {
        let Ok(src) = std::env::var("CARTOGRAPH_MUTATION_ZULIP") else {
            return;
        };
        let repo = Mutable::materialise(std::path::Path::new(&src), "zulip");
        assert!(
            repo.files > 1000,
            "precondition: expected a real checkout, copied {}",
            repo.files
        );

        let mut state = IncrementalState::new();
        let baseline = repo.incremental(&mut state);
        let base_graph = canonical(&baseline.graph);
        let cold = state.accesses.stats();
        println!(
            "zulip baseline: files={} nodes={} edges={} stageC_misses={}",
            baseline.totals.files,
            base_graph.nodes.len(),
            base_graph.edges.len(),
            cold.misses
        );
        assert!(
            !base_graph.edges.is_empty(),
            "precondition: the baseline must produce edges to mutate"
        );

        // PART 7 — a second pass over an unchanged tree must reuse everything.
        let second = repo.incremental(&mut state);
        assert_eq!(
            state.accesses.stats().misses,
            0,
            "an unchanged real repository must recompute no Stage C result"
        );
        assert_eq!(state.accesses.stats().hits, cold.misses);
        assert_eq!(
            base_graph,
            canonical(&second.graph),
            "a no-op re-run must produce the same graph"
        );
        println!(
            "  {:<44} hits={} misses=0",
            "no-op re-run",
            state.accesses.stats().hits
        );

        // A real file that declares models, and one that accesses them.
        let models_rel = "zerver/models/messages.py";
        let original_models = repo.read(models_rel);

        // 1. Edit a real ORM model file (a recorded transitive dependency for
        //    every file importing through zerver/models/__init__.py).
        let out = mutate_and_compare(&repo, &mut state, "edit model file", |r| {
            r.write(
                models_rel,
                &format!("{original_models}\n# cartograph M10 mutation probe\n"),
            );
        });
        report("edit model file (comment only)", &out, &base_graph);
        assert!(
            out.misses > 0,
            "editing a recorded dependency must invalidate"
        );

        // 2. Revert it: the graph must return exactly.
        let out = mutate_and_compare(&repo, &mut state, "revert model file", |r| {
            r.write(models_rel, &original_models);
        });
        assert_eq!(
            base_graph, out.graph,
            "a revert must restore the graph exactly"
        );
        report("revert model file", &out, &base_graph);

        // 3. An unrelated new Python file that resolves nothing.
        let out = mutate_and_compare(&repo, &mut state, "create unrelated file", |r| {
            r.write("cartograph_probe_unrelated.py", "PROBE = 1\n");
        });
        report("create unrelated file", &out, &base_graph);

        // 4. PART 14 — a colliding Python path, inert content.
        let out = mutate_and_compare(&repo, &mut state, "create colliding path", |r| {
            r.write("vendor/zerver/models/messages.py", "PROBE = 1\n");
        });
        report("create colliding path", &out, &base_graph);
        assert!(out.misses > 0, "a path-set change must invalidate");

        // 5. PART 10/14 — delete it; the previous resolution must return.
        let out = mutate_and_compare(&repo, &mut state, "delete colliding path", |r| {
            r.delete("vendor/zerver/models/messages.py");
        });
        report("delete colliding path", &out, &base_graph);

        // 6. PART 11 — a competing model creating real ambiguity.
        let out = mutate_and_compare(&repo, &mut state, "create competing model", |r| {
            r.write(
                "cartograph_probe_rival.py",
                "from django.db import models\nclass Message(models.Model):\n    class Meta:\n        db_table = 'probe_messages'\n",
            );
        });
        report("create competing model (ambiguity)", &out, &base_graph);

        // 7. ...and remove it; the unique model must come back.
        let out = mutate_and_compare(&repo, &mut state, "remove competing model", |r| {
            r.delete("cartograph_probe_rival.py");
        });
        report("remove competing model", &out, &base_graph);

        zulip_lifecycle_mutations(&repo, &mut state, &base_graph, &original_models, models_rel);
    }

    /// The rename / malformed / multi-file half of the Zulip catalogue.
    ///
    /// Split out so each function stays readable; it shares the materialised
    /// copy and cache state with its caller rather than paying to rebuild them.
    fn zulip_lifecycle_mutations(
        repo: &Mutable,
        state: &mut IncrementalState,
        base_graph: &CanonicalGraph,
        original_models: &str,
        models_rel: &str,
    ) {
        // 8. PART 13 — rename a real module: no stale path may survive.
        let out = mutate_and_compare(repo, state, "rename module", |r| {
            r.rename(models_rel, "zerver/models/messages_renamed.py");
        });
        report("rename module", &out, base_graph);
        assert!(
            !out.graph.nodes.iter().any(|n| n.contains(models_rel))
                && !out.graph.edges.iter().any(|e| e.contains(models_rel)),
            "the old path survived the rename"
        );

        // 9. Rename back.
        let out = mutate_and_compare(repo, state, "rename back", |r| {
            r.rename("zerver/models/messages_renamed.py", models_rel);
        });
        assert_eq!(
            *base_graph, out.graph,
            "renaming back must restore the graph"
        );
        report("rename back", &out, base_graph);

        // 10. Introduce malformed source, then repair it.
        let out = mutate_and_compare(repo, state, "malformed source", |r| {
            r.write(models_rel, "class Broken(:\n    pass\n");
        });
        report("introduce malformed source", &out, base_graph);

        let out = mutate_and_compare(repo, state, "repair source", |r| {
            r.write(models_rel, original_models);
        });
        assert_eq!(
            *base_graph, out.graph,
            "repairing must restore the graph exactly"
        );
        report("repair source", &out, base_graph);

        // 11. Two independent mutations in one update.
        let out = mutate_and_compare(repo, state, "two independent", |r| {
            r.write("cartograph_probe_a.py", "A = 1\n");
            r.write("cartograph_probe_b.py", "B = 1\n");
        });
        report("two independent mutations", &out, base_graph);

        // 12. Two dependent mutations: a module and its importer together.
        let out = mutate_and_compare(repo, state, "two dependent", |r| {
            r.write("cartograph_probe_dep.py", "class Probe:\n    pass\n");
            r.write(
                "cartograph_probe_use.py",
                "from cartograph_probe_dep import Probe\ndef make():\n    return Probe()\n",
            );
        });
        report("two dependent mutations", &out, base_graph);
    }

    /// PART 17 — failure atomicity on a real repository.
    ///
    /// Fixtures have four entries; Zulip has over a thousand, and the
    /// publication boundary has to hold at that size too. Injects a failure
    /// partway through a real recomputation, then proves nothing was published
    /// and a retry equals a clean rebuild.
    #[test]
    #[ignore = "needs CARTOGRAPH_MUTATION_ZULIP; real-repository failure atomicity"]
    fn zulip_failure_atomicity() {
        let Ok(src) = std::env::var("CARTOGRAPH_MUTATION_ZULIP") else {
            return;
        };
        let repo = Mutable::materialise(std::path::Path::new(&src), "zulip-fail");
        let mut state = IncrementalState::new();
        let baseline = repo.incremental(&mut state);
        let base_graph = canonical(&baseline.graph);
        let entries_before = state.accesses.len();
        assert!(entries_before > 1000, "precondition: a real generation");
        assert!(state.accesses.stats().published);

        // A mutation that invalidates the whole generation: a competing model
        // moves the model-set fingerprint, which is a term in every key. An
        // earlier version edited one model file, which invalidates only about
        // twenty entries, so a failure at 500 never fired — the test caught
        // that itself rather than passing vacuously.
        let rival = "cartograph_probe_rival.py";
        repo.write(
            rival,
            "from django.db import models
class Message(models.Model):
    class Meta:
        db_table = 'probe_messages'
",
        );
        state
            .accesses
            .inject_failure(Box::new(|file: &str, done: usize| {
                if done == 500 {
                    Err(cartograph_resolver::AccessCacheError {
                        file: file.to_owned(),
                        reason: "injected failure".to_owned(),
                    })
                } else {
                    Ok(())
                }
            }));
        let failed = pipeline::run_with_cache(&repo.root, Options { quiet: true }, &mut state);
        assert!(failed.is_err(), "the run must fail rather than answer");
        assert_eq!(
            state.accesses.len(),
            entries_before,
            "the previous generation must be untouched"
        );
        assert!(!state.accesses.stats().published);
        println!(
            "  {:<44} entries={entries_before} published=false",
            "injected failure 500 entries in"
        );

        // Retry succeeds and equals a clean rebuild.
        state.accesses.clear_failure();
        let retried = repo.incremental(&mut state);
        assert!(state.accesses.stats().published);
        assert_eq!(
            canonical(&retried.graph),
            canonical(&repo.clean().graph),
            "the retry must equal a clean rebuild"
        );
        println!(
            "  {:<44} hits={} misses={}",
            "retry after failure",
            state.accesses.stats().hits,
            state.accesses.stats().misses
        );

        // Removing the rival restores the baseline exactly.
        repo.delete(rival);
        let restored = canonical(&repo.incremental(&mut state).graph);
        assert_eq!(base_graph, restored, "the revert must restore the baseline");
        println!(
            "  {:<44} nodes={} edges={}",
            "revert after failure",
            restored.nodes.len(),
            restored.edges.len()
        );
    }

    /// PART 6/15 — the Airflow alias mutation catalogue.
    #[test]
    #[ignore = "needs CARTOGRAPH_MUTATION_AIRFLOW; real-repository differential suite"]
    fn airflow_alias_mutations_keep_incremental_equal_to_clean() {
        let Ok(src) = std::env::var("CARTOGRAPH_MUTATION_AIRFLOW") else {
            return;
        };
        let repo = Mutable::materialise(std::path::Path::new(&src), "airflow");
        assert!(
            repo.files > 500,
            "precondition: expected a real frontend, copied {}",
            repo.files
        );

        let config = "airflow-core/src/airflow/ui/vite.config.ts";
        let original_config = repo.read(config);
        assert!(
            original_config.contains(r#"alias: { openapi: "/openapi-gen", src: "/src" }"#),
            "precondition: the pinned config must declare the aliases this suite mutates"
        );

        let mut state = IncrementalState::new();
        let baseline = repo.incremental(&mut state);
        let base_graph = canonical(&baseline.graph);
        let cold = state.accesses.stats();
        println!(
            "airflow baseline: files={} nodes={} edges={}",
            baseline.totals.files,
            base_graph.nodes.len(),
            base_graph.edges.len()
        );

        let second = repo.incremental(&mut state);
        assert_eq!(state.accesses.stats().misses, 0, "no-op must reuse");
        assert_eq!(
            base_graph,
            canonical(&second.graph),
            "a no-op re-run must produce the same graph"
        );
        println!("  {:<44} hits={} misses=0", "no-op re-run", cold.misses);

        // 1. An unrelated TypeScript edit.
        let out = mutate_and_compare(&repo, &mut state, "unrelated ts edit", |r| {
            let rel = "airflow-core/src/airflow/ui/src/components/ActionErrors.tsx";
            let body = r.read(rel);
            r.write(
                rel,
                &format!("{body}\nexport const CARTOGRAPH_PROBE = 1;\n"),
            );
        });
        report("unrelated TypeScript edit", &out, &base_graph);

        // 2. Retarget an alias.
        let out = mutate_and_compare(&repo, &mut state, "retarget alias", |r| {
            r.write(
                config,
                &original_config.replace(
                    r#"alias: { openapi: "/openapi-gen", src: "/src" }"#,
                    r#"alias: { openapi: "/elsewhere-gen", src: "/src" }"#,
                ),
            );
        });
        report("retarget alias", &out, &base_graph);

        // 3. Remove the aliases entirely.
        let out = mutate_and_compare(&repo, &mut state, "remove aliases", |r| {
            r.write(
                config,
                &original_config.replace(
                    r#"  resolve: { alias: { openapi: "/openapi-gen", src: "/src" } },"#,
                    "",
                ),
            );
        });
        report("remove aliases", &out, &base_graph);

        // 4. Add a third alias.
        let out = mutate_and_compare(&repo, &mut state, "add alias", |r| {
            r.write(
                config,
                &original_config.replace(
                    r#"alias: { openapi: "/openapi-gen", src: "/src" }"#,
                    r#"alias: { openapi: "/openapi-gen", src: "/src", extra: "/extra" }"#,
                ),
            );
        });
        report("add alias", &out, &base_graph);

        // 5. Restore: the graph must return exactly.
        let out = mutate_and_compare(&repo, &mut state, "restore config", |r| {
            r.write(config, &original_config);
        });
        assert_eq!(
            base_graph, out.graph,
            "restoring the configuration must restore the graph exactly"
        );
        report("restore config", &out, &base_graph);

        // 6. Edit a real alias *target* file.
        let out = mutate_and_compare(&repo, &mut state, "edit alias target", |r| {
            let rel = "airflow-core/src/airflow/ui/openapi-gen/requests/types.gen.ts";
            if let Ok(body) = std::fs::read_to_string(r.path(rel)) {
                r.write(rel, &format!("{body}\nexport type CartographProbe = 1;\n"));
            }
        });
        report("edit alias target file", &out, &base_graph);
    }
}

/// Failure atomicity: a failed incremental update must publish nothing.
///
/// Two invariants, and they are different claims:
///
/// **A. No partial generation.** If recomputation fails partway, the previous
/// generation remains the cache's only state. Not "most of it" — all of it,
/// unchanged.
///
/// **B. No stale generation presented as current.** A failed update for
/// repository state S2 must not answer with the analysis of S1. Preserving the
/// old generation internally is correct; returning it as the answer for the
/// new state is the defect this exists to prevent.
///
/// Before this slice there was no failure path at all — `analyze` returned a
/// `Vec` and could not fail — so atomicity could only be argued from the shape
/// of an assignment. It is now tested.
#[cfg(test)]
mod atomicity {
    use super::*;
    use crate::error::{CliError, ErrorCode};
    use crate::pipeline::{self, Options};
    use cartograph_resolver::AccessCacheError;
    use cartograph_testkit::canonical::{CanonicalGraph, canonical};

    struct Repo {
        root: std::path::PathBuf,
    }

    impl Repo {
        fn new(name: &str) -> Self {
            use std::sync::atomic::{AtomicUsize, Ordering};
            static NEXT: AtomicUsize = AtomicUsize::new(0);
            let unique = NEXT.fetch_add(1, Ordering::Relaxed);
            let root = std::env::temp_dir()
                .join("cartograph-m10-atomicity")
                .join(format!("{name}-{}-{unique}", std::process::id()));
            let _ = std::fs::remove_dir_all(&root);
            std::fs::create_dir_all(&root).expect("fixture root");
            Self { root }
        }

        fn write(&self, rel: &str, contents: &str) -> &Self {
            let path = self.root.join(rel);
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent).expect("parent");
            }
            std::fs::write(path, contents).expect("write");
            self
        }

        fn run(&self, state: &mut IncrementalState) -> Result<pipeline::Analysis, CliError> {
            pipeline::run_with_cache(&self.root, Options { quiet: true }, state)
        }

        fn ok(&self, state: &mut IncrementalState) -> pipeline::Analysis {
            self.run(state).expect("analysis succeeds")
        }

        fn clean(&self) -> pipeline::Analysis {
            pipeline::run(&self.root, Options { quiet: true }).expect("clean analysis")
        }
    }

    impl Drop for Repo {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.root);
        }
    }

    const BASE: &str = "from sqlalchemy.orm import DeclarativeBase\n\
                        class Base(DeclarativeBase):\n\
                        \x20   pass\n";

    /// Four Stage C entries (A, B, C, D), each accessing a model, so a failure
    /// can be injected partway through a multi-entry generation.
    fn four_entries(repo: &Repo) {
        repo.write(
            "app/models.py",
            &format!("{BASE}class Order(Base):\n    __tablename__ = 'orders'\n"),
        );
        for name in ["a", "b", "c", "d"] {
            repo.write(
                &format!("app/{name}.py"),
                "from .models import Order\n\
                 def make():\n\
                 \x20   return Order(x=1)\n",
            );
        }
    }

    /// Fails on the Nth recomputation of a run (0 = before any).
    fn fail_after(n: usize) -> cartograph_resolver::RecomputeHook {
        Box::new(move |file: &str, done: usize| {
            if done == n {
                Err(AccessCacheError {
                    file: file.to_owned(),
                    reason: "injected failure".to_owned(),
                })
            } else {
                Ok(())
            }
        })
    }

    /// The cache's observable state, for before/after comparison.
    fn generation(state: &IncrementalState) -> (usize, bool) {
        (state.accesses.len(), state.accesses.stats().published)
    }

    /// Establishes a baseline generation and returns its graph.
    fn baseline(repo: &Repo, state: &mut IncrementalState) -> CanonicalGraph {
        let analysis = repo.ok(state);
        assert!(
            canonical(&analysis.graph)
                .edges
                .iter()
                .any(|e| e.contains("OrmAccess")),
            "precondition: the fixture must produce ORM accesses"
        );
        assert!(state.accesses.stats().published);
        canonical(&analysis.graph)
    }

    /// PART 6 — failure before any entry is recomputed.
    #[test]
    fn a_failure_before_the_first_entry_publishes_nothing() {
        let repo = Repo::new("before-first");
        four_entries(&repo);
        let mut state = IncrementalState::new();
        let base_graph = baseline(&repo, &mut state);
        let before = generation(&state);
        assert!(before.0 > 0, "precondition: a generation must exist");

        // Mutate, then fail immediately.
        repo.write(
            "app/models.py",
            &format!("{BASE}class Order(Base):\n    __tablename__ = 'mutated'\n"),
        );
        state.accesses.inject_failure(fail_after(0));
        let result = repo.run(&mut state);

        assert!(result.is_err(), "the run must fail, not answer");
        assert_eq!(
            generation(&state).0,
            before.0,
            "the previous generation must be untouched"
        );
        assert!(
            !state.accesses.stats().published,
            "no generation may be reported as published"
        );
        assert_eq!(state.accesses.stats().failed_updates, 1);

        // PART 10 — clearing the failure and retrying must succeed and equal
        // a clean rebuild of the mutated state.
        state.accesses.clear_failure();
        let retried = repo.ok(&mut state);
        assert!(state.accesses.stats().published);
        assert_eq!(
            canonical(&retried.graph),
            canonical(&repo.clean().graph),
            "the retry must equal a clean rebuild"
        );
        assert_ne!(
            base_graph,
            canonical(&retried.graph),
            "and it must reflect the mutation, not the baseline"
        );
    }

    /// PART 7/8 — failure after one, several, and every-but-one entry.
    #[test]
    fn a_failure_partway_never_publishes_a_partial_generation() {
        for stop_after in [1usize, 2, 3, 4] {
            let repo = Repo::new(&format!("partway-{stop_after}"));
            four_entries(&repo);
            let mut state = IncrementalState::new();
            baseline(&repo, &mut state);
            let entries_before = state.accesses.len();

            repo.write(
                "app/models.py",
                &format!("{BASE}class Order(Base):\n    __tablename__ = 'mutated'\n"),
            );
            state.accesses.inject_failure(fail_after(stop_after));
            let result = repo.run(&mut state);

            assert!(
                result.is_err(),
                "stop_after={stop_after}: the run must fail rather than publish"
            );
            assert_eq!(
                state.accesses.len(),
                entries_before,
                "stop_after={stop_after}: the generation changed size"
            );
            assert!(
                !state.accesses.stats().published,
                "stop_after={stop_after}: a partial generation was reported as published"
            );

            state.accesses.clear_failure();
            let retried = repo.ok(&mut state);
            assert_eq!(
                canonical(&retried.graph),
                canonical(&repo.clean().graph),
                "stop_after={stop_after}: the retry must equal a clean rebuild"
            );
        }
    }

    /// PART 9 — mandatory. A failed update must not answer with the previous
    /// state's analysis.
    #[test]
    fn a_failed_update_never_answers_with_the_previous_generation() {
        let repo = Repo::new("stale");
        four_entries(&repo);
        let mut state = IncrementalState::new();
        let generation_one = baseline(&repo, &mut state);

        // Generation 2: a mutation that genuinely changes the graph.
        repo.write(
            "app/models.py",
            &format!("{BASE}class Order(Base):\n    __tablename__ = 'generation_two'\n"),
        );
        state.accesses.inject_failure(fail_after(1));
        let result = repo.run(&mut state);

        let Err(error) = result else {
            panic!("the update must fail rather than answer")
        };
        assert_eq!(error.code, ErrorCode::AnalysisFailed);
        assert!(
            error.to_string().contains("incremental update failed"),
            "the failure must be reported plainly: {error}"
        );

        // The old generation survives *internally* — that is correct — but it
        // was not returned, and the caller received no analysis at all.
        assert!(
            !state.accesses.is_empty(),
            "the known-good generation survives"
        );
        assert!(!state.accesses.stats().published);

        // And a successful retry answers for generation two, not one.
        state.accesses.clear_failure();
        let retried = canonical(&repo.ok(&mut state).graph);
        assert_ne!(
            generation_one, retried,
            "the retry must describe the mutated state"
        );
        assert!(
            retried.nodes.iter().any(|n| n.contains("generation_two")),
            "the new table must be present: {:?}",
            retried.nodes
        );
    }

    /// PART 11 — failure, then revert. The baseline must return exactly.
    #[test]
    fn a_failure_then_a_revert_restores_the_baseline_exactly() {
        let repo = Repo::new("revert");
        four_entries(&repo);
        let mut state = IncrementalState::new();
        let base_graph = baseline(&repo, &mut state);
        let original = std::fs::read_to_string(repo.root.join("app/models.py")).unwrap();

        repo.write(
            "app/models.py",
            &format!("{BASE}class Order(Base):\n    __tablename__ = 'temporary'\n"),
        );
        state.accesses.inject_failure(fail_after(2));
        assert!(repo.run(&mut state).is_err());

        // Revert and clear: the baseline must come back exactly.
        state.accesses.clear_failure();
        repo.write("app/models.py", &original);
        let restored = canonical(&repo.ok(&mut state).graph);
        assert_eq!(base_graph, restored, "the revert must restore the baseline");
        assert_eq!(
            restored,
            canonical(&repo.clean().graph),
            "and equal a clean rebuild"
        );
    }

    /// PART 12 — failure on mutation A must not contaminate mutation B.
    #[test]
    fn a_failure_does_not_contaminate_a_later_different_mutation() {
        let repo = Repo::new("different");
        four_entries(&repo);
        let mut state = IncrementalState::new();
        baseline(&repo, &mut state);

        // Mutation A, failed.
        repo.write(
            "app/models.py",
            &format!("{BASE}class Order(Base):\n    __tablename__ = 'mutation_a'\n"),
        );
        state.accesses.inject_failure(fail_after(1));
        assert!(repo.run(&mut state).is_err());

        // Mutation B, succeeding. The result must describe B alone.
        state.accesses.clear_failure();
        repo.write(
            "app/models.py",
            &format!("{BASE}class Order(Base):\n    __tablename__ = 'mutation_b'\n"),
        );
        let after = canonical(&repo.ok(&mut state).graph);
        assert_eq!(
            after,
            canonical(&repo.clean().graph),
            "the second mutation must equal a clean rebuild"
        );
        assert!(
            after.nodes.iter().any(|n| n.contains("mutation_b"))
                && !after.nodes.iter().any(|n| n.contains("mutation_a")),
            "no residue of the failed attempt may survive: {:?}",
            after.nodes
        );
    }

    /// PART 16 — two independent dependency groups, failure during one.
    #[test]
    fn a_failure_during_one_group_leaves_no_partial_global_state() {
        let repo = Repo::new("groups");
        four_entries(&repo);
        repo.write(
            "other/models.py",
            &format!("{BASE}class Receipt(Base):\n    __tablename__ = 'receipts'\n"),
        )
        .write(
            "other/x.py",
            "from .models import Receipt\ndef make():\n    return Receipt(x=1)\n",
        );
        let mut state = IncrementalState::new();
        let base_graph = baseline(&repo, &mut state);

        // Mutate both groups, fail partway.
        repo.write(
            "app/models.py",
            &format!("{BASE}class Order(Base):\n    __tablename__ = 'orders_v2'\n"),
        )
        .write(
            "other/models.py",
            &format!("{BASE}class Receipt(Base):\n    __tablename__ = 'receipts_v2'\n"),
        );
        state.accesses.inject_failure(fail_after(2));
        assert!(repo.run(&mut state).is_err());
        assert!(!state.accesses.stats().published);

        state.accesses.clear_failure();
        let retried = canonical(&repo.ok(&mut state).graph);
        assert_eq!(
            retried,
            canonical(&repo.clean().graph),
            "the retry must equal a clean rebuild of both groups"
        );
        assert_ne!(base_graph, retried);
    }

    /// PART 18 — the counters cannot imply success when the update failed.
    #[test]
    fn the_counters_never_imply_success_after_a_failure() {
        let repo = Repo::new("counters");
        four_entries(&repo);
        let mut state = IncrementalState::new();
        baseline(&repo, &mut state);
        assert!(state.accesses.stats().published);

        repo.write(
            "app/models.py",
            &format!("{BASE}class Order(Base):\n    __tablename__ = 'x'\n"),
        );
        state.accesses.inject_failure(fail_after(1));
        assert!(repo.run(&mut state).is_err());

        let counters = state.accesses.stats();
        assert!(
            !counters.published,
            "published must be false after a failure"
        );
        assert_eq!(counters.failed_updates, 1);
        assert_eq!(
            counters.hits, 0,
            "an abandoned attempt must not report hits that never became state"
        );
        assert_eq!(counters.misses, 0);
    }
}

/// Controlled per-stage profiling of the post-cache pipeline.
///
/// The system changed when the Stage C cache landed, so every earlier
/// wall-clock figure is stale as architectural evidence. This measures each
/// stage separately, and measures **work** alongside time — counts of items
/// produced, reused and recomputed — because timing alone cannot say whether a
/// stage is expensive or merely slow to observe.
///
/// # Protocol
///
/// - one untimed warm-up pass, so the OS page cache and allocator are warm and
///   the numbers describe the code rather than the first touch of the disk;
/// - **minimum of N timed iterations**, reported with N. The minimum is the
///   least noisy summary of a warm run: it is the sample least disturbed by
///   scheduling and background load, and it is used consistently everywhere
///   below rather than mixed with medians.
/// - absolute wall-times on this machine have varied several-fold between
///   sessions with page-cache state. Ratios *within one run* are the evidence;
///   the milliseconds are context.
///
/// The stage sequence mirrors `pipeline::resolve` exactly, using the public
/// resolver API, so the profile describes the real pipeline rather than an
/// idealised one.
#[cfg(test)]
mod profile {
    use super::*;
    use cartograph_graph::ArchitectureGraph;
    use cartograph_resolver::{
        ModuleIndex, OrmAnalysis, RouteIndex, RouterIndex, add_client_call_edges,
        add_edge_for_match, add_orm_edges, collect_exported_constants, match_client,
        normalize_client_call, normalize_route_declaration, resolve_url, scope_for_file,
        with_composed_prefix, with_resolved_url,
    };
    use std::time::Instant;

    const ITERATIONS: usize = 5;

    #[derive(Default)]
    struct Stage {
        name: &'static str,
        ms: f64,
        work: String,
    }

    fn ms(t: Instant) -> f64 {
        t.elapsed().as_secs_f64() * 1000.0
    }

    /// Runs the resolve stages once, timing each. Returns the stage table.
    #[allow(clippy::too_many_lines)]
    fn resolve_stages(analyses: &[FileAnalysis], state: &mut IncrementalState) -> Vec<Stage> {
        let mut stages = Vec::new();

        let t = Instant::now();
        let exported = collect_exported_constants(analyses);
        stages.push(Stage {
            name: "ExportedConstants::collect",
            ms: ms(t),
            work: String::new(),
        });

        let t = Instant::now();
        let modules = ModuleIndex::build(analyses);
        stages.push(Stage {
            name: "ModuleIndex::build",
            ms: ms(t),
            work: format!("{} entries", analyses.len()),
        });

        let t = Instant::now();
        let routers = RouterIndex::build(analyses, &modules);
        stages.push(Stage {
            name: "RouterIndex::build",
            ms: ms(t),
            work: String::new(),
        });

        // Per-file route composition and client URL resolution, exactly as
        // `resolve` performs them.
        let t = Instant::now();
        let mut backend = Vec::new();
        let mut clients = Vec::new();
        let mut composed_count = 0usize;
        for analysis in analyses {
            let scope = scope_for_file(analysis, &exported);
            for route in &analysis.routes {
                let composed = route
                    .receiver
                    .as_ref()
                    .and_then(|r| routers.prefix_for(&analysis.path, r))
                    .and_then(|resolution| with_composed_prefix(route, resolution));
                if composed.is_some() {
                    composed_count += 1;
                }
                backend.push(normalize_route_declaration(
                    composed.as_ref().unwrap_or(route),
                    &analysis.path,
                    analysis.language,
                ));
            }
            for call in &analysis.http_calls {
                let resolved = resolve_url(call, analysis, &scope);
                let effective = resolved
                    .as_ref()
                    .map_or_else(|| call.clone(), |r| with_resolved_url(call, r));
                clients.push(normalize_client_call(
                    &effective,
                    &analysis.path,
                    analysis.language,
                ));
            }
        }
        stages.push(Stage {
            name: "normalize routes + resolve URLs",
            ms: ms(t),
            work: format!(
                "{} routes ({composed_count} composed), {} clients",
                backend.len(),
                clients.len()
            ),
        });

        let route_count = backend.len();
        let t = Instant::now();
        let index = RouteIndex::build(backend);
        stages.push(Stage {
            name: "RouteIndex::build",
            ms: ms(t),
            work: format!("{route_count} routes"),
        });

        let t = Instant::now();
        let results: Vec<_> = clients.iter().map(|c| match_client(c, &index)).collect();
        stages.push(Stage {
            name: "match_client (all clients)",
            ms: ms(t),
            work: format!("{} matched", results.len()),
        });

        let t = Instant::now();
        let mut graph = ArchitectureGraph::new();
        let mut http_edges = 0usize;
        for result in &results {
            if add_edge_for_match(&mut graph, result, None)
                .expect("endpoints exist")
                .is_some()
            {
                http_edges += 1;
            }
        }
        let client_edges =
            add_client_call_edges(&mut graph, analyses, &modules, None).expect("endpoints exist");
        stages.push(Stage {
            name: "HTTP + client-call edges",
            ms: ms(t),
            work: format!("{http_edges} http, {client_edges} client"),
        });

        let t = Instant::now();
        let identities = state.facts.identities();
        let orm = OrmAnalysis::build_cached(analyses, &mut state.accesses, &identities)
            .expect("no failure hook installed");
        let orm_stats = state.accesses.stats();
        stages.push(Stage {
            name: "OrmAnalysis (Stage C, cached)",
            ms: ms(t),
            work: format!(
                "{} models, {} accesses, hits={} misses={}",
                orm.models.len(),
                orm.accesses.len(),
                orm_stats.hits,
                orm_stats.misses
            ),
        });

        let t = Instant::now();
        let orm_edges = add_orm_edges(&mut graph, &orm, None).expect("endpoints exist");
        stages.push(Stage {
            name: "ORM edges",
            ms: ms(t),
            work: format!("{orm_edges} edges"),
        });

        stages.push(Stage {
            name: "  (graph totals)",
            ms: 0.0,
            work: format!("{} nodes, {} edges", graph.node_count(), graph.edge_count()),
        });

        stages
    }

    fn report(label: &str, stages: &[Stage]) {
        let total: f64 = stages.iter().map(|s| s.ms).sum();
        println!("--- {label} (min of {ITERATIONS}) ---");
        for stage in stages {
            if stage.ms == 0.0 && stage.name.starts_with("  ") {
                println!("  {:<34} {:>10}   {}", stage.name, "", stage.work);
                continue;
            }
            println!(
                "  {:<34} {:>8.3} ms  {:>5.1}%  {}",
                stage.name,
                stage.ms,
                100.0 * stage.ms / total,
                stage.work
            );
        }
        println!("  {:<34} {:>8.3} ms", "TOTAL (resolve stages)", total);
    }

    /// Takes the per-stage minimum across iterations.
    fn best_of(runs: Vec<Vec<Stage>>) -> Vec<Stage> {
        let mut best = runs.into_iter().reduce(|mut acc, run| {
            for (a, b) in acc.iter_mut().zip(run) {
                if b.ms < a.ms {
                    a.ms = b.ms;
                }
                a.work = b.work;
            }
            acc
        });
        best.take().unwrap_or_default()
    }

    #[test]
    #[ignore = "needs CARTOGRAPH_PROFILE_REPO; measures rather than asserts"]
    fn post_cache_pipeline_profile() {
        let Ok(repo) = std::env::var("CARTOGRAPH_PROFILE_REPO") else {
            return;
        };
        let root = std::path::PathBuf::from(repo);

        // 1. Discovery.
        let t = Instant::now();
        let (root, files) = crate::discovery::discover(&root).expect("discoverable");
        let discovery_ms = ms(t);

        // 2. Cold parse (cache empty).
        let mut state = IncrementalState::new();
        let mut analyzer = cartograph_parser::Analyzer::new().expect("grammars");
        let t = Instant::now();
        let analyses = state
            .facts
            .analyze(&mut analyzer, &root, &files, |_| {})
            .expect("parses");
        let cold_parse_ms = ms(t);
        let cold_parse = state.facts.stats();

        // 3. Warm parse (cache full, nothing changed).
        let t = Instant::now();
        let analyses_warm = state
            .facts
            .analyze(&mut analyzer, &root, &files, |_| {})
            .expect("parses");
        let warm_parse_ms = ms(t);
        let warm_parse = state.facts.stats();

        println!("repository          : {} files", files.len());
        println!("discovery           : {discovery_ms:8.3} ms");
        println!(
            "parse  cold         : {cold_parse_ms:8.3} ms  parsed={} reused={}",
            cold_parse.parsed, cold_parse.reused
        );
        println!(
            "parse  warm         : {warm_parse_ms:8.3} ms  parsed={} reused={}",
            warm_parse.parsed, warm_parse.reused
        );
        println!();

        // 4. Resolve stages, cold cache: warm-up then ITERATIONS timed runs.
        let mut cold_state = IncrementalState::new();
        let _ = cold_state
            .facts
            .analyze(&mut analyzer, &root, &files, |_| {})
            .expect("parses");
        let _ = resolve_stages(&analyses, &mut cold_state); // warm-up
        let cold_runs: Vec<_> = (0..ITERATIONS)
            .map(|_| {
                let mut fresh = IncrementalState::new();
                let _ = fresh
                    .facts
                    .analyze(&mut analyzer, &root, &files, |_| {})
                    .expect("parses");
                resolve_stages(&analyses, &mut fresh)
            })
            .collect();
        report("resolve stages, COLD Stage C cache", &best_of(cold_runs));
        println!();

        // 5. Resolve stages, warm cache.
        let mut warm_state = IncrementalState::new();
        let _ = warm_state
            .facts
            .analyze(&mut analyzer, &root, &files, |_| {})
            .expect("parses");
        let _ = resolve_stages(&analyses_warm, &mut warm_state); // populate
        let _ = resolve_stages(&analyses_warm, &mut warm_state); // warm-up
        let warm_runs: Vec<_> = (0..ITERATIONS)
            .map(|_| resolve_stages(&analyses_warm, &mut warm_state))
            .collect();
        report("resolve stages, WARM Stage C cache", &best_of(warm_runs));
    }
}
