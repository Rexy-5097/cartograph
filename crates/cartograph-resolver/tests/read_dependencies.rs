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

// ── merged model set: canonical form and fingerprint ────────────────────

use cartograph_resolver::{ModelSetFingerprint, OrmAnalysis, canonical_model_set};

fn model_set(files: &[(&str, &str)]) -> (Vec<String>, ModelSetFingerprint) {
    let analyses = analyze(files);
    let orm = OrmAnalysis::build(&analyses);
    (canonical_model_set(&orm.models), orm.model_fingerprint)
}

const BASE_HDR: &str =
    "from sqlalchemy.orm import DeclarativeBase\nclass Base(DeclarativeBase):\n    pass\n";

/// One model: canonical form is `name@file`, sorted, repository-relative.
#[test]
fn the_canonical_model_set_is_name_and_declaring_file() {
    let (canonical, _) = model_set(&[(
        "app/models.py",
        &format!("{BASE_HDR}class Order(Base):\n    __tablename__ = 'orders'\n"),
    )]);
    assert!(
        canonical.contains(&"Order@app/models.py".to_owned()),
        "canonical set: {canonical:?}"
    );
    for entry in &canonical {
        assert!(!entry.contains(':'), "no absolute path may appear: {entry}");
    }
}

/// Two runs over the same repository must agree exactly.
#[test]
fn the_fingerprint_is_deterministic() {
    let files: &[(&str, &str)] = &[
        (
            "app/a.py",
            "from sqlalchemy.orm import DeclarativeBase\nclass Base(DeclarativeBase):\n    pass\nclass Order(Base):\n    __tablename__ = 'orders'\n",
        ),
        (
            "app/b.py",
            "from sqlalchemy.orm import DeclarativeBase\nclass Ledger(DeclarativeBase):\n    pass\nclass Receipt(Ledger):\n    __tablename__ = 'receipts'\n",
        ),
    ];
    let (c1, f1) = model_set(files);
    let (c2, f2) = model_set(files);
    assert_eq!(c1, c2);
    assert_eq!(f1, f2);
}

/// Declaration order in the repository must not change the fingerprint: the
/// canonical form is sorted, not insertion-ordered.
#[test]
fn declaration_order_does_not_change_the_fingerprint() {
    let a = "from sqlalchemy.orm import DeclarativeBase\nclass Base(DeclarativeBase):\n    pass\nclass Order(Base):\n    __tablename__ = 'orders'\n";
    let b = "from sqlalchemy.orm import DeclarativeBase\nclass Ledger(DeclarativeBase):\n    pass\nclass Receipt(Ledger):\n    __tablename__ = 'receipts'\n";
    let (_, forward) = model_set(&[("app/a.py", a), ("app/b.py", b)]);
    let (_, reverse) = model_set(&[("app/b.py", b), ("app/a.py", a)]);
    assert_eq!(forward, reverse, "sorting must remove order sensitivity");
}

/// Adding a model changes the fingerprint.
#[test]
fn adding_a_model_changes_the_fingerprint() {
    let one = "from sqlalchemy.orm import DeclarativeBase\nclass Base(DeclarativeBase):\n    pass\nclass Order(Base):\n    __tablename__ = 'orders'\n";
    let (_, before) = model_set(&[("app/a.py", one)]);
    let (_, after) = model_set(&[
        ("app/a.py", one),
        (
            "app/b.py",
            "from sqlalchemy.orm import DeclarativeBase\nclass Ledger(DeclarativeBase):\n    pass\nclass Receipt(Ledger):\n    __tablename__ = 'receipts'\n",
        ),
    ]);
    assert_ne!(before, after);
}

/// Renaming a model changes the fingerprint.
#[test]
fn renaming_a_model_changes_the_fingerprint() {
    let (_, before) = model_set(&[(
        "app/a.py",
        "from sqlalchemy.orm import DeclarativeBase\nclass Base(DeclarativeBase):\n    pass\nclass Order(Base):\n    __tablename__ = 'orders'\n",
    )]);
    let (_, after) = model_set(&[(
        "app/a.py",
        "from sqlalchemy.orm import DeclarativeBase\nclass Base(DeclarativeBase):\n    pass\nclass Invoice(Base):\n    __tablename__ = 'orders'\n",
    )]);
    assert_ne!(before, after);
}

/// Moving a model to another file changes the fingerprint, because
/// `resolves_to_the_model` compares the declaring file.
#[test]
fn moving_a_model_to_another_file_changes_the_fingerprint() {
    let body = "from sqlalchemy.orm import DeclarativeBase\nclass Base(DeclarativeBase):\n    pass\nclass Order(Base):\n    __tablename__ = 'orders'\n";
    let (canon_a, here) = model_set(&[("app/a.py", body)]);
    let (canon_b, there) = model_set(&[("app/b.py", body)]);
    assert_ne!(
        canon_a, canon_b,
        "the declaring file is part of the identity"
    );
    assert_ne!(here, there);
}

