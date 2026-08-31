//! The one analysis pipeline every command runs.
//!
//! # Why this module exists
//!
//! Before M09 the pipeline lived inside `match_cmd`. M09 adds two more
//! commands that need the same graph — the default summary and `trace` — and
//! three copies of "extract, compose prefixes, resolve URLs, match, build
//! edges" is three chances for them to disagree. A `trace` that walked a graph
//! built slightly differently from the one `match` reported would be worse
//! than no `trace` at all: the two would contradict each other and neither
//! would be wrong on its face.
//!
//! So the pipeline is here, once, and the commands differ only in how they
//! *render* what it produced. Nothing in this module decides a relationship;
//! every decision is made by `cartograph-resolver` and every edge by
//! `cartograph-graph`. The CLI composes library calls and holds no analysis
//! logic of its own.

use std::io::IsTerminal;
use std::path::Path;

use cartograph_graph::ArchitectureGraph;
use cartograph_parser::Analyzer;
use cartograph_parser::model::{FileAnalysis, ParseStatus, SourceLanguage};
use cartograph_resolver::{
    MatchResult, MatchStatus, ModuleIndex, OrmAnalysis, RouteIndex, RouterIndex,
    add_client_call_edges, add_edge_for_match, add_orm_edges, collect_exported_constants,
    match_client, normalize_client_call, normalize_route_declaration, resolve_url, scope_for_file,
    with_composed_prefix, with_resolved_url,
};
use serde::Serialize;

use crate::discovery;
use crate::error::{CliError, ErrorCode};
use crate::incremental::IncrementalState;

/// Progress is reported once every this many files.
///
/// A fixed count rather than a time interval: a wall-clock trigger would make
/// the stderr stream differ between two runs over the same repository, and
/// "deterministic behaviour" includes the parts a human reads.
const PROGRESS_EVERY: usize = 250;

/// Below this many files an analysis finishes before a progress line would
/// help, so none is printed.
const PROGRESS_THRESHOLD: usize = 500;

/// What was analysed, named without leaking where it lives on this machine.
///
/// The absolute path is deliberately absent from every serialised form. A
/// graph is a set of facts about a repository, and `/Volumes/T7 Shield/...`
/// is a fact about the laptop that ran the analysis (PART 12, RULE 015).
#[derive(Debug, Clone, Serialize)]
pub struct Repository {
    /// The final component of the analysed path — the repository's own name.
    pub name: String,
    /// The path exactly as the user typed it, when that is already relative.
    ///
    /// `.` and `../sibling` are safe to echo back and are what the user
    /// recognises. A rooted path — absolute, or Windows drive-less such as
    /// `\rooted\secret` — is replaced by `<absolute>` rather than reproduced.
    pub requested: String,
}

/// The analysed repository's own name.
///
/// Deliberately not the path: an absolute path names this machine, and a
/// serialised graph is a set of facts about a repository (PART 12, RULE 015).
/// Shared by every command so they cannot name the same repository
/// differently.
pub fn repository_name(requested: &Path, root: &Path) -> String {
    root.canonicalize()
        .ok()
        .as_deref()
        .and_then(Path::file_name)
        .or_else(|| root.file_name())
        .or_else(|| requested.file_name())
        .and_then(|n| n.to_str())
        .unwrap_or("repository")
        .to_owned()
}

impl Repository {
    /// Derives the repository identity from the requested path.
    ///
    /// Public so every command names a repository the same way. The schema
    /// conformance tests caught `parse` emitting a bare string here while the
    /// summary emitted an object — two shapes for one field is how a contract
    /// quietly breaks.
    pub fn describe(requested: &Path, root: &Path) -> Self {
        let name = repository_name(requested, root);

        let requested = if discovery::is_rooted(requested) {
            // Never reproduced: a rooted path names this machine. Rooted
            // rather than absolute because a Windows `\rooted\secret` is not
            // `is_absolute()` — see `discovery::is_rooted`.
            "<absolute>".to_owned()
        } else {
            requested.to_string_lossy().replace('\\', "/")
        };

        Self { name, requested }
    }
}

