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

// ── Aliases (out of sample: polarsource/polar) ──────────────────────
//
// Found on polarsource/polar `773cfcf`, a repository the authors never
// tested: `from polar.customer.endpoints import router as customer_router`
// was looked up in the declaring module by its alias, `customer_router`, so
// the inclusion and its `/v1` prefix were dropped. 380 of 401 front-end
// `/v1/…` calls then matched nothing. Airflow names its routers uniquely,
// which is why the benchmark never caught it.

/// The Polar shape: a router imported under an alias.
#[test]
fn an_aliased_router_is_found_by_the_name_its_module_declares() {
    let served = served(
        &[
            (
                "polar/customer/endpoints.py",
                r#"
router = APIRouter(prefix="/customers")

@router.get("/{id}")
def get_customer(id):
    pass
"#,
            ),
            (
                "polar/api.py",
                r#"
from polar.customer.endpoints import router as customer_router

router = APIRouter(prefix="/v1")
router.include_router(customer_router)
"#,
            ),
        ],
        "polar/customer/endpoints.py",
    );
    assert_eq!(served.as_deref(), Some("/v1/customers/{id}"));
}

/// Not yet: a prefix passed to the application.
///
/// `app = FastAPI()` is not recorded as a router, so `app.include_router(…,
/// prefix=…)` composes nothing, and the route keeps the prefix its own router
/// states rather than a guessed one (RULE 009). Counting every `FastAPI()` as
/// a parent was tried and measured: test files build their own applications
/// and mount production routers on them, so those routers looked mounted
/// twice. On Significant-Gravitas/AutoGPT `601093d` that cost 15 correct
/// edges (8 became ambiguous, 7 matched nothing) for 14 gained. Composing the
/// application's prefix needs a model of which application serves which
/// call; until then this pins the refusal.
#[test]
fn a_prefix_passed_to_the_application_is_not_yet_composed() {
    let served = served(
        &[
            (
                "app/users.py",
                r#"
router = APIRouter(prefix="/users")

@router.get("/me")
def read_me():
    pass
"#,
            ),
            (
                "app/main.py",
                r#"
from fastapi import FastAPI
from app.users import router as users_router

app = FastAPI()
app.include_router(users_router, prefix="/api/v1")
"#,
            ),
        ],
        "app/users.py",
    );
    assert_eq!(
        served.as_deref(),
        Some("/users/me"),
        "the router's own prefix, and nothing guessed beyond it",
    );
}

/// An alias names one router; the module's other router is not it.
#[test]
fn an_alias_resolves_to_the_router_it_names_not_another_in_its_module() {
    let files = [
        (
            "api/orders.py",
            r#"
router = APIRouter(prefix="/orders")
admin_router = APIRouter(prefix="/admin/orders")
"#,
        ),
        (
            "api/main.py",
            r#"
from api.orders import admin_router as orders_admin

root = APIRouter(prefix="/api")
root.include_router(orders_admin)
"#,
        ),
    ];
    assert_eq!(
        resolution(&files, "api/orders.py", "admin_router").known(),
        Some("/api/admin/orders"),
    );
    assert_eq!(
        resolution(&files, "api/orders.py", "router").known(),
        Some("/orders"),
        "the router the alias does not name stays unmounted",
    );
}

/// An alias at the inclusion site, through a package that re-exports the
/// router under an alias of its own.
#[test]
fn an_alias_through_an_aliasing_re_export_reaches_the_declaration() {
    let served = served(
        &[
            (
                "shop/cart/endpoints.py",
                r#"
router = APIRouter(prefix="/cart")

@router.post("")
def add_to_cart(item):
    pass
"#,
            ),
            (
                "shop/cart/__init__.py",
                "from shop.cart.endpoints import router as cart_router\n",
            ),
            (
                "shop/api.py",
                r#"
from shop.cart import cart_router as cart

api = APIRouter(prefix="/v2")
api.include_router(cart)
"#,
            ),
        ],
        "shop/cart/endpoints.py",
    );
    assert_eq!(served.as_deref(), Some("/v2/cart"));
}