/// Ambiguity needs no field of its own: an ambiguous name leaves the merged
/// map, so the canonical set shrinks and the fingerprint moves.
#[test]
fn creating_and_removing_ambiguity_moves_the_fingerprint() {
    let a = "from sqlalchemy.orm import DeclarativeBase\nclass Base(DeclarativeBase):\n    pass\nclass Order(Base):\n    __tablename__ = 'orders'\n";
    let rival = "from sqlalchemy.orm import DeclarativeBase\nclass Base(DeclarativeBase):\n    pass\nclass Order(Base):\n    __tablename__ = 'other'\n";

    let (canon_one, unique) = model_set(&[("app/a.py", a)]);
    assert!(
        canon_one.iter().any(|e| e.starts_with("Order@")),
        "precondition: exactly one Order must resolve first, got {canon_one:?}"
    );

    let (canon_two, ambiguous) = model_set(&[("app/a.py", a), ("app/dup.py", rival)]);
    assert!(
        !canon_two.iter().any(|e| e.starts_with("Order@")),
        "precondition: the duplicate must make Order ambiguous, got {canon_two:?}"
    );
    assert_ne!(unique, ambiguous, "ambiguity must move the fingerprint");

    // ...and removing the rival restores the original identity exactly.
    let (canon_back, restored) = model_set(&[("app/a.py", a)]);
    assert_eq!(canon_one, canon_back);
    assert_eq!(unique, restored, "removing ambiguity must restore it");
}

/// A table rename must NOT move the fingerprint. Stage C never reads `table`,
/// so folding it in would invalidate every access entry for a change that
/// cannot alter a single access. Proven by tracing, pinned here.
#[test]
fn a_table_rename_does_not_move_the_stage_c_fingerprint() {
    let (_, orders) = model_set(&[(
        "app/a.py",
        "from sqlalchemy.orm import DeclarativeBase\nclass Base(DeclarativeBase):\n    pass\nclass Order(Base):\n    __tablename__ = 'orders'\n",
    )]);
    let (_, renamed) = model_set(&[(
        "app/a.py",
        "from sqlalchemy.orm import DeclarativeBase\nclass Base(DeclarativeBase):\n    pass\nclass Order(Base):\n    __tablename__ = 'purchase_orders'\n",
    )]);
    assert_eq!(
        orders, renamed,
        "the table is not a Stage C input; including it would over-invalidate"
    );
}

/// A file that declares no model leaves the fingerprint untouched.
#[test]
fn an_unrelated_non_model_file_does_not_move_the_fingerprint() {
    let a = "from sqlalchemy.orm import DeclarativeBase\nclass Base(DeclarativeBase):\n    pass\nclass Order(Base):\n    __tablename__ = 'orders'\n";
    let (_, without) = model_set(&[("app/a.py", a)]);
    let (_, with_extra) = model_set(&[("app/a.py", a), ("app/notes.py", "VALUE = 1\n")]);
    assert_eq!(without, with_extra);
}

// ── per-file aggregation ────────────────────────────────────────────────

/// Every Python file gets a bundle, including files with no accesses, and the
/// flattened `accesses` list is exactly the concatenation of the per-file
/// lists — the grouping regroups, it does not recompute.
#[test]
fn per_file_bundles_partition_the_flattened_accesses() {
    let files = &[
        ("app/handlers.py", HANDLERS_PY),
        ("app/models.py", MODELS_PY),
        ("app/quiet.py", "VALUE = 1\n"),
        ("web/thing.ts", "export const X = 1;\n"),
    ];
    let analyses = analyze(files);
    let orm = OrmAnalysis::build(&analyses);

    assert!(
        !orm.accesses.is_empty(),
        "precondition: the fixture must produce at least one access"
    );
    let regrouped: Vec<_> = orm
        .per_file
        .iter()
        .flat_map(|f| f.accesses.iter().cloned())
        .collect();
    assert_eq!(
        format!("{:?}", orm.accesses),
        format!("{regrouped:?}"),
        "the flattened list must equal the concatenation of the bundles"
    );

    let paths: Vec<&str> = orm.per_file.iter().map(|f| f.file.as_str()).collect();
    assert!(paths.contains(&"app/quiet.py"), "bundles: {paths:?}");
    assert!(
        !paths.contains(&"web/thing.ts"),
        "TypeScript files are not part of the ORM stage: {paths:?}"
    );
}

/// Each bundle carries the dependencies of its own file, and only those.
#[test]
fn each_bundle_carries_its_own_dependencies() {
    let files = &[
        ("app/handlers.py", HANDLERS_PY),
        ("app/models.py", MODELS_PY),
        ("app/quiet.py", "VALUE = 1\n"),
    ];
    let analyses = analyze(files);
    let orm = OrmAnalysis::build(&analyses);

    let handlers = orm
        .per_file
        .iter()
        .find(|f| f.file == "app/handlers.py")
        .expect("handlers bundle");
    assert!(
        !handlers.accesses.is_empty(),
        "precondition: handlers.py must produce an access"
    );
    assert!(handlers.dependencies.reads("app/handlers.py"));
    assert!(handlers.dependencies.reads("app/models.py"));
    assert!(handlers.dependencies.consults_path_set());
    assert!(handlers.dependencies.is_complete());
    assert!(
        !handlers.dependencies.reads("app/quiet.py"),
        "reads: {:?}",
        handlers.dependencies.files().collect::<Vec<_>>()
    );

    let quiet = orm
        .per_file
        .iter()
        .find(|f| f.file == "app/quiet.py")
        .expect("quiet bundle");
    assert!(quiet.accesses.is_empty());
    assert_eq!(
        quiet.dependencies.files().collect::<Vec<_>>(),
        vec!["app/quiet.py"]
    );
}

/// Aggregation must be deterministic across runs: same bundles, same order,
/// same dependency sets, same fingerprint.
#[test]
fn aggregation_is_deterministic_across_runs() {
    let files: &[(&str, &str)] = &[
        ("app/handlers.py", HANDLERS_PY),
        ("app/models.py", MODELS_PY),
    ];
    let first = OrmAnalysis::build(&analyze(files));
    let second = OrmAnalysis::build(&analyze(files));

    assert_eq!(first.model_fingerprint, second.model_fingerprint);
    assert_eq!(first.per_file.len(), second.per_file.len());
    for (a, b) in first.per_file.iter().zip(second.per_file.iter()) {
        assert_eq!(a.file, b.file);
        assert_eq!(format!("{:?}", a.accesses), format!("{:?}", b.accesses));
        assert_eq!(a.dependencies, b.dependencies);
    }
}

