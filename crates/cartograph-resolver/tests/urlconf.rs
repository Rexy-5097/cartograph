//! Django URL-conf composition: the path an entry is actually served on.
//!
//! Recovery programme F-002 (2026-10-30 assessment, F4). The fixtures are the
//! shapes of real URL confs, named where they came from.

use cartograph_parser::Analyzer;
use cartograph_parser::model::{FileAnalysis, UrlObservation};
use cartograph_resolver::{
    MatchStatus, ModuleIndex, PrefixResolution, RouteIndex, UrlConfIndex, match_client,
    normalize_client_call, normalize_route_declaration,
};

fn analyze(files: &[(&str, &str)]) -> Vec<FileAnalysis> {
    let mut a = Analyzer::new().expect("grammars load");
    files
        .iter()
        .map(|(p, s)| a.analyze_source(p, s).expect("fixture parses"))
        .collect()
}

/// Every route in the project as `(handler, served path)`, sorted.
fn served(files: &[(&str, &str)]) -> Vec<(String, String)> {
    let parsed = analyze(files);
    let modules = ModuleIndex::build(&parsed);
    let index = UrlConfIndex::build(&parsed, &modules);
    let mut out: Vec<(String, String)> = parsed
        .iter()
        .flat_map(|f| {
            f.routes.iter().map(|r| {
                let route = index.served(&f.path, r).unwrap_or_else(|| r.clone());
                let path = match route.path {
                    UrlObservation::Literal { value } => value,
                    other => format!("{other:?}"),
                };
                (route.handler.unwrap_or_default(), path)
            })
        })
        .collect();
    out.sort();
    out
}

fn path_of(served: &[(String, String)], handler: &str) -> String {
    served
        .iter()
        .find(|(h, _)| h == handler)
        .unwrap_or_else(|| panic!("no route for {handler}: {served:?}"))
        .1
        .clone()
}

/// Plane's shape: a root conf, string includes, and a package whose
/// `__init__` spreads its submodules' patterns.
const PLANE: &[(&str, &str)] = &[
    (
        "apps/api/plane/urls.py",
        r#"
from django.urls import include, path

urlpatterns = [
    path("api/", include("plane.app.urls")),
    path("api/instances/", include("plane.license.urls")),
]
"#,
    ),
    (
        "apps/api/plane/license/urls.py",
        r#"
from django.urls import path
from plane.license.api.views import InstanceEndpoint, InstanceAdminEndpoint

urlpatterns = [
    path("", InstanceEndpoint.as_view(), name="instance"),
    path("admins/", InstanceAdminEndpoint.as_view()),
]
"#,
    ),
    (
        "apps/api/plane/app/urls/__init__.py",
        r"
from .analytic import urlpatterns as analytic_urls

urlpatterns = [
    *analytic_urls,
]
",
    ),
    (
        "apps/api/plane/app/urls/analytic.py",
        r#"
from django.urls import path

urlpatterns = [
    path("workspaces/<str:slug>/analytics/", AnalyticsEndpoint.as_view()),
]
"#,
    ),
];

// ── Positive ────────────────────────────────────────────────────────

#[test]
fn a_string_include_prefixes_the_included_modules_entries() {
    let s = served(PLANE);
    assert_eq!(path_of(&s, "InstanceEndpoint.as_view"), "api/instances/");
    assert_eq!(
        path_of(&s, "InstanceAdminEndpoint.as_view"),
        "api/instances/admins/"
    );
}

#[test]
fn spread_patterns_inherit_the_prefix_of_the_list_they_are_spread_into() {
    let s = served(PLANE);
    assert_eq!(
        path_of(&s, "AnalyticsEndpoint.as_view"),
        "api/workspaces/<str:slug>/analytics/"
    );
}

/// The defect itself, end to end: the frontend call reaches the view, and
/// nothing reaches a mount.
#[test]
fn a_frontend_call_under_a_mount_matches_the_view_not_the_include() {
    let mut files = PLANE.to_vec();
    files.push((
        "apps/web/services/instance.service.ts",
        r#"
import axios from "axios";
export async function getInstanceInfo() {
  return axios.get("/api/instances/");
}
"#,
    ));
    let parsed = analyze(&files);
    let modules = ModuleIndex::build(&parsed);
    let index = UrlConfIndex::build(&parsed, &modules);
    let routes = RouteIndex::build(parsed.iter().flat_map(|f| {
        f.routes.iter().map(|r| {
            let r = index.served(&f.path, r).unwrap_or_else(|| r.clone());
            normalize_route_declaration(&r, &f.path, f.language)
        })
    }));
    let web = parsed
        .iter()
        .find(|f| f.path == "apps/web/services/instance.service.ts")
        .expect("web file");
    let call = normalize_client_call(&web.http_calls[0], &web.path, web.language);
    let result = match_client(&call, &routes);
    assert_eq!(result.status, MatchStatus::Strong, "{result:?}");
    assert_eq!(result.candidates.len(), 1, "{result:?}");
    assert_eq!(
        result.candidates[0].route.provenance.handler.as_deref(),
        Some("InstanceEndpoint.as_view")
    );
}

#[test]
fn mounts_compose_through_every_level_by_plain_concatenation() {
    let s = served(&[
        (
            "proj/urls.py",
            r#"urlpatterns = [path("api/", include("proj.api.urls"))]"#,
        ),
        (
            "proj/api/urls.py",
            r#"urlpatterns = [path("v1", include("proj.api.v1"))]"#,
        ),
        (
            "proj/api/v1.py",
            r#"urlpatterns = [path("/items/", views.items)]"#,
        ),
    ]);
    assert_eq!(
        path_of(&s, "views.items"),
        "api/v1/items/",
        "Django concatenates; no separator is added or dropped"
    );
}

