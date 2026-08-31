//! What a resolution read, and — just as important — what it did not.
//!
//! A dependency set that quietly contains every file in the repository would
//! pass every "did we record enough?" test and be useless, because a cache
//! keyed on it would never hit. So each case here asserts both directions: the
//! files that can change the answer are present, and the files that cannot are
//! absent.
//!
//! These tests describe the recording mechanism, not the resolution result.
//! The results themselves are covered by `imports.rs` and are unchanged.

use cartograph_parser::Analyzer;
use cartograph_parser::model::FileAnalysis;
use cartograph_resolver::{ModuleIndex, Resolution, ResolutionContext};

fn analyze(files: &[(&str, &str)]) -> Vec<FileAnalysis> {
    let mut a = Analyzer::new().expect("grammars load");
    files
        .iter()
        .map(|(p, s)| a.analyze_source(p, s).expect("fixture parses"))
        .collect()
}

/// Resolves `name` from `from`, returning the answer and what it read.
fn resolve_tracked(
    files: &[(&str, &str)],
    from: &str,
    name: &str,
) -> (Resolution, ResolutionContext) {
    let analyses = analyze(files);
    let index = ModuleIndex::build(&analyses);
    let file = index.file(from).expect("the using file was analysed");
    let mut deps = ResolutionContext::new();
    let resolution = index.resolve_python_tracked(file, name, &mut deps);
    (resolution, deps)
}

fn read_set(deps: &ResolutionContext) -> Vec<&str> {
    deps.files().collect()
}

/// A three-file re-export chain: A imports from B, B re-exports from C, and C
/// holds the declaration. Every hop can change the answer, so every hop must
/// be recorded — and D, which nothing touches, must not be.
#[test]
fn a_transitive_re_export_chain_records_every_hop_and_nothing_else() {
    let files = &[
        (
            "pkg/a.py",
            "from .b import Order\ndef use():\n    return Order()\n",
        ),
        ("pkg/b.py", "from .c import Order\n"),
        ("pkg/c.py", "class Order:\n    __tablename__ = 'orders'\n"),
        ("pkg/d.py", "class Unrelated:\n    pass\n"),
    ];
    let (resolution, deps) = resolve_tracked(files, "pkg/a.py", "Order");

    // Precondition: the chain must actually resolve, or the read set below
    // would describe a failed lookup rather than a followed chain.
    assert!(
        matches!(&resolution, Resolution::Declared { file, .. } if file == "pkg/c.py"),
        "the fixture must resolve through to c.py, got {resolution:?}"
    );

    for hop in ["pkg/a.py", "pkg/b.py", "pkg/c.py"] {
        assert!(
            deps.reads(hop),
            "{hop} can change the answer and must be recorded: {:?}",
            read_set(&deps)
        );
    }
    assert!(
        !deps.reads("pkg/d.py"),
        "an untouched file must stay out of the read set: {:?}",
        read_set(&deps)
    );
    assert!(deps.is_complete());
}

/// The negative direction, stated on its own because it is the property that
/// makes the read set worth recording at all.
#[test]
fn unrelated_python_and_typescript_files_stay_out_of_the_read_set() {
    let files = &[
        (
            "app/handlers.py",
            "from .models import Order\ndef make():\n    return Order()\n",
        ),
        (
            "app/models.py",
            "class Order:\n    __tablename__ = 'orders'\n",
        ),
        ("app/other.py", "VALUE = 1\n"),
        ("web/thing.ts", "export const X = 1;\n"),
    ];
    let (_, deps) = resolve_tracked(files, "app/handlers.py", "Order");

    assert!(deps.reads("app/handlers.py"));
    assert!(deps.reads("app/models.py"));
    assert!(
        !deps.reads("app/other.py"),
        "read set: {:?}",
        read_set(&deps)
    );
    assert!(
        !deps.reads("web/thing.ts"),
        "read set: {:?}",
        read_set(&deps)
    );
    assert_eq!(deps.file_count(), 2, "read set: {:?}", read_set(&deps));
}