/// Failure safety: the aggregation layer must never launder an incomplete
/// context into a complete one. Simulated by marking a bundle's context
/// incomplete and absorbing it, the way a cache-eligibility check would.
#[test]
fn aggregation_preserves_incompleteness() {
    let files = &[
        ("app/handlers.py", HANDLERS_PY),
        ("app/models.py", MODELS_PY),
    ];
    let analyses = analyze(files);
    let orm = OrmAnalysis::build(&analyses);
    assert!(
        orm.per_file.iter().all(|f| f.dependencies.is_complete()),
        "precondition: this fixture tracks everything"
    );

    let mut rolled_up = ResolutionContext::new();
    for bundle in &orm.per_file {
        rolled_up.absorb(&bundle.dependencies);
    }
    assert!(rolled_up.is_complete());

    // One incomplete contribution must disqualify the whole roll-up.
    let mut broken = orm.per_file[0].dependencies.clone();
    broken.mark_incomplete();
    let mut with_broken = ResolutionContext::new();
    with_broken.absorb(&broken);
    for bundle in orm.per_file.iter().skip(1) {
        with_broken.absorb(&bundle.dependencies);
    }
    assert!(
        !with_broken.is_complete(),
        "an incomplete contribution must never be laundered by absorbing complete ones"
    );
}

/// Per-file Stage C bundles and the model fingerprint on a real repository.
///
/// Opt-in; ignored by default. Lives in `tests/` because QG-008 forbids the
/// resolver's sources from reading the environment.
#[test]
#[ignore = "needs CARTOGRAPH_READSET_REPO; inspects rather than asserts"]
fn per_file_bundles_on_a_real_repository() {
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

    let orm = OrmAnalysis::build(&analyses);

    let mut histogram: std::collections::BTreeMap<usize, usize> = std::collections::BTreeMap::new();
    let mut path_set = 0usize;
    let mut alias_set = 0usize;
    let mut incomplete = 0usize;
    let mut with_accesses = 0usize;
    for bundle in &orm.per_file {
        *histogram
            .entry(bundle.dependencies.file_count())
            .or_default() += 1;
        if bundle.dependencies.consults_path_set() {
            path_set += 1;
        }
        if bundle.dependencies.consults_alias_set() {
            alias_set += 1;
        }
        if !bundle.dependencies.is_complete() {
            incomplete += 1;
        }
        if !bundle.accesses.is_empty() {
            with_accesses += 1;
        }
    }

    println!("python files        : {}", analyses.len());
    println!("per-file bundles    : {}", orm.per_file.len());
    println!("bundles w/ accesses : {with_accesses}");
    println!("total accesses      : {}", orm.accesses.len());
    println!(
        "models / ambiguous  : {} / {}",
        orm.models.len(),
        orm.ambiguous.len()
    );
    println!("model fingerprint   : {:016x}", orm.model_fingerprint.get());
    let index = ModuleIndex::build(&analyses);
    let py_paths = index.canonical_python_path_set();
    let aliases = index.canonical_alias_set();
    println!(
        "python path set     : {} paths, fingerprint {:016x}",
        py_paths.len(),
        index.python_path_set_fingerprint().get()
    );
    println!(
        "alias set           : {} entries, fingerprint {:016x}",
        aliases.len(),
        index.alias_set_fingerprint().get()
    );
    for a in aliases.iter().take(4) {
        println!("      alias {a}");
    }
    println!("consulted path set  : {path_set}");
    println!("consulted alias set : {alias_set}");
    println!("incomplete bundles  : {incomplete}");
    println!("dependency-set size histogram (files -> bundles):");
    for (size, count) in &histogram {
        println!("  {size:3} -> {count}");
    }
    println!("largest bundles:");
    let mut by_size: Vec<_> = orm.per_file.iter().collect();
    by_size.sort_by_key(|b| std::cmp::Reverse(b.dependencies.file_count()));
    for bundle in by_size.iter().take(3) {
        println!(
            "  {} accesses={} reads={} path_set={} complete={}",
            bundle.file,
            bundle.accesses.len(),
            bundle.dependencies.file_count(),
            bundle.dependencies.consults_path_set(),
            bundle.dependencies.is_complete()
        );
        for read in bundle.dependencies.files().take(6) {
            println!("      {read}");
        }
    }

    // The flattened list must be exactly the concatenation of the bundles,
    // on a real repository and not only on fixtures.
    let regrouped: usize = orm.per_file.iter().map(|b| b.accesses.len()).sum();
    assert_eq!(
        regrouped,
        orm.accesses.len(),
        "bundles must partition the flattened accesses"
    );
    // TypeScript must never depend on the Python ORM path.
    assert_eq!(
        alias_set, 0,
        "Python ORM bundles must not consult build aliases"
    );
}

// ── Python path-set fingerprint ─────────────────────────────────────────

use cartograph_resolver::{AliasSetFingerprint, PathSetFingerprint, PerFileDependencyIdentity};

fn path_set(files: &[(&str, &str)]) -> (Vec<String>, PathSetFingerprint) {
    let analyses = analyze(files);
    let index = ModuleIndex::build(&analyses);
    (
        index.canonical_python_path_set(),
        index.python_path_set_fingerprint(),
    )
}

fn alias_set(files: &[(&str, &str)]) -> (Vec<String>, AliasSetFingerprint) {
    let analyses = analyze(files);
    let index = ModuleIndex::build(&analyses);
    (index.canonical_alias_set(), index.alias_set_fingerprint())
}

