//! Import resolution: a name, the module it came from, and the declaration.
//!
//! Every supported form has a positive fixture, a negative fixture and an
//! ambiguity fixture, and the forms that exist because a real repository
//! needed them say which repository.

use cartograph_parser::Analyzer;
use cartograph_parser::model::FileAnalysis;
use cartograph_resolver::{ModuleIndex, Resolution};

fn analyze(files: &[(&str, &str)]) -> Vec<FileAnalysis> {
    let mut a = Analyzer::new().expect("grammars load");
    files
        .iter()
        .map(|(p, s)| a.analyze_source(p, s).expect("fixture parses"))
        .collect()
}

fn resolve(files: &[(&str, &str)], from: &str, name: &str) -> Resolution {
    let analyses = analyze(files);
    let index = ModuleIndex::build(&analyses);
    let file = index.file(from).expect("the using file was analysed");
    index.resolve_python(file, name)
}

const MODELS: &str = r"
from django.db import models

class Order(models.Model):
    class Meta:
        db_table = 'orders'
";

// ── Positive ────────────────────────────────────────────────────────

#[test]
fn an_absolute_import_resolves_to_its_declaration() {
    let r = resolve(
        &[
            ("app/models.py", MODELS),
            (
                "app/views.py",
                "from app.models import Order\n\ndef create():\n    return Order()\n",
            ),
        ],
        "app/views.py",
        "Order",
    );
    let Resolution::Declared { file, line, .. } = &r else {
        panic!("expected a declaration, got {r:?}");
    };
    assert_eq!(file, "app/models.py");
    assert_eq!(*line, 4, "the class declaration's own line");
}

#[test]
fn a_relative_import_resolves_within_the_package() {
    let r = resolve(
        &[
            ("app/models.py", MODELS),
            (
                "app/views.py",
                "from .models import Order\n\ndef create():\n    return Order()\n",
            ),
        ],
        "app/views.py",
        "Order",
    );
    assert_eq!(r.declared_in(), Some("app/models.py"), "{r:?}");
}

#[test]
fn a_parent_relative_import_climbs_one_package() {
    let r = resolve(
        &[
            ("app/models.py", MODELS),
            (
                "app/api/views.py",
                "from ..models import Order\n\ndef create():\n    return Order()\n",
            ),
        ],
        "app/api/views.py",
        "Order",
    );
    assert_eq!(r.declared_in(), Some("app/models.py"), "{r:?}");
}

#[test]
fn a_package_resolves_through_its_init() {
    let r = resolve(
        &[
            ("app/models/__init__.py", MODELS),
            ("app/views.py", "from app.models import Order\n"),
        ],
        "app/views.py",
        "Order",
    );
    assert_eq!(r.declared_in(), Some("app/models/__init__.py"), "{r:?}");
}

#[test]
fn an_alias_resolves_to_the_exported_name() {
    let r = resolve(
        &[
            ("app/models.py", MODELS),
            (
                "app/views.py",
                "from app.models import Order as OrderModel\n",
            ),
        ],
        "app/views.py",
        "OrderModel",
    );
    assert_eq!(r.declared_in(), Some("app/models.py"), "{r:?}");
}

/// Onyx, `backend/onyx/db/search_settings.py`.
///
/// The module a use site imports from is often not the module that declares
/// the symbol; a package re-exports it. Refusing that would trade M07's false
/// positives for false negatives.
#[test]
fn a_re_export_is_followed_to_the_real_declaration() {
    let r = resolve(
        &[
            ("backend/onyx/db/models.py", MODELS),
            (
                "backend/onyx/db/search_settings.py",
                "from onyx.db.models import Order\n",
            ),
            (
                "backend/onyx/service.py",
                "from onyx.db.search_settings import Order\n",
            ),
        ],
        "backend/onyx/service.py",
        "Order",
    );
    assert_eq!(r.declared_in(), Some("backend/onyx/db/models.py"), "{r:?}");
    assert!(
        r.explain().contains("onyx.db.search_settings"),
        "{}",
        r.explain()
    );
}

