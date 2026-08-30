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
    /// An empty cache. The first run through it is a cold run.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

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
        let cache = FactCache::new();
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
        fn incremental(&self, cache: &mut FactCache) -> pipeline::Analysis {
            pipeline::run_with_cache(&self.root, Options { quiet: true }, cache)
                .expect("incremental analysis")
        }

        /// Analysis from nothing — the reference answer.
        fn clean(&self) -> pipeline::Analysis {
            pipeline::run(&self.root, Options { quiet: true }).expect("clean analysis")
        }

        /// The whole point: after a mutation, the two must agree.
        fn assert_incremental_matches_clean(&self, cache: &mut FactCache, context: &str) {
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
        let mut cache = FactCache::new();

        let first = repo.incremental(&mut cache);
        assert_eq!(cache.stats().parsed, 3, "cold run must parse all 3");
        assert_eq!(cache.stats().reused, 0);

        let second = repo.incremental(&mut cache);
        assert_eq!(cache.stats().parsed, 0, "warm run must parse nothing");
        assert_eq!(cache.stats().reused, 3);
        assert_eq!(cache.stats().evicted, 0);

        assert_graphs_equal(&first.graph, &second.graph, "no-op re-run");
        repo.assert_incremental_matches_clean(&mut cache, "no-op");
    }

    /// PART 7 — the first proof. One file changes; only that file is reparsed
    /// and the result still equals a clean build.
    #[test]
    fn one_changed_file_reparses_only_itself() {
        let repo = Repo::new("one-file");
        full_stack(&repo);
        let mut cache = FactCache::new();
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
        assert_eq!(cache.stats().parsed, 1, "only the edited file");
        assert_eq!(cache.stats().reused, 2);
        repo.assert_incremental_matches_clean(&mut cache, "single-file edit");
    }

    /// PART 11 — a new file is analysed; the others are not touched.
    #[test]
    fn a_created_file_is_analysed_and_others_are_reused() {
        let repo = Repo::new("create");
        full_stack(&repo);
        let mut cache = FactCache::new();
        repo.incremental(&mut cache);

        repo.write(
            "app/extra.py",
            "class Invoice:\n\
             \x20   __tablename__ = 'invoices'\n",
        );

        repo.incremental(&mut cache);
        assert_eq!(cache.stats().parsed, 1, "only the new file");
        assert_eq!(cache.stats().reused, 3);
        assert_eq!(cache.len(), 4);
        repo.assert_incremental_matches_clean(&mut cache, "file creation");
    }

    /// PART 12 — a deleted file's facts and every edge derived from them go.
    #[test]
    fn a_deleted_file_leaves_no_trace() {
        let repo = Repo::new("delete");
        full_stack(&repo);
        let mut cache = FactCache::new();
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
        assert_eq!(cache.stats().evicted, 1);
        assert_eq!(cache.len(), 2);
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
        let mut cache = FactCache::new();
        repo.incremental(&mut cache);

        repo.rename("app/models.py", "app/schema.py");

        let after = repo.incremental(&mut cache);
        assert_eq!(cache.stats().evicted, 1, "the old path is dropped");
        assert_eq!(cache.stats().parsed, 1, "the new path is parsed");
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
        let mut cache = FactCache::new();
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
        assert_eq!(cache.stats().parsed, 1, "only the route file");
        // The frontend was not reparsed, yet its edge must be reconsidered.
        repo.assert_incremental_matches_clean(&mut cache, "backend route change");
    }

    /// PART 15 — M05's evaluator under incremental update. `BASE` moves, and
    /// the dependent URL and its match must move with it.
    #[test]
    fn a_dynamic_url_base_change_propagates() {
        let repo = Repo::new("dynamic");
        full_stack(&repo);
        let mut cache = FactCache::new();
        repo.incremental(&mut cache);

        repo.write(
            "web/api.ts",
            "export const BASE = '/v2';\n\
             export async function listOrders() {\n\
             \x20 return fetch(`${BASE}/orders`, { method: 'GET' });\n\
             }\n",
        );

        repo.incremental(&mut cache);
        assert_eq!(cache.stats().parsed, 1);
        repo.assert_incremental_matches_clean(&mut cache, "dynamic URL base change");
    }

    /// PART 16 — an ORM table rename must move the model→table edge.
    #[test]
    fn an_orm_table_change_propagates() {
        let repo = Repo::new("orm");
        full_stack(&repo);
        let mut cache = FactCache::new();
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
        assert_eq!(cache.stats().parsed, 1);
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
        let mut cache = FactCache::new();
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
        let mut cache = FactCache::new();
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
        let mut cache = FactCache::new();
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
        assert_eq!(cache.stats().parsed, 2, "both edits, nothing else");
        assert_eq!(cache.stats().reused, 2);
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
        let mut cache = FactCache::new();
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
        assert_eq!(cache.stats().parsed, 1, "only the importing file changed");
        assert_eq!(cache.stats().reused, 3);
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
        let mut cache = FactCache::new();
        let before = repo.incremental(&mut cache);
        let before_edges = canonical(&before.graph).edges;

        repo.write("web/config_a.ts", "export const ROOT = '/api';\n");

        let after = repo.incremental(&mut cache);
        assert_eq!(cache.stats().parsed, 1, "only the exporting file changed");
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
        let mut cache = FactCache::new();
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
        assert_eq!(cache.stats().parsed, 1);
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
        let mut cache = FactCache::new();
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
        assert_eq!(cache.stats().parsed, 2, "both dependent files");
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
        let mut cache = FactCache::new();
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
        let mut cache = FactCache::new();
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
        let mut cache = FactCache::new();
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
        let mut cache = FactCache::new();
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
        let mut cache = FactCache::new();

        let t = std::time::Instant::now();
        let cold = pipeline::run_with_cache(&root, Options { quiet: true }, &mut cache)
            .expect("cold analysis");
        let cold_ms = t.elapsed().as_secs_f64() * 1000.0;
        let cold_stats = cache.stats();

        let t = std::time::Instant::now();
        let warm = pipeline::run_with_cache(&root, Options { quiet: true }, &mut cache)
            .expect("warm analysis");
        let warm_ms = t.elapsed().as_secs_f64() * 1000.0;
        let warm_stats = cache.stats();

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
        let mut cache = FactCache::new();
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