/// Counts describing what a run found.
///
/// Field names use the vocabulary the summary prints, and that vocabulary is
/// chosen carefully: `observed` for what the source says, `resolved` for what
/// the resolver determined, `refused` for what it declined to claim. Nothing
/// here is called "verified".
#[derive(Debug, Default, Clone, Serialize)]
pub struct Totals {
    /// Files opened and parsed.
    pub files: usize,
    /// Files parsed with no diagnostics at all.
    pub files_clean: usize,
    /// Files that produced at least one diagnostic, whether or not parsing
    /// then failed. Counted independently of the status buckets: a file can
    /// both fail and explain why, and reporting "4 diagnostics across 2 files"
    /// when four files each produced one is simply false.
    pub files_with_diagnostics: usize,
    /// Files that could not be parsed. Analysis continued without them.
    pub files_failed: usize,
    /// Declared symbols across every parsed file.
    pub symbols: usize,
    /// Route declarations observed in source.
    pub routes: usize,
    /// Routes whose served path was composed from enclosing router prefixes.
    pub routes_composed: usize,
    /// Syntactically HTTP-looking call sites observed in source.
    pub http_observations: usize,
    /// Client observations matched exactly.
    pub exact: usize,
    /// Client observations matched strongly.
    pub strong: usize,
    /// Client observations with rival candidates and therefore no edge.
    pub ambiguous: usize,
    /// Client observations with no candidate route at all.
    pub no_match: usize,
    /// Client observations outside the supported subset.
    pub unsupported: usize,
    /// Dynamic URLs the evaluator fully determined.
    pub dynamic_resolved: usize,
    /// Dynamic URLs partly determined, with the gaps kept explicit.
    pub dynamic_partial: usize,
    /// ORM models discovered.
    pub orm_models: usize,
    /// ORM models that resolved to a table.
    pub orm_tables: usize,
    /// Model names claimed by more than one class, which resolve to nothing.
    pub orm_ambiguous: usize,
    /// Nodes in the produced graph.
    pub nodes: usize,
    /// Edges in the produced graph.
    pub edges: usize,
    /// `HttpCall` edges, the HTTP boundary crossings.
    pub http_call_edges: usize,
    /// `Call` edges into a function that issues a request.
    pub client_call_edges: usize,
    /// `OrmAccess` and `Queries` edges.
    pub orm_edges: usize,
    /// Edges whose source and target are in different languages.
    pub cross_language_edges: usize,
    /// Parse diagnostics across every file.
    pub diagnostics: usize,
}

impl Totals {
    /// Observations the resolver declined to turn into an edge.
    ///
    /// Named "refused" rather than "failed" because declining is the designed
    /// behaviour: an ambiguous or unsupported observation produces no edge on
    /// purpose, and counting it as a failure would invert the product's
    /// priorities.
    #[must_use]
    pub fn refused(&self) -> usize {
        self.ambiguous + self.no_match + self.unsupported
    }
}

/// One parse problem, with the file it belongs to.
#[derive(Debug, Clone, Serialize)]
pub struct FileDiagnostic {
    /// Repository-relative path.
    pub file: String,
    /// `error` or `warning`.
    pub severity: String,
    /// Structural classification.
    pub kind: String,
    /// Structural description. Never source text (RULE 015).
    pub message: String,
    /// Line, when the problem has one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line: Option<u32>,
}

/// Languages seen, and how many files of each.
#[derive(Debug, Default, Clone, Serialize)]
pub struct Languages {
    /// `.ts`, `.mts`, `.cts`.
    pub typescript: usize,
    /// `.tsx`.
    pub tsx: usize,
    /// `.py`, `.pyi`.
    pub python: usize,
}

impl Languages {
    /// The languages actually present, for display.
    #[must_use]
    pub fn present(&self) -> Vec<String> {
        let mut names = Vec::new();
        if self.typescript > 0 {
            names.push(format!("TypeScript ({})", self.typescript));
        }
        if self.tsx > 0 {
            names.push(format!("TSX ({})", self.tsx));
        }
        if self.python > 0 {
            names.push(format!("Python ({})", self.python));
        }
        names
    }
}