const PY_A: &str = "class A:\n    pass\n";
const PY_B: &str = "class B:\n    pass\n";

/// The canonical set is exactly the `.py` paths, sorted and
/// repository-relative.
#[test]
fn the_canonical_python_path_set_is_the_dot_py_paths() {
    let (canonical, _) = path_set(&[("b/second.py", PY_B), ("a/first.py", PY_A)]);
    assert_eq!(canonical, vec!["a/first.py", "b/second.py"]);
    for p in &canonical {
        assert!(!p.contains(':'), "no absolute path: {p}");
        assert!(!p.starts_with('/'), "no rooted path: {p}");
    }
}

#[test]
fn adding_or_removing_a_python_path_moves_the_fingerprint() {
    let (_, one) = path_set(&[("a.py", PY_A)]);
    let (_, two) = path_set(&[("a.py", PY_A), ("b.py", PY_B)]);
    assert_ne!(one, two, "adding a path must move it");

    let (_, back) = path_set(&[("a.py", PY_A)]);
    assert_eq!(one, back, "removing it must restore it exactly");
}

#[test]
fn renaming_a_python_path_moves_the_fingerprint() {
    let (_, before) = path_set(&[("app/models.py", PY_A)]);
    let (_, after) = path_set(&[("app/schema.py", PY_A)]);
    assert_ne!(before, after);
}

#[test]
fn enumeration_order_does_not_move_the_path_set_fingerprint() {
    let (_, forward) = path_set(&[("a.py", PY_A), ("b.py", PY_B)]);
    let (_, reverse) = path_set(&[("b.py", PY_B), ("a.py", PY_A)]);
    assert_eq!(forward, reverse, "the canonical set is sorted");
}

#[test]
fn the_empty_python_path_set_is_stable_and_distinct() {
    let (canonical, empty) = path_set(&[("web/x.ts", "export const X = 1;\n")]);
    assert!(canonical.is_empty(), "canonical: {canonical:?}");
    let (_, also_empty) = path_set(&[("web/y.ts", "export const Y = 2;\n")]);
    assert_eq!(empty, also_empty);
    let (_, non_empty) = path_set(&[("a.py", PY_A)]);
    assert_ne!(empty, non_empty);
}

/// A TypeScript file cannot satisfy a Python module candidate, so it must not
/// enter the Python path set. Asserted together with the resolver behaviour it
/// describes, so the fingerprint cannot drift away from what it stands for.
#[test]
fn typescript_files_are_absent_from_the_python_path_set() {
    let base: &[(&str, &str)] = &[
        (
            "app/h.py",
            "from .m import Order\ndef f():\n    return Order()\n",
        ),
        ("app/m.py", "class Order:\n    pass\n"),
    ];
    let (canonical_before, before) = path_set(base);
    let (resolution_before, _) = resolve_tracked(base, "app/h.py", "Order");
    assert!(
        matches!(&resolution_before, Resolution::Declared { file, .. } if file == "app/m.py"),
        "precondition: it must resolve, got {resolution_before:?}"
    );

    let with_ts: &[(&str, &str)] = &[
        (
            "app/h.py",
            "from .m import Order\ndef f():\n    return Order()\n",
        ),
        ("app/m.py", "class Order:\n    pass\n"),
        ("app/m.ts", "export const Order = 1;\n"),
        ("web/other.tsx", "export const Q = 2;\n"),
    ];
    let (canonical_after, after) = path_set(with_ts);
    let (resolution_after, _) = resolve_tracked(with_ts, "app/h.py", "Order");

    assert_eq!(
        canonical_before, canonical_after,
        "TS must not enter the set"
    );
    assert_eq!(before, after, "TS must not move the fingerprint");
    assert_eq!(
        format!("{resolution_before:?}"),
        format!("{resolution_after:?}"),
        "and the resolver must agree that TS changed nothing"
    );
}

/// `.pyi` cannot satisfy a candidate either — candidates end in `.py`, and
/// `"x.pyi".ends_with("/x.py")` is false.
#[test]
fn pyi_stubs_are_absent_from_the_python_path_set() {
    let (canonical, without) = path_set(&[("app/m.py", PY_A)]);
    let (canonical_with, with_stub) = path_set(&[("app/m.py", PY_A), ("app/m.pyi", PY_A)]);
    assert_eq!(canonical, canonical_with, "canonical: {canonical_with:?}");
    assert_eq!(without, with_stub);
}

/// The colliding-candidate case, tied to the resolver: adding a `.py` path
/// that collides moves the fingerprint *and* changes the resolution.
#[test]
fn a_colliding_python_path_moves_the_fingerprint_and_the_resolution() {
    let before_files: &[(&str, &str)] = &[
        (
            "app/h.py",
            "from .m import Order\ndef f():\n    return Order()\n",
        ),
        ("app/m.py", "class Order:\n    pass\n"),
    ];
    let (_, before) = path_set(before_files);
    let (r_before, _) = resolve_tracked(before_files, "app/h.py", "Order");
    assert!(
        matches!(r_before, Resolution::Declared { .. }),
        "precondition: exactly one candidate must match first"
    );

    let after_files: &[(&str, &str)] = &[
        (
            "app/h.py",
            "from .m import Order\ndef f():\n    return Order()\n",
        ),
        ("app/m.py", "class Order:\n    pass\n"),
        ("vendor/app/m.py", "VERSION = 1\n"),
    ];
    let (_, after) = path_set(after_files);
    let (r_after, _) = resolve_tracked(after_files, "app/h.py", "Order");

    assert_ne!(
        before, after,
        "the colliding path must move the fingerprint"
    );
    assert!(
        matches!(r_after, Resolution::Unresolved { .. }),
        "and the resolution must actually change, got {r_after:?}"
    );
}