#[test]
fn an_imported_module_and_an_imported_list_can_be_included_by_name() {
    let s = served(&[
        (
            "proj/urls.py",
            r#"
from proj.shop import urls as shop_urls
from proj.blog.urls import extra_patterns

urlpatterns = [
    path("shop/", include(shop_urls)),
    path("blog/", include(extra_patterns)),
]
"#,
        ),
        (
            "proj/shop/urls.py",
            r#"urlpatterns = [path("cart/", views.cart)]"#,
        ),
        (
            "proj/blog/urls.py",
            r#"
extra_patterns = [path("feed/", views.feed)]
urlpatterns = [path("post/", views.post)]
"#,
        ),
    ]);
    assert_eq!(path_of(&s, "views.cart"), "shop/cart/");
    assert_eq!(path_of(&s, "views.feed"), "blog/feed/");
    assert_eq!(
        path_of(&s, "views.post"),
        "post/",
        "only the list that was included moves; the rest of its module does not"
    );
}

/// The Django documentation's own forms: an inline list, a nested inline
/// list, and a list declared in the same module.
#[test]
fn inline_and_local_lists_are_prefixed_where_they_are_mounted() {
    let s = served(&[(
        "proj/urls.py",
        r#"
extra = [path("reports/", views.reports)]

urlpatterns = [
    path("admin-api/", include(extra)),
    path("billing/", include([
        path("invoices/", views.invoices),
        path("v2/", include([path("credits/", views.credits)])),
    ])),
    path("health/", views.health),
]
"#,
    )]);
    assert_eq!(path_of(&s, "views.reports"), "admin-api/reports/");
    assert_eq!(path_of(&s, "views.invoices"), "billing/invoices/");
    assert_eq!(path_of(&s, "views.credits"), "billing/v2/credits/");
    assert_eq!(path_of(&s, "views.health"), "health/");
}

// ── Negative and ambiguous: refuse, and keep the entry as written ───

#[test]
fn a_module_mounted_twice_is_ambiguous_and_keeps_its_written_path() {
    let files = [
        (
            "proj/urls.py",
            r#"
urlpatterns = [
    path("api/", include("proj.items")),
    path("api/v1/", include("proj.items")),
]
"#,
        ),
        (
            "proj/items.py",
            r#"urlpatterns = [path("items/", views.items)]"#,
        ),
    ];
    assert_eq!(path_of(&served(&files), "views.items"), "items/");
    let parsed = analyze(&files);
    let modules = ModuleIndex::build(&parsed);
    let index = UrlConfIndex::build(&parsed, &modules);
    let route = &parsed[1].routes[0];
    assert_eq!(
        index.prefix_for("proj/items.py", route.span),
        PrefixResolution::Ambiguous { parents: 2 }
    );
}

#[test]
fn a_regex_or_variable_prefix_is_unresolved_never_empty() {
    let files = [
        (
            "proj/urls.py",
            r#"
urlpatterns = [
    re_path(r"^api/", include("proj.items")),
    path(PREFIX, include("proj.orders")),
]
"#,
        ),
        (
            "proj/items.py",
            r#"urlpatterns = [path("items/", views.items)]"#,
        ),
        (
            "proj/orders.py",
            r#"urlpatterns = [path("orders/", views.orders)]"#,
        ),
    ];
    let parsed = analyze(&files);
    let modules = ModuleIndex::build(&parsed);
    let index = UrlConfIndex::build(&parsed, &modules);
    for (file, at) in [(1, "proj/items.py"), (2, "proj/orders.py")] {
        let route = &parsed[file].routes[0];
        assert!(
            matches!(
                index.prefix_for(at, route.span),
                PrefixResolution::Unresolved { .. }
            ),
            "{at}"
        );
        assert!(
            index.served(at, route).is_none(),
            "{at} must keep its written path"
        );
    }
}

#[test]
fn an_unmodelled_target_mounts_nothing_and_a_regex_entry_is_never_composed() {
    let s = served(&[
        (
            "proj/urls.py",
            r#"
urlpatterns = [
    path("drf/", include(router.urls)),
    path("api/", include("proj.items")),
]
"#,
        ),
        (
            "proj/items.py",
            r#"urlpatterns = [re_path(r"^items/(?P<pk>[0-9]+)/$", views.item)]"#,
        ),
    ]);
    assert_eq!(
        path_of(&s, "views.item"),
        "^items/(?P<pk>[0-9]+)/$",
        "a regex entry is not interpreted, so no prefix is glued onto it"
    );
}

#[test]
fn a_mount_cycle_refuses() {
    let files = [
        (
            "proj/a.py",
            r#"urlpatterns = [path("a/", include("proj.b")), path("x/", views.x)]"#,
        ),
        (
            "proj/b.py",
            r#"urlpatterns = [path("b/", include("proj.a"))]"#,
        ),
    ];
    let parsed = analyze(&files);
    let modules = ModuleIndex::build(&parsed);
    let index = UrlConfIndex::build(&parsed, &modules);
    let route = &parsed[0].routes[0];
    assert!(matches!(
        index.prefix_for("proj/a.py", route.span),
        PrefixResolution::Unresolved { .. }
    ));
}