/// A name declared in the asking file never leaves it, so the read set is that
/// file alone and the repository's path set was never consulted.
#[test]
fn a_local_declaration_reads_only_its_own_file() {
    let files = &[
        (
            "app/models.py",
            "class Order:\n    pass\ndef make():\n    return Order()\n",
        ),
        ("app/other.py", "VALUE = 1\n"),
    ];
    let (resolution, deps) = resolve_tracked(files, "app/models.py", "Order");

    assert!(matches!(resolution, Resolution::Local { .. }));
    assert_eq!(read_set(&deps), vec!["app/models.py"]);
    assert!(
        !deps.consults_path_set(),
        "a local name needs no module lookup, so the path set is irrelevant"
    );
}

/// Following an import consults the repository's **path set** — which files
/// exist — separately from any file's contents.
#[test]
fn following_an_import_records_the_path_set_dependency() {
    let files = &[
        (
            "app/handlers.py",
            "from .models import Order\ndef make():\n    return Order()\n",
        ),
        ("app/models.py", "class Order:\n    pass\n"),
    ];
    let (_, deps) = resolve_tracked(files, "app/handlers.py", "Order");
    assert!(
        deps.consults_path_set(),
        "resolving a module scans repository paths and must say so"
    );
}

/// The case content hashing cannot see: a second file whose **path** collides
/// makes the import ambiguous, and the resolution changes even though no
/// recorded file's contents changed. The path-set flag is what a cache would
/// have to consult to notice.
#[test]
fn a_colliding_path_changes_the_result_with_no_content_change() {
    let without = &[
        (
            "app/handlers.py",
            "from .models import Order\ndef make():\n    return Order()\n",
        ),
        ("app/models.py", "class Order:\n    pass\n"),
    ];
    let (before, deps_before) = resolve_tracked(without, "app/handlers.py", "Order");
    assert!(
        matches!(&before, Resolution::Declared { file, .. } if file == "app/models.py"),
        "precondition: it must resolve first, got {before:?}"
    );
    assert!(deps_before.consults_path_set());

    // Identical contents for handlers.py and models.py; only a path is added,
    // and its contents are inert.
    let with = &[
        (
            "app/handlers.py",
            "from .models import Order\ndef make():\n    return Order()\n",
        ),
        ("app/models.py", "class Order:\n    pass\n"),
        ("vendor/app/models.py", "VERSION = 1\n"),
    ];
    let (after, _) = resolve_tracked(with, "app/handlers.py", "Order");
    assert!(
        matches!(after, Resolution::Unresolved { .. }),
        "two candidate paths must resolve to nothing, got {after:?}"
    );
}

/// A missing target still depends on the path set: creating the module later
/// is exactly what would change the answer.
#[test]
fn an_unresolved_import_still_records_the_path_set() {
    let files = &[(
        "app/handlers.py",
        "from .nowhere import Order\ndef make():\n    return Order()\n",
    )];
    let (resolution, deps) = resolve_tracked(files, "app/handlers.py", "Order");
    assert!(matches!(resolution, Resolution::Unresolved { .. }));
    assert!(
        deps.consults_path_set(),
        "creating the missing module would change this answer"
    );
    assert!(deps.reads("app/handlers.py"));
}

/// A name that is never imported is answered from the asking file alone, with
/// no module lookup at all.
#[test]
fn an_unbound_name_reads_only_the_asking_file() {
    let files = &[
        ("app/handlers.py", "def make():\n    return Order()\n"),
        ("app/models.py", "class Order:\n    pass\n"),
    ];
    let (resolution, deps) = resolve_tracked(files, "app/handlers.py", "Order");
    assert!(matches!(resolution, Resolution::NotBound));
    assert_eq!(read_set(&deps), vec!["app/handlers.py"]);
    assert!(
        !deps.reads("app/models.py"),
        "a name that was never imported must not depend on the module that happens to declare it"
    );
}