// ── TypeScript ordered alias fingerprint ────────────────────────────────

const VITE_ONE: &str = "export default { resolve: { alias: { '@app': '/src/app' } } };\n";
const VITE_TWO: &str =
    "export default { resolve: { alias: { '@app': '/src/app', '@lib': '/src/lib' } } };\n";
const VITE_RETARGET: &str = "export default { resolve: { alias: { '@app': '/src/other' } } };\n";

#[test]
fn no_aliases_gives_a_stable_empty_alias_fingerprint() {
    let (canonical, empty) = alias_set(&[("web/a.ts", "export const X = 1;\n")]);
    assert!(canonical.is_empty(), "canonical: {canonical:?}");
    let (_, also_empty) = alias_set(&[("web/b.ts", "export const Y = 2;\n")]);
    assert_eq!(empty, also_empty);
}

#[test]
fn adding_and_removing_an_alias_moves_the_alias_fingerprint() {
    let (canonical_without, without_config) = alias_set(&[("web/a.ts", "export const X = 1;\n")]);
    let (canonical_with, with_config) = alias_set(&[
        ("web/a.ts", "export const X = 1;\n"),
        ("web/vite.config.ts", VITE_ONE),
    ]);
    assert!(
        canonical_without.len() < canonical_with.len(),
        "precondition: the config must actually declare an alias, got {canonical_with:?}"
    );
    assert_ne!(without_config, with_config);

    let (_, removed_again) = alias_set(&[("web/a.ts", "export const X = 1;\n")]);
    assert_eq!(
        without_config, removed_again,
        "removing the config must restore the identity"
    );
}

#[test]
fn adding_a_second_alias_moves_the_alias_fingerprint() {
    let (canonical_one, one) = alias_set(&[("web/vite.config.ts", VITE_ONE)]);
    let (canonical_two, two) = alias_set(&[("web/vite.config.ts", VITE_TWO)]);
    assert!(
        canonical_two.len() > canonical_one.len(),
        "precondition: the second config must declare more aliases: {canonical_two:?}"
    );
    assert_ne!(one, two);
}

#[test]
fn retargeting_an_alias_moves_the_alias_fingerprint() {
    let (canonical_a, a) = alias_set(&[("web/vite.config.ts", VITE_ONE)]);
    let (canonical_b, b) = alias_set(&[("web/vite.config.ts", VITE_RETARGET)]);
    assert_ne!(
        canonical_a, canonical_b,
        "precondition: the target must actually differ"
    );
    assert_ne!(a, b, "the target participates in resolution");
}

/// Order is semantic — `apply_alias` returns the first match — so two lists
/// with the same members in a different order must not share an identity.
#[test]
fn alias_order_is_part_of_the_alias_fingerprint() {
    let ordered = vec![
        "@app|/src/app|web".to_owned(),
        "@lib|/src/lib|web".to_owned(),
    ];
    let reversed = vec![
        "@lib|/src/lib|web".to_owned(),
        "@app|/src/app|web".to_owned(),
    ];
    assert_ne!(
        AliasSetFingerprint::of(&ordered),
        AliasSetFingerprint::of(&reversed),
        "a membership-only identity would be unsound here"
    );
    assert_eq!(
        AliasSetFingerprint::of(&ordered),
        AliasSetFingerprint::of(&ordered.clone())
    );
}

/// An ordinary TypeScript edit that declares no alias must leave the alias
/// identity alone.
#[test]
fn an_unrelated_typescript_edit_does_not_move_the_alias_fingerprint() {
    let (_, before) = alias_set(&[
        ("web/vite.config.ts", VITE_ONE),
        ("web/a.ts", "export const X = 1;\n"),
    ]);
    let (_, after) = alias_set(&[
        ("web/vite.config.ts", VITE_ONE),
        ("web/a.ts", "export const X = 999;\nexport const Z = 2;\n"),
    ]);
    assert_eq!(before, after);
}

// ── language separation ─────────────────────────────────────────────────

/// A Python result must never carry an alias identity, and a relative
/// TypeScript import must never carry one either.
#[test]
fn the_dependency_identity_attaches_only_the_state_actually_consulted() {
    let py: &[(&str, &str)] = &[
        (
            "app/h.py",
            "from .m import Order\ndef f():\n    return Order()\n",
        ),
        ("app/m.py", "class Order:\n    pass\n"),
        ("web/vite.config.ts", VITE_ONE),
    ];
    let analyses = analyze(py);
    let index = ModuleIndex::build(&analyses);
    let from = index.file("app/h.py").expect("analysed");
    let mut deps = ResolutionContext::new();
    index.resolve_python_tracked(from, "Order", &mut deps);

    let identity = PerFileDependencyIdentity::new(
        "app/h.py",
        0xdead_beef,
        0x1234,
        &deps,
        index.python_path_set_fingerprint(),
        index.alias_set_fingerprint(),
    );
    assert!(
        identity.path_set.is_some(),
        "Python resolution consulted repository membership"
    );
    assert!(
        identity.alias_set.is_none(),
        "Python must not inherit TypeScript alias identity"
    );
    assert!(identity.complete && identity.is_reusable());
    assert!(identity.reads.contains(&"app/m.py".to_owned()));
    assert!(!identity.reads.contains(&"web/vite.config.ts".to_owned()));

    // A relative TypeScript import: path set yes, alias set no.
    let ts: &[(&str, &str)] = &[
        (
            "web/a.ts",
            "import { X } from './b';\nexport const u = () => X;\n",
        ),
        ("web/b.ts", "export const X = 1;\n"),
        ("web/vite.config.ts", VITE_ONE),
    ];
    let analyses = analyze(ts);
    let index = ModuleIndex::build(&analyses);
    let from = index.file("web/a.ts").expect("analysed");
    let mut deps = ResolutionContext::new();
    index.resolve_typescript_tracked(from, "X", &mut deps);
    let identity = PerFileDependencyIdentity::new(
        "web/a.ts",
        1,
        2,
        &deps,
        index.python_path_set_fingerprint(),
        index.alias_set_fingerprint(),
    );
    assert!(
        identity.alias_set.is_none(),
        "a relative import must not inherit global alias identity"
    );
}