/// Everything one run produced.
pub struct Analysis {
    /// What was analysed.
    pub repository: Repository,
    /// Counts.
    pub totals: Totals,
    /// Files per language.
    pub languages: Languages,
    /// Every match decision, including the refusals.
    pub results: Vec<MatchResult>,
    /// The ORM layer.
    pub orm: OrmAnalysis,
    /// The graph itself. The deliverable; the decisions above are the
    /// reasoning that produced it.
    pub graph: ArchitectureGraph,
    /// Parse problems, in file order.
    pub diagnostics: Vec<FileDiagnostic>,
    /// Wall-clock time for extraction, resolution and graph construction.
    pub elapsed: std::time::Duration,
}

impl Analysis {
    /// Whether any file failed to parse.
    ///
    /// This is what `--strict` turns into a non-zero exit: a file the analyser
    /// could not read at all means the graph is missing whatever that file
    /// contained, and a caller may reasonably want to know.
    #[must_use]
    pub fn is_partial(&self) -> bool {
        self.totals.files_failed > 0
    }
}

/// How a run should behave while it works.
#[derive(Debug, Clone, Copy)]
pub struct Options {
    /// Suppress progress output. Set for `--json`, where stderr noise is
    /// unwanted and stdout must stay pure.
    pub quiet: bool,
}

/// Runs the full pipeline over `path`.
///
/// # Errors
///
/// [`ErrorCode::InvalidPath`] when the path cannot be used,
/// [`ErrorCode::NoSupportedSources`] when it holds nothing to analyse, and
/// [`ErrorCode::AnalysisFailed`] when extraction itself breaks.
pub fn run(path: &Path, options: Options) -> Result<Analysis, CliError> {
    // A clean run is an incremental run against an empty cache. Deliberately
    // the same code path: two implementations of "analyse a repository" is how
    // `incremental == clean` would quietly stop being true (M10).
    run_with_cache(path, options, &mut IncrementalState::new())
}

/// Runs the pipeline, reusing per-file facts that are still valid.
///
/// Identical to [`run`] except that unchanged files are not reparsed. Every
/// derived fact — imports, routes, matches, ORM relationships, edges — is
/// still recomputed from the full analyses set, so the result cannot differ
/// from a clean run; see `docs/development/incremental-engine.md`.
///
/// # Errors
///
/// The same conditions as [`run`].
pub fn run_with_cache(
    path: &Path,
    options: Options,
    state: &mut IncrementalState,
) -> Result<Analysis, CliError> {
    let (root, files) = discovery::discover(path)?;
    let repository = Repository::describe(path, &root);

    if files.is_empty() {
        return Err(CliError::new(
            ErrorCode::NoSupportedSources,
            format!(
                "no TypeScript, TSX or Python source files were found under `{}`",
                repository.requested
            ),
        )
        .with_hint(
            "Cartograph analyses .ts, .tsx, .mts, .cts, .py and .pyi files. \
             Vendored and generated trees (node_modules, dist, build, target, \
             __pycache__, .venv, migrations) are skipped by design.",
        ));
    }

    let mut analyzer = Analyzer::new().map_err(|error| {
        CliError::new(
            ErrorCode::AnalysisFailed,
            "the TypeScript, TSX and Python grammars could not be loaded",
        )
        .with_hint(format!("underlying cause: {error}"))
    })?;

    let started = std::time::Instant::now();
    let progress =
        !options.quiet && files.len() >= PROGRESS_THRESHOLD && std::io::stderr().is_terminal();

    // Every file is analysed before any URL is resolved: dynamic resolution
    // needs the whole project's exported constants to follow an import.
    //
    // A per-file problem is a diagnostic on the FileAnalysis, not a reason to
    // abandon the repository; only an unexpected analyser fault errors here.
    let total = files.len();
    let analyses = state
        .facts
        .analyze(&mut analyzer, &root, &files, |index| {
            if progress && index > 0 && index % PROGRESS_EVERY == 0 {
                eprintln!("  analysing… {index}/{total} files");
            }
        })
        .map_err(|error| {
            CliError::new(
                ErrorCode::AnalysisFailed,
                "analysis failed while reading a file",
            )
            .with_hint(format!("underlying cause: {error}"))
        })?;
    if progress {
        eprintln!("  analysing… {total}/{total} files");
    }

    // M10 instrumentation. Counts only — never a file's contents, and never a
    // path beyond the repository-relative ones already in the fact model. Not
    // a CLI contract: this is behind `-v` for correctness and performance work
    // on the cache, and nothing parses it.
    let parse_stats = state.facts.stats();
    tracing::debug!(
        files_seen = parse_stats.seen,
        files_reused = parse_stats.reused,
        files_parsed = parse_stats.parsed,
        entries_evicted = parse_stats.evicted,
        cached_entries = state.facts.len(),
        "parse cache"
    );

    let (graph, results, orm, mut totals) = resolve(&analyses, Some(state))?;
    let (languages, diagnostics) = tally_files(&analyses, &mut totals);
    finish_totals(&mut totals, &graph, &analyses);

    Ok(Analysis {
        repository,
        totals,
        languages,
        results,
        orm,
        graph,
        diagnostics,
        elapsed: started.elapsed(),
    })
}