/// A cycle must terminate and still report a bounded, honest read set rather
/// than looping or silently dropping hops.
#[test]
fn a_cyclic_import_terminates_and_records_the_files_it_walked() {
    let files = &[
        ("pkg/a.py", "from .b import Order\n"),
        ("pkg/b.py", "from .a import Order\n"),
    ];
    let (resolution, deps) = resolve_tracked(files, "pkg/a.py", "Order");

    assert!(
        matches!(resolution, Resolution::Unresolved { .. }),
        "a cycle cannot produce a declaration, got {resolution:?}"
    );
    assert!(deps.reads("pkg/a.py"));
    assert!(deps.reads("pkg/b.py"));
    assert!(deps.is_complete(), "termination is not incompleteness");
}

/// Beyond the hop limit the resolver gives up. The answer still depends on
/// every file it walked getting there, so they must all be recorded.
#[test]
fn a_chain_past_the_hop_limit_records_what_it_walked() {
    let files = &[
        ("p/a.py", "from .b import Order\n"),
        ("p/b.py", "from .c import Order\n"),
        ("p/c.py", "from .d import Order\n"),
        ("p/d.py", "from .e import Order\n"),
        ("p/e.py", "from .f import Order\n"),
        ("p/f.py", "class Order:\n    pass\n"),
        ("p/unrelated.py", "X = 1\n"),
    ];
    let (_, deps) = resolve_tracked(files, "p/a.py", "Order");

    for walked in ["p/a.py", "p/b.py", "p/c.py"] {
        assert!(deps.reads(walked), "read set: {:?}", read_set(&deps));
    }
    assert!(
        !deps.reads("p/unrelated.py"),
        "read set: {:?}",
        read_set(&deps)
    );
}

/// Two runs over the same repository must produce byte-identical dependency
/// sets, or a cache key derived from them would be unstable.
#[test]
fn the_read_set_is_deterministic_across_runs() {
    let files = &[
        ("pkg/a.py", "from .b import Order\n"),
        ("pkg/b.py", "from .c import Order\n"),
        ("pkg/c.py", "class Order:\n    pass\n"),
    ];
    let (_, first) = resolve_tracked(files, "pkg/a.py", "Order");
    let (_, second) = resolve_tracked(files, "pkg/a.py", "Order");
    assert_eq!(read_set(&first), read_set(&second));
    assert_eq!(first, second);
}

/// The untracked entry point must answer exactly what the tracked one does.
/// Tracking is an observation, never a behaviour change (M09 semantics).
#[test]
fn tracking_does_not_change_the_resolution() {
    let cases: &[&[(&str, &str)]] = &[
        &[
            ("pkg/a.py", "from .b import Order\n"),
            ("pkg/b.py", "from .c import Order\n"),
            ("pkg/c.py", "class Order:\n    pass\n"),
        ],
        &[(
            "solo.py",
            "class Order:\n    pass\ndef m():\n    return Order()\n",
        )],
        &[("miss.py", "from .gone import Order\n")],
        &[
            ("app/handlers.py", "from .models import Order\n"),
            ("app/models.py", "class Order:\n    pass\n"),
            ("vendor/app/models.py", "VERSION = 1\n"),
        ],
    ];
    for files in cases {
        let analyses = analyze(files);
        let index = ModuleIndex::build(&analyses);
        let from = index.file(files[0].0).expect("analysed");
        let untracked = index.resolve_python(from, "Order");
        let mut deps = ResolutionContext::new();
        let tracked = index.resolve_python_tracked(from, "Order", &mut deps);
        assert_eq!(
            format!("{untracked:?}"),
            format!("{tracked:?}"),
            "tracking changed the answer for {:?}",
            files[0].0
        );
    }
}

/// Walks the Python sources of a repository, for the inspection below.
fn walk(root: &std::path::Path, dir: &std::path::Path, out: &mut Vec<String>) {
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
            walk(root, &path, out);
        } else if std::path::Path::new(name)
            .extension()
            .is_some_and(|e| e.eq_ignore_ascii_case("py"))
        {
            if let Ok(rel) = path.strip_prefix(root) {
                out.push(rel.to_string_lossy().replace('\\', "/"));
            }
        }
    }
}