#[test]
fn a_project_rooted_under_a_source_directory_still_resolves() {
    // Airflow declares `airflow.models` in airflow-core/src/airflow/models.py.
    let r = resolve(
        &[
            ("airflow-core/src/airflow/models.py", MODELS),
            (
                "airflow-core/src/airflow/api/routes.py",
                "from airflow.models import Order\n",
            ),
        ],
        "airflow-core/src/airflow/api/routes.py",
        "Order",
    );
    assert_eq!(
        r.declared_in(),
        Some("airflow-core/src/airflow/models.py"),
        "{r:?}"
    );
}

// ── Negative ────────────────────────────────────────────────────────

/// Airflow, `airflow-core/docs/img/diagram_*_architecture.py`.
///
/// The defect this whole module exists for: a third-party symbol sharing a
/// model's name. 218 of these were attributed to the Flask-AppBuilder model.
#[test]
fn a_third_party_module_is_unresolved_not_guessed() {
    let r = resolve(
        &[
            ("app/models.py", MODELS),
            (
                "docs/diagram.py",
                "from diagrams.onprem.client import Order\n",
            ),
        ],
        "docs/diagram.py",
        "Order",
    );
    let Resolution::Unresolved { module, .. } = &r else {
        panic!("expected Unresolved, got {r:?}");
    };
    assert_eq!(module, "diagrams.onprem.client");
    assert!(!r.is_definite());
}

#[test]
fn a_module_that_does_not_export_the_name_is_unresolved() {
    let r = resolve(
        &[
            ("app/models.py", MODELS),
            ("app/other.py", "SOMETHING = 1\n"),
            ("app/views.py", "from app.other import Order\n"),
        ],
        "app/views.py",
        "Order",
    );
    assert!(matches!(r, Resolution::Unresolved { .. }), "{r:?}");
}

#[test]
fn a_name_nothing_binds_is_reported_as_unbound() {
    // A star import, a conditional import or a builtin lands here. It is not
    // a refusal on its own — the caller decides.
    let r = resolve(
        &[(
            "app/views.py",
            "from app.models import *\n\ndef f():\n    return Order()\n",
        )],
        "app/views.py",
        "Order",
    );
    assert_eq!(r, Resolution::NotBound, "{r:?}");
}

#[test]
fn a_similar_module_path_does_not_satisfy_an_import() {
    // `ics.models` must not be answered by `analytics/models.py`. Matching is
    // anchored at a path separator.
    let r = resolve(
        &[
            ("analytics/models.py", MODELS),
            ("app/views.py", "from ics.models import Order\n"),
        ],
        "app/views.py",
        "Order",
    );
    assert!(matches!(r, Resolution::Unresolved { .. }), "{r:?}");
}

// ── Ambiguity ───────────────────────────────────────────────────────

#[test]
fn a_dotted_path_matching_two_files_is_refused() {
    // A monorepo where two source roots both contain `app/models.py`. Which
    // one `from app.models import Order` meant is unanswerable.
    let r = resolve(
        &[
            ("services/a/app/models.py", MODELS),
            ("services/b/app/models.py", MODELS),
            ("services/a/views.py", "from app.models import Order\n"),
        ],
        "services/a/views.py",
        "Order",
    );
    assert!(matches!(r, Resolution::Unresolved { .. }), "{r:?}");
    assert!(
        !r.is_definite(),
        "an unanswerable import must never be definite"
    );
}

#[test]
fn a_local_declaration_wins_over_any_import() {
    let r = resolve(
        &[
            ("app/models.py", MODELS),
            (
                "app/views.py",
                "from app.models import Order\n\nclass Order:\n    pass\n",
            ),
        ],
        "app/views.py",
        "Order",
    );
    assert!(matches!(r, Resolution::Local { .. }), "{r:?}");
}

#[test]
fn a_re_export_cycle_terminates() {
    let r = resolve(
        &[
            ("app/a.py", "from app.b import Order\n"),
            ("app/b.py", "from app.a import Order\n"),
            ("app/views.py", "from app.a import Order\n"),
        ],
        "app/views.py",
        "Order",
    );
    assert!(
        !r.is_definite(),
        "a cycle must not produce a declaration: {r:?}"
    );
}

// ── Evidence ────────────────────────────────────────────────────────

