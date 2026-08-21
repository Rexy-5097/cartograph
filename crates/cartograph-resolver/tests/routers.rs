//! Router prefix composition: the path a route is actually served on.

use cartograph_parser::Analyzer;
use cartograph_parser::model::{FileAnalysis, UrlObservation};
use cartograph_resolver::{ModuleIndex, PrefixResolution, RouterIndex, with_composed_prefix};

fn analyze(files: &[(&str, &str)]) -> Vec<FileAnalysis> {
    let mut a = Analyzer::new().expect("grammars load");
    files
        .iter()
        .map(|(p, s)| a.analyze_source(p, s).expect("fixture parses"))
        .collect()
}

/// The composed served path of the first route in `file`.
fn served(files: &[(&str, &str)], file: &str) -> Option<String> {
    let parsed = analyze(files);
    let modules = ModuleIndex::build(&parsed);
    let routers = RouterIndex::build(&parsed, &modules);
    let target = parsed.iter().find(|a| a.path == file)?;
    let route = target.routes.first()?;
    let receiver = route.receiver.as_ref()?;
    let resolution = routers.prefix_for(&target.path, receiver)?;
    let composed = with_composed_prefix(route, resolution)?;
    match composed.path {
        UrlObservation::Literal { value } => Some(value),
        other => Some(format!("{other:?}")),
    }
}

fn resolution(files: &[(&str, &str)], file: &str, router: &str) -> PrefixResolution {
    let parsed = analyze(files);
    let modules = ModuleIndex::build(&parsed);
    let routers = RouterIndex::build(&parsed, &modules);
    routers
        .prefix_for(file, router)
        .cloned()
        .unwrap_or(PrefixResolution::Unresolved {
            reason: "no such router".into(),
        })
}

// ── Positive ────────────────────────────────────────────────────────

#[test]
fn a_routers_own_prefix_is_applied() {
    let served = served(
        &[(
            "api/items.py",
            r#"
router = APIRouter(prefix="/items")

@router.get("/{id}")
def read_item(id):
    pass
"#,
        )],
        "api/items.py",
    );
    assert_eq!(served.as_deref(), Some("/items/{id}"));
}

/// Airflow: `pools_router` under `public_router`, decorator states `""`.
///
/// The acceptance target's fourth step, reduced. Two levels of composition
/// across two files, and a decorator whose own path is empty.
#[test]
fn a_nested_router_composes_across_files() {
    let served = served(
        &[
            (
                "api/routes/pools.py",
                r#"
pools_router = AirflowRouter(tags=["Pool"], prefix="/pools")

@pools_router.post("")
def post_pool(body):
    pass
"#,
            ),
            (
                "api/routes/__init__.py",
                r#"
from api.routes.pools import pools_router

public_router = AirflowRouter(prefix="/api/v2")
authenticated_router = AirflowRouter()

authenticated_router.include_router(pools_router)
public_router.include_router(authenticated_router)
"#,
            ),
        ],
        "api/routes/pools.py",
    );
    assert_eq!(
        served.as_deref(),
        Some("/api/v2/pools"),
        "the served path must compose every enclosing router"
    );
}

#[test]
fn a_prefix_stated_at_the_inclusion_site_is_composed() {
    let served = served(
        &[
            (
                "api/items.py",
                r#"
router = APIRouter()

@router.get("/list")
def read_items():
    pass
"#,
            ),
            (
                "api/main.py",
                r#"
from api.items import router

app_router = APIRouter(prefix="/api/v1")
app_router.include_router(router, prefix="/items")
"#,
            ),
        ],
        "api/items.py",
    );
    assert_eq!(served.as_deref(), Some("/api/v1/items/list"));
}

#[test]
fn a_trailing_slash_survives_composition() {
    // M03 keeps `/items` and `/items/` distinct; composition must not blur
    // them.
    let served = served(
        &[(
            "api/items.py",
            r#"
router = APIRouter(prefix="/items")

@router.get("/")
def read_items():
    pass
"#,
        )],
        "api/items.py",
    );
    assert_eq!(served.as_deref(), Some("/items/"));
}