/// An incomplete identity is never reusable, whatever else matches.
#[test]
fn an_incomplete_identity_is_never_reusable() {
    let mut deps = ResolutionContext::new();
    deps.record_file("a.py");
    deps.record_path_set();
    deps.mark_incomplete();

    let identity = PerFileDependencyIdentity::new(
        "a.py",
        7,
        9,
        &deps,
        PathSetFingerprint::default(),
        AliasSetFingerprint::default(),
    );
    assert!(!identity.complete);
    assert!(
        !identity.is_reusable(),
        "incomplete tracking must disqualify reuse"
    );
}

/// The identity is deterministic for the same semantic state.
#[test]
fn the_dependency_identity_is_deterministic() {
    let files: &[(&str, &str)] = &[
        (
            "app/h.py",
            "from .m import Order\ndef f():\n    return Order()\n",
        ),
        ("app/m.py", "class Order:\n    pass\n"),
    ];
    let build = || {
        let analyses = analyze(files);
        let index = ModuleIndex::build(&analyses);
        let from = index.file("app/h.py").expect("analysed");
        let mut deps = ResolutionContext::new();
        index.resolve_python_tracked(from, "Order", &mut deps);
        PerFileDependencyIdentity::new(
            "app/h.py",
            42,
            43,
            &deps,
            index.python_path_set_fingerprint(),
            index.alias_set_fingerprint(),
        )
    };
    assert_eq!(build(), build());
}

// ── real-repository alias validation ────────────────────────────────────

/// Loads a repository's TypeScript sources as `(path, source)` pairs.
///
/// Sources are kept in memory so mutations can be applied without touching
/// the checkout: the pinned corpus must stay pristine.
fn load_typescript(root: &std::path::Path) -> Vec<(String, String)> {
    fn walk_ts(root: &std::path::Path, dir: &std::path::Path, out: &mut Vec<String>) {
        const SKIP: [&str; 6] = [
            "node_modules",
            ".git",
            "dist",
            "build",
            "coverage",
            "target",
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
                walk_ts(root, &path, out);
            } else if matches!(
                std::path::Path::new(name)
                    .extension()
                    .and_then(|e| e.to_str()),
                Some("ts" | "tsx")
            ) {
                if let Ok(rel) = path.strip_prefix(root) {
                    out.push(rel.to_string_lossy().replace('\\', "/"));
                }
            }
        }
    }

    let mut paths = Vec::new();
    walk_ts(root, root, &mut paths);
    paths.sort();
    paths
        .into_iter()
        .filter_map(|rel| {
            std::fs::read_to_string(root.join(&rel))
                .ok()
                .map(|src| (rel, src))
        })
        .collect()
}

fn analyze_owned(files: &[(String, String)]) -> Vec<FileAnalysis> {
    let mut a = Analyzer::new().expect("grammars load");
    files
        .iter()
        .filter_map(|(p, s)| a.analyze_source(p, s).ok())
        .collect()
}

/// The first file importing through the `openapi` alias, and a name it binds.
fn aliased_importer(analyses: &[FileAnalysis]) -> (&FileAnalysis, String) {
    let importer = analyses
        .iter()
        .find(|f| {
            f.imports
                .iter()
                .any(|i| i.specifier.starts_with("openapi/"))
        })
        .expect("precondition: some file must import through the `openapi` alias");
    let name = importer
        .imports
        .iter()
        .find(|i| i.specifier.starts_with("openapi/"))
        .and_then(|i| i.named.first())
        .map(|n| n.local.as_deref().unwrap_or(n.imported.as_str()).to_owned())
        .expect("that import must bind a name");
    (importer, name)
}

/// A purely relative importer must not inherit the alias dependency.
fn assert_relative_import_has_no_alias_dependency(
    index: &ModuleIndex<'_>,
    analyses: &[FileAnalysis],
) {
    let relative_importer = analyses
        .iter()
        .find(|f| {
            f.imports.iter().any(|i| i.specifier.starts_with("./"))
                && !f.imports.iter().any(|i| !i.specifier.starts_with('.'))
        })
        .or_else(|| {
            analyses.iter().find(|f| {
                f.imports.iter().all(|i| i.specifier.starts_with('.')) && !f.imports.is_empty()
            })
        });
    if let Some(rel) = relative_importer {
        let rel_name = rel
            .imports
            .iter()
            .find(|i| i.specifier.starts_with('.'))
            .and_then(|i| i.named.first())
            .map(|n| n.local.as_deref().unwrap_or(n.imported.as_str()).to_owned());
        if let Some(rel_name) = rel_name {
            let mut rel_deps = ResolutionContext::new();
            index.resolve_typescript_tracked(rel, &rel_name, &mut rel_deps);
            println!(
                "relative resolution : {} :: {rel_name} alias_set={}",
                rel.path,
                rel_deps.consults_alias_set()
            );
            assert!(
                !rel_deps.consults_alias_set(),
                "a purely relative importer must not depend on the alias list"
            );
        }
    }
}