/// Read sets on a real repository, inspected rather than asserted.
///
/// Tests passing on hand-written fixtures does not establish that the recorded
/// dependencies are plausible at scale — a mechanism that silently recorded
/// every file would pass all of them. This prints the distribution and a few
/// representative resolutions so they can be read by eye.
///
/// Opt-in; ignored by default. It lives in `tests/` rather than `src/` because
/// QG-008 forbids the resolver's sources from reading the environment.
#[test]
#[ignore = "needs CARTOGRAPH_READSET_REPO; inspects rather than asserts"]
fn read_sets_on_a_real_repository() {
    let Ok(repo) = std::env::var("CARTOGRAPH_READSET_REPO") else {
        return;
    };
    let root = std::path::PathBuf::from(repo);

    let mut paths = Vec::new();
    walk(&root, &root, &mut paths);
    paths.sort();

    let mut analyzer = Analyzer::new().expect("grammars load");
    let analyses: Vec<FileAnalysis> = paths
        .iter()
        .filter_map(|rel| analyzer.analyze_file(&root, rel).ok())
        .collect();
    let index = ModuleIndex::build(&analyses);

    let mut histogram: std::collections::BTreeMap<usize, usize> = std::collections::BTreeMap::new();
    let mut path_set_uses = 0usize;
    let mut queries = 0usize;
    let mut samples: Vec<String> = Vec::new();

    for file in &analyses {
        for import in &file.imports {
            for named in &import.named {
                let name = named.local.as_deref().unwrap_or(named.imported.as_str());
                let mut deps = ResolutionContext::new();
                let resolution = index.resolve_python_tracked(file, name, &mut deps);
                queries += 1;
                *histogram.entry(deps.file_count()).or_default() += 1;
                if deps.consults_path_set() {
                    path_set_uses += 1;
                }
                if samples.len() < 6 && deps.file_count() >= 3 {
                    samples.push(format!(
                        "  {} :: {name} -> {:?}\n     reads {:?} path_set={}",
                        file.path,
                        match &resolution {
                            Resolution::Declared { file, .. } => format!("Declared({file})"),
                            Resolution::Local { .. } => "Local".to_owned(),
                            Resolution::NotBound => "NotBound".to_owned(),
                            Resolution::Unresolved { reason, .. } =>
                                format!("Unresolved({reason})"),
                            Resolution::Ambiguous { .. } => "Ambiguous".to_owned(),
                        },
                        deps.files().collect::<Vec<_>>(),
                        deps.consults_path_set()
                    ));
                }
            }
        }
    }

    println!("python files       : {}", analyses.len());
    println!("resolution queries : {queries}");
    println!("consulted path set : {path_set_uses}");
    println!("read-set size histogram (files read -> queries):");
    for (size, count) in &histogram {
        println!("  {size:3} -> {count}");
    }
    println!("representative resolutions:");
    for s in &samples {
        println!("{s}");
    }

    // The one property asserted: a read set must never be the whole repository.
    let largest = histogram.keys().next_back().copied().unwrap_or(0);
    assert!(
        largest < analyses.len(),
        "a read set covering every file would make any cache useless: {largest} of {}",
        analyses.len()
    );
}

// ── TypeScript resolution ───────────────────────────────────────────────

fn resolve_ts_tracked(
    files: &[(&str, &str)],
    from: &str,
    name: &str,
) -> (Resolution, ResolutionContext) {
    let analyses = analyze(files);
    let index = ModuleIndex::build(&analyses);
    let file = index.file(from).expect("the using file was analysed");
    let mut deps = ResolutionContext::new();
    let resolution = index.resolve_typescript_tracked(file, name, &mut deps);
    (resolution, deps)
}

/// TypeScript A -> B -> C, mirroring the Python chain test.
#[test]
fn a_typescript_re_export_chain_records_every_hop_and_nothing_else() {
    let files = &[
        (
            "web/a.ts",
            "import { Order } from './b';\nexport const use = () => Order;\n",
        ),
        ("web/b.ts", "export { Order } from './c';\n"),
        ("web/c.ts", "export const Order = 1;\n"),
        ("web/d.ts", "export const Unrelated = 2;\n"),
    ];
    let (resolution, deps) = resolve_ts_tracked(files, "web/a.ts", "Order");
    assert!(
        matches!(&resolution, Resolution::Declared { file, .. } if file == "web/c.ts"),
        "precondition: the chain must resolve to c.ts, got {resolution:?}"
    );
    for hop in ["web/a.ts", "web/b.ts", "web/c.ts"] {
        assert!(deps.reads(hop), "read set: {:?}", read_set(&deps));
    }
    assert!(!deps.reads("web/d.ts"), "read set: {:?}", read_set(&deps));
    assert!(
        deps.consults_path_set(),
        "extension probing reads which files exist"
    );
}