#[test]
fn a_templated_route_path_keeps_its_unknown_parts() {
    let files = [(
        "api/items.py",
        r#"
router = APIRouter(prefix="/items")

@router.get(f"/{PREFIX}/detail")
def detail():
    pass
"#,
    )];
    let parsed = analyze(&files);
    let modules = ModuleIndex::build(&parsed);
    let routers = RouterIndex::build(&parsed, &modules);
    let target = &parsed[0];
    if let Some(route) = target.routes.first() {
        if let Some(receiver) = &route.receiver {
            if let Some(r) = routers.prefix_for(&target.path, receiver) {
                if let Some(composed) = with_composed_prefix(route, r) {
                    assert!(
                        !matches!(composed.path, UrlObservation::Literal { .. }),
                        "an interpolated path must not become a literal: {:?}",
                        composed.path
                    );
                }
            }
        }
    }
}

// ── Negative ────────────────────────────────────────────────────────

#[test]
fn a_dynamic_prefix_refuses_rather_than_composing_as_empty() {
    let r = resolution(
        &[(
            "api/items.py",
            r"
router = APIRouter(prefix=settings.API_PREFIX)

@router.get('/list')
def read_items():
    pass
",
        )],
        "api/items.py",
        "router",
    );
    assert!(
        matches!(r, PrefixResolution::Unresolved { .. }),
        "a prefix that is not a literal must not compose: {r:?}"
    );
    assert!(r.known().is_none());
}

#[test]
fn a_dynamic_prefix_at_the_inclusion_site_also_refuses() {
    let r = resolution(
        &[
            ("api/items.py", "router = APIRouter()\n"),
            (
                "api/main.py",
                r"
from api.items import router

app_router = APIRouter(prefix='/api')
app_router.include_router(router, prefix=settings.ITEMS)
",
            ),
        ],
        "api/items.py",
        "router",
    );
    assert!(matches!(r, PrefixResolution::Unresolved { .. }), "{r:?}");
}

#[test]
fn an_unresolved_prefix_leaves_the_route_path_untouched() {
    // The safety property: composition can only make a path more complete.
    let served = served(
        &[(
            "api/items.py",
            r"
router = APIRouter(prefix=settings.API_PREFIX)

@router.get('/list')
def read_items():
    pass
",
        )],
        "api/items.py",
    );
    assert_eq!(served, None, "no composed path is produced at all");
}

#[test]
fn a_receiver_that_names_no_router_composes_nothing() {
    let files = [(
        "api/items.py",
        r"
@some_object.get('/list')
def read_items():
    pass
",
    )];
    let parsed = analyze(&files);
    let modules = ModuleIndex::build(&parsed);
    let routers = RouterIndex::build(&parsed, &modules);
    assert!(
        routers.prefix_for("api/items.py", "some_object").is_none(),
        "an undeclared receiver must not resolve to a prefix"
    );
}

// ── Ambiguity ───────────────────────────────────────────────────────

#[test]
fn a_router_mounted_twice_is_ambiguous_and_composes_nothing() {
    let r = resolution(
        &[
            ("api/items.py", "router = APIRouter(prefix='/items')\n"),
            (
                "api/main.py",
                r"
from api.items import router

v1 = APIRouter(prefix='/api/v1')
v2 = APIRouter(prefix='/api/v2')
v1.include_router(router)
v2.include_router(router)
",
            ),
        ],
        "api/items.py",
        "router",
    );
    assert!(
        matches!(r, PrefixResolution::Ambiguous { parents: 2 }),
        "a router served on two paths must not be given one: {r:?}"
    );
    assert!(r.known().is_none());
}

#[test]
fn an_inclusion_cycle_terminates_without_a_prefix() {
    let r = resolution(
        &[(
            "api/main.py",
            r"
a = APIRouter(prefix='/a')
b = APIRouter(prefix='/b')
a.include_router(b)
b.include_router(a)
",
        )],
        "api/main.py",
        "a",
    );
    assert!(
        r.known().is_none(),
        "a cycle must not produce a prefix: {r:?}"
    );
}

// ── Evidence ────────────────────────────────────────────────────────

#[test]
fn a_composed_prefix_explains_the_chain_it_came_from() {
    let r = resolution(
        &[
            (
                "api/routes/pools.py",
                "pools_router = AirflowRouter(prefix='/pools')\n",
            ),
            (
                "api/routes/__init__.py",
                r"
from api.routes.pools import pools_router

public_router = AirflowRouter(prefix='/api/v2')
public_router.include_router(pools_router)
",
            ),
        ],
        "api/routes/pools.py",
        "pools_router",
    );
    assert_eq!(r.known(), Some("/api/v2/pools"));
    let text = r.explain();
    assert!(text.contains("public_router"), "{text}");
    assert!(text.contains("pools_router"), "{text}");
}