/// What one alias mutation produced: the canonical list, its identity, and the
/// resolution of the aliased import under it.
type MutationOutcome = (Vec<String>, AliasSetFingerprint, Resolution);

/// Adding an alias moves the identity; an edit declaring none does not.
fn assert_alias_addition_and_unrelated_edit(
    files: &[(String, String)],
    config_index: usize,
    original_source: &str,
    baseline_len: usize,
    fingerprint: AliasSetFingerprint,
    mutate: &dyn Fn(Option<&str>) -> MutationOutcome,
) {
    // 3. Adding an alias.
    let added = original_source.replace(
        r#"alias: { openapi: "/openapi-gen", src: "/src" }"#,
        r#"alias: { openapi: "/openapi-gen", src: "/src", extra: "/extra" }"#,
    );
    let (added_canonical, added_fp, _) = mutate(Some(&added));
    assert!(
        added_canonical.len() > baseline_len,
        "precondition: the alias must actually be added: {added_canonical:?}"
    );
    assert_ne!(
        fingerprint, added_fp,
        "adding an alias must move the fingerprint"
    );

    // 4. An unrelated TypeScript edit must leave the alias identity alone.
    let mut unrelated = files.to_vec();
    let victim = unrelated
        .iter()
        .position(|(p, _)| {
            p != &files[config_index].0
                && std::path::Path::new(p)
                    .extension()
                    .is_some_and(|e| e == "tsx")
        })
        .expect("some other file exists");
    unrelated[victim]
        .1
        .push_str("\nexport const CARTOGRAPH_M10_PROBE = 1;\n");
    let unrelated_index_analyses = analyze_owned(&unrelated);
    let unrelated_index = ModuleIndex::build(&unrelated_index_analyses);
    println!(
        "mutation: unrelated edit -> fp {:016x}",
        unrelated_index.alias_set_fingerprint().get()
    );
    assert_eq!(
        fingerprint,
        unrelated_index.alias_set_fingerprint(),
        "an edit that declares no alias must not move the alias identity"
    );
}

/// Resolves every aliased import in the repository and reports the population.
///
/// Separated from the test body so the reader sees the scan and the assertions
/// apart, and so the test function stays a readable length.
fn report_aliased_population(index: &ModuleIndex<'_>, analyses: &[FileAnalysis]) -> usize {
    // The exemplar reported by the caller is whichever aliased import came first, which may be
    // a type-only import whose name the extractor does not record as a symbol.
    // Scan every aliased import so the report describes the population rather
    // than one arbitrary member, and surface a fully resolved one if it exists.
    let mut declared = 0usize;
    let mut not_bound = 0usize;
    let mut unresolved = 0usize;
    let mut other = 0usize;
    let mut alias_aware = 0usize;
    let mut example_declared: Option<String> = None;
    for file in analyses {
        for import in &file.imports {
            if !(import.specifier.starts_with("openapi/") || import.specifier.starts_with("src/")) {
                continue;
            }
            for named in &import.named {
                let n = named.local.as_deref().unwrap_or(named.imported.as_str());
                let mut d = ResolutionContext::new();
                let r = index.resolve_typescript_tracked(file, n, &mut d);
                if d.consults_alias_set() {
                    alias_aware += 1;
                }
                match &r {
                    Resolution::Declared { file: at, .. } => {
                        declared += 1;
                        if example_declared.is_none() {
                            example_declared = Some(format!(
                                "{} :: {n} -> Declared({at})  reads {:?}",
                                file.path,
                                d.files().collect::<Vec<_>>()
                            ));
                        }
                    }
                    Resolution::NotBound => not_bound += 1,
                    Resolution::Unresolved { .. } => unresolved += 1,
                    _ => other += 1,
                }
            }
        }
    }
    println!(
        "aliased imports     : declared={declared} not_bound={not_bound} unresolved={unresolved} other={other}"
    );
    println!("alias-aware results : {alias_aware}");
    if let Some(example) = &example_declared {
        println!("fully resolved      : {example}");
    }
    alias_aware
}