/// A relative TypeScript import needs no alias, so the alias-set flag must
/// stay clear — otherwise every TypeScript result would depend on every build
/// configuration in the repository.
#[test]
fn a_relative_typescript_import_does_not_consult_the_alias_set() {
    let files = &[
        (
            "web/a.ts",
            "import { X } from './b';\nexport const u = () => X;\n",
        ),
        ("web/b.ts", "export const X = 1;\n"),
    ];
    let (_, deps) = resolve_ts_tracked(files, "web/a.ts", "X");
    assert!(!deps.consults_alias_set());
    assert!(deps.consults_path_set());
}

/// A bare specifier is matched against the globally ordered alias list, so the
/// answer depends on every file that declares a build alias.
#[test]
fn a_bare_typescript_specifier_consults_the_alias_set() {
    let files = &[(
        "web/a.ts",
        "import { X } from 'somepkg';\nexport const u = () => X;\n",
    )];
    let (_, deps) = resolve_ts_tracked(files, "web/a.ts", "X");
    assert!(
        deps.consults_alias_set(),
        "a package specifier is resolved through the alias list"
    );
}

/// Python resolution never consults the alias set — `resolve_python_module`
/// does not call `apply_alias`. Pinned so the two languages cannot quietly
/// share an invalidation scope they do not share.
#[test]
fn python_resolution_never_consults_the_alias_set() {
    let files = &[
        (
            "app/h.py",
            "from .m import Order\ndef f():\n    return Order()\n",
        ),
        ("app/m.py", "class Order:\n    pass\n"),
        (
            "web/vite.config.ts",
            "export default { resolve: { alias: { openapi: './x' } } };\n",
        ),
    ];
    let (_, deps) = resolve_tracked(files, "app/h.py", "Order");
    assert!(
        !deps.consults_alias_set(),
        "Python resolution must not depend on build aliases"
    );
    assert!(
        !deps.reads("web/vite.config.ts"),
        "read set: {:?}",
        read_set(&deps)
    );
}

// ── whole-file Stage C dependency sets ──────────────────────────────────

const MODELS_PY: &str = "from sqlalchemy.orm import DeclarativeBase\nclass Base(DeclarativeBase):\n    pass\nclass Order(Base):\n    __tablename__ = 'orders'\n";
const HANDLERS_PY: &str = "from .models import Order\ndef make():\n    return Order(x=1)\n";

/// The unit a future cache would key: one file's entire ORM access
/// contribution, with the dependency set of everything it consulted.
#[test]
fn a_stage_c_result_carries_the_dependencies_of_every_access_it_found() {
    let files = &[
        ("app/handlers.py", HANDLERS_PY),
        ("app/models.py", MODELS_PY),
        ("app/unrelated.py", "VALUE = 1\n"),
        ("web/thing.ts", "export const X = 1;\n"),
    ];
    let analyses = analyze(files);
    let orm = cartograph_resolver::OrmAnalysis::build(&analyses);
    assert!(
        orm.models.contains_key("Order"),
        "precondition: the fixture must declare a resolvable model"
    );
    let index = ModuleIndex::build(&analyses);
    let handlers = index.file("app/handlers.py").expect("analysed");

    let mut deps = ResolutionContext::new();
    let sites =
        cartograph_resolver::discover_accesses_tracked(handlers, &orm.models, &index, &mut deps);

    assert!(
        !sites.is_empty(),
        "precondition: the fixture must produce an ORM access"
    );
    assert!(
        deps.reads("app/handlers.py"),
        "read set: {:?}",
        read_set(&deps)
    );
    assert!(
        deps.reads("app/models.py"),
        "read set: {:?}",
        read_set(&deps)
    );
    assert!(deps.consults_path_set());
    assert!(deps.is_complete());
    assert!(
        !deps.reads("app/unrelated.py"),
        "read set: {:?}",
        read_set(&deps)
    );
    assert!(
        !deps.reads("web/thing.ts"),
        "read set: {:?}",
        read_set(&deps)
    );
}