/// Resolution and graph construction — the part that decides nothing itself.
fn resolve(
    analyses: &[FileAnalysis],
    state: Option<&mut IncrementalState>,
) -> Result<(ArchitectureGraph, Vec<MatchResult>, OrmAnalysis, Totals), CliError> {
    let mut totals = Totals::default();
    let exported = collect_exported_constants(analyses);
    // A route's served path is not the path its decorator states: the router
    // it is registered on may carry a prefix, and that router may itself be
    // included in another.
    let modules = ModuleIndex::build(analyses);
    let routers = RouterIndex::build(analyses, &modules);

    let mut backend = Vec::new();
    let mut clients = Vec::new();

    for analysis in analyses {
        let scope = scope_for_file(analysis, &exported);

        for route in &analysis.routes {
            // An unresolved or ambiguous prefix leaves the route exactly as
            // declared, so composition can only ever make a path more complete
            // — never replace a known path with a guessed one.
            let composed = route
                .receiver
                .as_ref()
                .and_then(|r| routers.prefix_for(&analysis.path, r))
                .and_then(|resolution| with_composed_prefix(route, resolution));
            if composed.is_some() {
                totals.routes_composed += 1;
            }
            backend.push(normalize_route_declaration(
                composed.as_ref().unwrap_or(route),
                &analysis.path,
                analysis.language,
            ));
        }

        for call in &analysis.http_calls {
            let resolved = resolve_url(call, analysis, &scope);
            if let Some(r) = &resolved {
                if r.is_fully_known() {
                    totals.dynamic_resolved += 1;
                } else {
                    totals.dynamic_partial += 1;
                }
            }
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

    let index = RouteIndex::build(backend);
    let results: Vec<MatchResult> = clients.iter().map(|c| match_client(c, &index)).collect();

    // Edges are built only from accepted matches; the graph is the product of
    // the decisions, never an input to them.
    let mut graph = ArchitectureGraph::new();
    for result in &results {
        match result.status {
            MatchStatus::Exact => totals.exact += 1,
            MatchStatus::Strong => totals.strong += 1,
            MatchStatus::Ambiguous => totals.ambiguous += 1,
            MatchStatus::NoMatch => totals.no_match += 1,
            MatchStatus::Unsupported => totals.unsupported += 1,
            _ => {}
        }
        if add_edge_for_match(&mut graph, result, None)
            .map_err(|e| graph_failure(&e))?
            .is_some()
        {
            totals.http_call_edges += 1;
        }
    }

    // The client wrapper stays on the path: a component calls a generated
    // client function, and that function issues the request (ADR-0012).
    totals.client_call_edges = add_client_call_edges(&mut graph, analyses, &modules, None)
        .map_err(|e| graph_failure(&e))?;

    // Stage C is the only resolution stage with a proven dependency identity,
    // so it is the only one that reads a cache. Everything else in this
    // function is still recomputed in full.
    let orm = match state {
        Some(state) => {
            let identities = state.facts.identities();
            // A failed Stage C update publishes nothing, and the whole run
            // fails with it. Returning the previous generation here would
            // present an analysis of the *old* repository state as the answer
            // for the new one — the precise failure this path exists to
            // prevent.
            OrmAnalysis::build_cached(analyses, &mut state.accesses, &identities).map_err(
                |error| {
                    CliError::new(
                        ErrorCode::AnalysisFailed,
                        "the incremental update failed and was abandoned",
                    )
                    .with_hint(format!(
                        "underlying cause: {error}; the previous analysis was left                          untouched and no result is reported for the current state"
                    ))
                },
            )?
        }
        None => OrmAnalysis::build(analyses),
    };
    totals.orm_models = orm.models.len();
    totals.orm_tables = orm
        .models
        .values()
        .filter(|m| m.table.name().is_some())
        .count();
    totals.orm_ambiguous = orm.ambiguous.len();
    totals.orm_edges = add_orm_edges(&mut graph, &orm, None).map_err(|e| graph_failure(&e))?;

    Ok((graph, results, orm, totals))
}

fn graph_failure(error: &cartograph_graph::GraphError) -> CliError {
    CliError::new(
        ErrorCode::AnalysisFailed,
        "an edge could not be added to the graph",
    )
    .with_hint(format!("underlying cause: {error}"))
}

/// Per-file counts, languages and diagnostics.
fn tally_files(analyses: &[FileAnalysis], totals: &mut Totals) -> (Languages, Vec<FileDiagnostic>) {
    let mut languages = Languages::default();
    let mut diagnostics = Vec::new();

    for analysis in analyses {
        totals.files += 1;
        totals.symbols += analysis.symbols.len();
        totals.routes += analysis.routes.len();
        totals.http_observations += analysis.http_calls.len();

        match analysis.language {
            SourceLanguage::TypeScript => languages.typescript += 1,
            SourceLanguage::Tsx => languages.tsx += 1,
            SourceLanguage::Python => languages.python += 1,
            // SourceLanguage is non-exhaustive: a language added later is
            // still counted as a file, it simply has no column of its own yet.
            _ => {}
        }

        match analysis.status {
            ParseStatus::Failed => totals.files_failed += 1,
            _ if analysis.diagnostics.is_empty() => totals.files_clean += 1,
            _ => {}
        }
        if !analysis.diagnostics.is_empty() {
            totals.files_with_diagnostics += 1;
        }

        for diagnostic in &analysis.diagnostics {
            totals.diagnostics += 1;
            diagnostics.push(FileDiagnostic {
                file: analysis.path.clone(),
                severity: format!("{:?}", diagnostic.severity).to_lowercase(),
                kind: format!("{:?}", diagnostic.kind),
                message: diagnostic.message.clone(),
                line: diagnostic.span.map(|s| s.start_line),
            });
        }
    }

    (languages, diagnostics)
}

/// Graph-shaped totals, computed from the graph rather than from the
/// bookkeeping that built it — so a discrepancy between the two would show.
fn finish_totals(totals: &mut Totals, graph: &ArchitectureGraph, analyses: &[FileAnalysis]) {
    totals.nodes = graph.node_count();
    totals.edges = graph.edge_count();

    let language_of: std::collections::HashMap<&str, SourceLanguage> = analyses
        .iter()
        .map(|a| (a.path.as_str(), a.language))
        .collect();

    // "Cross-language" means the two endpoints were extracted by different
    // grammars — TypeScript calling Python, which is the relationship the
    // product exists to recover. TypeScript and TSX are one language here.
    let family = |language: SourceLanguage| match language {
        SourceLanguage::TypeScript | SourceLanguage::Tsx => "ts",
        SourceLanguage::Python => "py",
        // A language added later is its own family until it is named here,
        // which keeps "cross-language" true rather than convenient.
        _ => "other",
    };

    totals.cross_language_edges = graph
        .edges()
        .filter(|edge| {
            let source = graph
                .node(edge.source())
                .and_then(cartograph_core::Node::location);
            let target = graph
                .node(edge.target())
                .and_then(cartograph_core::Node::location);
            match (source, target) {
                (Some(s), Some(t)) => {
                    let s = language_of.get(s.file()).copied().map(family);
                    let t = language_of.get(t.file()).copied().map(family);
                    matches!((s, t), (Some(a), Some(b)) if a != b)
                }
                _ => false,
            }
        })
        .count();
}

/// Layout over graphs the real analyser produced, not over fixtures.
///
/// A fixture graph is built by the test that then checks it, so it can only
/// contain shapes the author thought of. A graph the analyser produced from a
/// real tree contains what is actually there: files at the repository root,
/// artefacts with no source location, directories holding one node and
/// directories holding many, and names nobody would have invented.
///
/// The default subject is **this repository**, which is always present, so
/// these run everywhere rather than being skipped when a corpus is missing.
/// `CARTOGRAPH_LAYOUT_REPO` points the second case at another tree; the pinned
/// benchmark corpora are the interesting subject and are opt-in because this
/// repository does not vendor them.
///
/// Nothing here mutates the tree it analyses — the analyser only reads.
#[cfg(test)]
mod layout_real_repository {
    use std::path::{Path, PathBuf};

    use cartograph_graph::layout::{LayoutParams, layout};

    use crate::pipeline::{self, Options};

    /// The repository this crate lives in.
    fn own_repository() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(Path::parent)
            .expect("the crate sits two levels below the repository root")
            .to_path_buf()
    }

    /// Analyses `root`, lays the graph out, and asserts what must hold for
    /// every real graph.
    fn check(root: &Path, label: &str) {
        let analysis = pipeline::run(root, Options { quiet: true }).expect("the tree analyses");
        let graph = &analysis.graph;

        assert!(
            graph.node_count() > 0,
            "precondition: {label} must produce a non-empty graph, or this proves nothing"
        );

        let params = LayoutParams::default();
        let result = layout(graph, &params);

        // Totality: one record per node, no duplicates, no strangers.
        assert_eq!(
            result.node_count(),
            graph.node_count(),
            "{label}: every node must receive exactly one layout record"
        );
        let mut seen = std::collections::BTreeSet::new();
        for node in &result.nodes {
            assert!(seen.insert(node.id), "{label}: {} laid out twice", node.id);
            assert!(
                graph.node(node.id).is_some(),
                "{label}: {} is not in the source graph",
                node.id
            );
        }
        for node in graph.nodes() {
            assert!(
                seen.contains(&node.id()),
                "{label}: {} received no layout record",
                node.id()
            );
        }

        // Well-formedness: finite, bounded, every cluster resolvable.
        for node in &result.nodes {
            assert!(
                node.x.is_finite() && node.y.is_finite(),
                "{label}: {} is at ({}, {})",
                node.id,
                node.x,
                node.y
            );
            assert!(
                node.x.abs() <= params.extent + 1e-9 && node.y.abs() <= params.extent + 1e-9,
                "{label}: {} escaped the extent at ({}, {})",
                node.id,
                node.x,
                node.y
            );
            assert!(
                result.clusters.iter().any(|c| c.id == node.cluster),
                "{label}: {} names a cluster outside the table",
                node.id
            );
        }

        // Every cluster is named, and every cluster is used.
        for cluster in &result.clusters {
            assert!(
                !cluster.label.is_empty(),
                "{label}: cluster {} has no name",
                cluster.id
            );
            assert!(
                result.nodes.iter().any(|n| n.cluster == cluster.id),
                "{label}: cluster {} ({}) has no members",
                cluster.id,
                cluster.label
            );
        }

        // Determinism over one graph.
        assert_eq!(
            result,
            layout(graph, &params),
            "{label}: two layouts of one graph must be identical"
        );

        // Determinism across a *second independent analysis* of the same tree.
        // The two graphs allocate their handles independently, so this is what
        // shows the geometry follows the content rather than the numbering.
        let reanalysed = pipeline::run(root, Options { quiet: true }).expect("analyses twice");
        let second = layout(&reanalysed.graph, &params);
        let first_points: Vec<(f64, f64)> = result.nodes.iter().map(|n| (n.x, n.y)).collect();
        let second_points: Vec<(f64, f64)> = second.nodes.iter().map(|n| (n.x, n.y)).collect();
        assert_eq!(
            result.clusters, second.clusters,
            "{label}: an independent analysis must produce the same clusters"
        );
        assert_eq!(
            first_points, second_points,
            "{label}: an independent analysis must produce the same positions"
        );

        println!(
            "{label}: {} nodes, {} edges -> {} positioned in {} clusters",
            graph.node_count(),
            graph.edge_count(),
            result.node_count(),
            result.cluster_count()
        );
    }

    #[test]
    fn the_cartograph_repository_lays_out() {
        check(&own_repository(), "cartograph");
    }

    #[test]
    #[ignore = "needs CARTOGRAPH_LAYOUT_REPO; the pinned corpora are not vendored here"]
    fn a_pinned_corpus_lays_out() {
        let Ok(path) = std::env::var("CARTOGRAPH_LAYOUT_REPO") else {
            return;
        };
        check(Path::new(&path), "corpus");
    }
}