/// Validates the ordered alias fingerprint against a real pinned repository
/// that genuinely declares and uses Vite aliases.
///
/// Apache Airflow at `9b43d6abc0fc` declares, in
/// `airflow-core/src/airflow/ui/vite.config.ts`:
///
/// ```text
/// resolve: { alias: { openapi: "/openapi-gen", src: "/src" } }
/// ```
///
/// Both targets are string literals, so the parser records them; both
/// directories exist; and the UI imports through both. Fixtures alone could
/// not establish that the fingerprint describes real resolution.
///
/// Opt-in and ignored by default: it needs a checkout this repository does not
/// vendor. Mutations are applied to in-memory copies of the sources, never to
/// the checkout.
#[test]
#[ignore = "needs CARTOGRAPH_ALIAS_REPO; validates against a real pinned checkout"]
fn alias_dependencies_on_a_real_repository() {
    let Ok(repo) = std::env::var("CARTOGRAPH_ALIAS_REPO") else {
        return;
    };
    let root = std::path::PathBuf::from(repo);
    let files = load_typescript(&root);
    assert!(
        files.len() > 100,
        "precondition: expected a real frontend, found {} files",
        files.len()
    );

    let analyses = analyze_owned(&files);
    let index = ModuleIndex::build(&analyses);
    let canonical = index.canonical_alias_set();
    let fingerprint = index.alias_set_fingerprint();

    println!("typescript files    : {}", analyses.len());
    println!("alias declarations  : {}", canonical.len());
    for a in &canonical {
        println!("      {a}");
    }
    println!("alias fingerprint   : {:016x}", fingerprint.get());

    // ── PART 3/4: the aliases are real and ordered as the resolver walks ──
    assert!(
        canonical.len() >= 2,
        "precondition: this repository must declare at least two aliases, got {canonical:?}"
    );
    assert!(
        canonical.iter().any(|a| a.starts_with("openapi|")),
        "the `openapi` alias must be recorded: {canonical:?}"
    );
    assert!(
        canonical.iter().any(|a| a.starts_with("src|")),
        "the `src` alias must be recorded: {canonical:?}"
    );
    // Sorted by directory depth, then alias length: `openapi` precedes `src`.
    let openapi_at = canonical.iter().position(|a| a.starts_with("openapi|"));
    let src_at = canonical.iter().position(|a| a.starts_with("src|"));
    assert!(
        openapi_at < src_at,
        "longer alias must sort first, which is what makes ordering semantic: {canonical:?}"
    );

    // ── PART 3: an alias actually participates in a resolution ────────────
    let (importer, name) = aliased_importer(&analyses);

    let mut deps = ResolutionContext::new();
    let resolution = index.resolve_typescript_tracked(importer, &name, &mut deps);
    println!("aliased resolution  : {} :: {name}", importer.path);
    println!("      -> {resolution:?}");
    println!("      reads {:?}", deps.files().collect::<Vec<_>>());
    println!(
        "      path_set={} alias_set={} complete={}",
        deps.consults_path_set(),
        deps.consults_alias_set(),
        deps.is_complete()
    );
    assert!(
        deps.consults_alias_set(),
        "a bare aliased specifier must record the alias dependency"
    );

    let alias_aware = report_aliased_population(&index, &analyses);
    assert!(
        alias_aware > 100,
        "the alias mechanism must be exercised broadly, not once: {alias_aware}"
    );
    assert!(
        deps.reads(&importer.path),
        "the importing file's own contents are a dependency: {:?}",
        deps.files().collect::<Vec<_>>()
    );

    assert_relative_import_has_no_alias_dependency(&index, &analyses);
}

/// Alias mutations against the same real pinned repository.
///
/// Split from the declaration/resolution checks so each stays readable.
/// Mutations are applied to in-memory copies of the sources; the checkout is
/// never modified.
#[test]
#[ignore = "needs CARTOGRAPH_ALIAS_REPO; validates against a real pinned checkout"]
fn alias_mutations_on_a_real_repository() {
    let Ok(repo) = std::env::var("CARTOGRAPH_ALIAS_REPO") else {
        return;
    };
    let root = std::path::PathBuf::from(repo);
    let files = load_typescript(&root);
    let analyses = analyze_owned(&files);
    let index = ModuleIndex::build(&analyses);
    let canonical = index.canonical_alias_set();
    let fingerprint = index.alias_set_fingerprint();
    assert_eq!(
        canonical.len(),
        2,
        "precondition: two aliases expected, got {canonical:?}"
    );

    let (importer, name) = aliased_importer(&analyses);
    let mut base_deps = ResolutionContext::new();
    let resolution = index.resolve_typescript_tracked(importer, &name, &mut base_deps);

    // ── PART 6/10: mutations ──────────────────────────────────────────────
    let config_index = files
        .iter()
        .position(|(p, _)| p.ends_with("vite.config.ts"))
        .expect("precondition: the vite config must be among the loaded files");

    let mutate = |replace_config: Option<&str>| -> MutationOutcome {
        let mut copy = files.clone();
        match replace_config {
            Some(new_source) => copy[config_index].1 = new_source.to_owned(),
            None => {
                copy.remove(config_index);
            }
        }
        let analyses = analyze_owned(&copy);
        let index = ModuleIndex::build(&analyses);
        let from = index
            .file(&importer.path)
            .expect("the importer survives every mutation");
        let mut d = ResolutionContext::new();
        let r = index.resolve_typescript_tracked(from, &name, &mut d);
        (
            index.canonical_alias_set(),
            index.alias_set_fingerprint(),
            r,
        )
    };

    let original_source = files[config_index].1.clone();
    let baseline_resolution = format!("{resolution:?}");

    // 1. Removing the config removes both aliases.
    let (removed_canonical, removed_fp, removed_resolution) = mutate(None);
    println!(
        "mutation: config removed -> {} aliases, fp {:016x}",
        removed_canonical.len(),
        removed_fp.get()
    );
    assert!(removed_canonical.is_empty(), "{removed_canonical:?}");
    assert_ne!(
        fingerprint, removed_fp,
        "removing aliases must move the fingerprint"
    );
    assert_ne!(
        baseline_resolution,
        format!("{removed_resolution:?}"),
        "and the aliased import must stop resolving the same way"
    );

    // 2. Retargeting the alias: the target participates in resolution.
    let retargeted = original_source.replace(
        r#"alias: { openapi: "/openapi-gen", src: "/src" }"#,
        r#"alias: { openapi: "/elsewhere-gen", src: "/src" }"#,
    );
    assert_ne!(
        retargeted, original_source,
        "precondition: the retarget substitution must actually apply"
    );
    let (retargeted_canonical, retargeted_fp, retargeted_resolution) = mutate(Some(&retargeted));
    println!(
        "mutation: retargeted     -> fp {:016x}  {retargeted_canonical:?}",
        retargeted_fp.get()
    );
    assert_ne!(
        fingerprint, retargeted_fp,
        "the target is part of the identity"
    );
    assert_ne!(
        baseline_resolution,
        format!("{retargeted_resolution:?}"),
        "retargeting to a directory that does not exist must change the result"
    );

    assert_alias_addition_and_unrelated_edit(
        &files,
        config_index,
        &original_source,
        canonical.len(),
        fingerprint,
        &mutate,
    );
}