/// A file with no ORM accesses still reports its own contents as a
/// dependency: editing it is exactly what could introduce one.
#[test]
fn a_file_with_no_accesses_still_depends_on_its_own_contents() {
    let files = &[
        ("app/quiet.py", "VALUE = 1\n"),
        ("app/models.py", MODELS_PY),
    ];
    let analyses = analyze(files);
    let orm = cartograph_resolver::OrmAnalysis::build(&analyses);
    let index = ModuleIndex::build(&analyses);
    let quiet = index.file("app/quiet.py").expect("analysed");

    let mut deps = ResolutionContext::new();
    let sites =
        cartograph_resolver::discover_accesses_tracked(quiet, &orm.models, &index, &mut deps);
    assert!(sites.is_empty());
    assert_eq!(read_set(&deps), vec!["app/quiet.py"]);
    assert!(deps.is_complete());
}

/// The tracked and untracked ORM entry points must agree exactly.
#[test]
fn tracking_does_not_change_the_discovered_accesses() {
    let files = &[
        ("app/handlers.py", HANDLERS_PY),
        ("app/models.py", MODELS_PY),
    ];
    let analyses = analyze(files);
    let orm = cartograph_resolver::OrmAnalysis::build(&analyses);
    let index = ModuleIndex::build(&analyses);
    let handlers = index.file("app/handlers.py").expect("analysed");

    let untracked = cartograph_resolver::discover_accesses(handlers, &orm.models, &index);
    let mut deps = ResolutionContext::new();
    let tracked =
        cartograph_resolver::discover_accesses_tracked(handlers, &orm.models, &index, &mut deps);
    assert_eq!(format!("{untracked:?}"), format!("{tracked:?}"));
}

/// Golden read set: deterministic, sorted, repository-relative, no absolute
/// paths. The shape a future cache key would be derived from.
#[test]
fn the_golden_read_set_for_a_re_export_chain_is_stable() {
    let files = &[
        (
            "pkg/a.py",
            "from .b import Order\ndef f():\n    return Order()\n",
        ),
        ("pkg/b.py", "from .c import Order\n"),
        ("pkg/c.py", "class Order:\n    pass\n"),
        ("pkg/unused.py", "X = 1\n"),
    ];
    let (_, deps) = resolve_tracked(files, "pkg/a.py", "Order");
    assert_eq!(
        read_set(&deps),
        vec!["pkg/a.py", "pkg/b.py", "pkg/c.py"],
        "golden read set changed"
    );
    assert!(deps.consults_path_set());
    assert!(!deps.consults_alias_set());
    assert!(deps.is_complete());
    for path in read_set(&deps) {
        assert!(!path.contains(':'), "no absolute path may appear: {path}");
        assert!(!path.starts_with('/'), "no rooted path may appear: {path}");
    }
}

/// Completeness propagates through `absorb` across several nesting levels and
/// never recovers — the property a cache will rely on to refuse a result whose
/// dependencies were not fully observed.
#[test]
fn incompleteness_propagates_through_nested_absorption() {
    let mut innermost = ResolutionContext::new();
    innermost.record_file("deep.py");
    innermost.mark_incomplete();

    let mut middle = ResolutionContext::new();
    middle.record_file("mid.py");
    middle.absorb(&innermost);
    assert!(!middle.is_complete(), "one level");

    let mut outer = ResolutionContext::new();
    outer.record_file("outer.py");
    outer.absorb(&middle);
    assert!(!outer.is_complete(), "two levels");

    // Absorbing further complete contexts must not launder it back.
    let mut clean = ResolutionContext::new();
    clean.record_file("clean.py");
    outer.absorb(&clean);
    assert!(!outer.is_complete(), "completeness must never be restored");
    assert_eq!(
        read_set(&outer),
        vec!["clean.py", "deep.py", "mid.py", "outer.py"]
    );
}