#[test]
fn every_resolution_explains_itself_without_quoting_source() {
    let files = [
        ("app/models.py", MODELS),
        ("app/views.py", "from app.models import Order\n"),
        (
            "docs/diagram.py",
            "from diagrams.onprem.client import Order\n",
        ),
    ];
    for (from, name) in [("app/views.py", "Order"), ("docs/diagram.py", "Order")] {
        let r = resolve(&files, from, name);
        let text = r.explain();
        assert!(!text.trim().is_empty(), "empty explanation for {from}");
        assert!(
            !text.contains("class Order") && !text.contains("db_table"),
            "evidence must not quote source (RULE 015): {text}"
        );
    }
}

// ── T-009: one name imported as two different things in one file ────

const TWO_FILTERS: [(&str, &str); 3] = [
    (
        "shop/models.py",
        "from django.db import models\n\n\nclass Filter(models.Model):\n    class Meta:\n        db_table = 'filters'\n",
    ),
    ("shop/params.py", "class Filter:\n    pass\n"),
    (
        "shop/tests.py",
        "def uses_the_model():\n    from shop.models import Filter\n    return Filter.objects.create()\n\n\ndef uses_the_other_class():\n    from shop.params import Filter\n    return Filter(value=1)\n",
    ),
];

#[test]
fn a_name_imported_as_two_different_things_is_ambiguous() {
    // Imports are recorded per file, not per function, so which `Filter` a
    // given use sees is not known. The first import must not win.
    let resolution = resolve(&TWO_FILTERS, "shop/tests.py", "Filter");
    assert!(
        matches!(&resolution, Resolution::Ambiguous { candidates } if candidates.len() == 2),
        "got {resolution:?}"
    );
}

#[test]
fn the_same_import_repeated_in_several_functions_is_one_binding() {
    let files = [
        TWO_FILTERS[0],
        (
            "shop/tests.py",
            "def a():\n    from shop.models import Filter\n    return Filter.objects.all()\n\n\ndef b():\n    from shop.models import Filter\n    return Filter.objects.count()\n",
        ),
    ];
    let resolution = resolve(&files, "shop/tests.py", "Filter");
    assert_eq!(
        resolution.declared_in(),
        Some("shop/models.py"),
        "got {resolution:?}"
    );
}

#[test]
fn two_aliases_of_one_declaration_do_not_conflict_with_each_other() {
    // `Filter` and `F` are different local names; each is bound once.
    let files = [
        TWO_FILTERS[0],
        (
            "shop/tests.py",
            "from shop.models import Filter\nfrom shop.models import Filter as F\n",
        ),
    ];
    for name in ["Filter", "F"] {
        let resolution = resolve(&files, "shop/tests.py", name);
        assert_eq!(
            resolution.declared_in(),
            Some("shop/models.py"),
            "{name}: {resolution:?}"
        );
    }
}

#[test]
fn one_module_imported_relatively_and_absolutely_is_one_binding() {
    // Found on Label Studio: `from .models import ConvertedFormat` at the top
    // of data_export/api.py and `from data_export.models import
    // ConvertedFormat` inside a function are the same class.
    let files = [
        TWO_FILTERS[0],
        (
            "shop/api.py",
            "from .models import Filter


def later():
    from shop.models import Filter
    return Filter.objects.all()
",
        ),
    ];
    let resolution = resolve(&files, "shop/api.py", "Filter");
    assert_eq!(
        resolution.declared_in(),
        Some("shop/models.py"),
        "got {resolution:?}"
    );
}

#[test]
fn a_module_imported_under_a_models_name_conflicts_with_it() {
    // Third review, U-11: `import shop.legacy as Order` rebinds `Order`.
    let files = [
        TWO_FILTERS[0],
        ("shop/legacy.py", "X = 1\n"),
        (
            "shop/views.py",
            "from shop.models import Filter\nimport shop.legacy as Filter\n\n\ndef v(request):\n    return Filter.objects.all()\n",
        ),
    ];
    let resolution = resolve(&files, "shop/views.py", "Filter");
    assert!(
        matches!(resolution, Resolution::Ambiguous { .. }),
        "got {resolution:?}"
    );
}
