//! T-002: a URL-conf entry's handler reference resolves to its declaration.
//!
//! `path("orders/", views.list_orders)` in `urls.py` names a function declared
//! in `views.py`. The request edge used to end at a node named after the
//! reference, in `urls.py`; the handler's model accesses start at the function
//! in `views.py`. These tests pin the resolution, each refusal, and the
//! connected chain.

use cartograph_core::{EdgeKind, NodeKind};
use cartograph_graph::ArchitectureGraph;
use cartograph_parser::Analyzer;
use cartograph_parser::model::FileAnalysis;
use cartograph_resolver::{
    HandlerIndex, ModuleIndex, OrmAnalysis, RouteIndex, UrlConfIndex, add_edge_for_match_with,
    add_orm_edges, match_client, normalize_client_call, normalize_route_declaration,
};

fn analyse(files: &[(&str, &str)]) -> Vec<FileAnalysis> {
    let mut analyzer = Analyzer::new().expect("grammars load");
    files
        .iter()
        .map(|(path, src)| analyzer.analyze_source(path, src).expect("fixture parses"))
        .collect()
}

/// `(kind, name, file)` of the declaration behind `reference` in `url_conf`.
fn declared(
    files: &[(&str, &str)],
    url_conf: &str,
    reference: &str,
) -> Option<(NodeKind, String, String)> {
    let analyses = analyse(files);
    let modules = ModuleIndex::build(&analyses);
    let handlers = HandlerIndex::build(&analyses, &modules);
    handlers
        .declaration(url_conf, reference)
        .map(|d| (d.kind, d.name.clone(), d.file.clone()))
}

fn function(name: &str, file: &str) -> (NodeKind, String, String) {
    (NodeKind::Function, name.to_owned(), file.to_owned())
}

fn class(name: &str, file: &str) -> (NodeKind, String, String) {
    (NodeKind::Class, name.to_owned(), file.to_owned())
}

const VIEWS: &str = "
from django.views import View
from .models import Order


def list_orders(request):
    return Order.objects.all()


class OrderView(View):
    def get(self, request):
        return Order.objects.all()


orders_alias = OrderView.as_view()
";

const MODELS: &str = "
from django.db import models


class Order(models.Model):
    class Meta:
        db_table = 'orders'
";

// ── every way a URL conf names a function view ──────────────────────

#[test]
fn a_view_named_through_its_module_resolves_to_the_function() {
    let files = [
        ("shop/views.py", VIEWS),
        (
            "shop/urls.py",
            "from django.urls import path\nfrom . import views\n\nurlpatterns = [path('orders/', views.list_orders)]\n",
        ),
    ];
    assert_eq!(
        declared(&files, "shop/urls.py", "views.list_orders"),
        Some(function("list_orders", "shop/views.py"))
    );
}

#[test]
fn a_view_imported_by_name_resolves_to_the_function() {
    let files = [
        ("shop/views.py", VIEWS),
        (
            "shop/urls.py",
            "from django.urls import path\nfrom .views import list_orders\n\nurlpatterns = [path('orders/', list_orders)]\n",
        ),
    ];
    assert_eq!(
        declared(&files, "shop/urls.py", "list_orders"),
        Some(function("list_orders", "shop/views.py"))
    );
}

#[test]
fn an_aliased_view_resolves_to_the_name_its_module_declares() {
    let files = [
        ("shop/views.py", VIEWS),
        (
            "shop/urls.py",
            "from django.urls import path\nfrom .views import list_orders as orders_view\n\nurlpatterns = [path('orders/', orders_view)]\n",
        ),
    ];
    assert_eq!(
        declared(&files, "shop/urls.py", "orders_view"),
        Some(function("list_orders", "shop/views.py"))
    );
}

#[test]
fn every_module_import_form_resolves() {
    for (import, reference) in [
        ("from shop import views", "views.list_orders"),
        ("from shop import views as v", "v.list_orders"),
        ("import shop.views", "shop.views.list_orders"),
        ("import shop.views as sv", "sv.list_orders"),
        ("from . import views as v", "v.list_orders"),
    ] {
        let url_conf = format!(
            "from django.urls import path\n{import}\n\nurlpatterns = [path('orders/', {reference})]\n"
        );
        let files = [
            ("shop/views.py", VIEWS),
            ("shop/urls.py", url_conf.as_str()),
        ];
        assert_eq!(
            declared(&files, "shop/urls.py", reference),
            Some(function("list_orders", "shop/views.py")),
            "`{import}` then `{reference}`"
        );
    }
}

#[test]
fn a_view_declared_in_the_url_conf_itself_resolves_there() {
    let files = [(
        "shop/urls.py",
        "from django.urls import path\n\n\ndef health(request):\n    return None\n\n\nurlpatterns = [path('health/', health)]\n",
    )];
    assert_eq!(
        declared(&files, "shop/urls.py", "health"),
        Some(function("health", "shop/urls.py"))
    );
}

#[test]
fn a_view_re_exported_by_a_views_package_resolves_to_its_real_file() {
    let files = [
        (
            "shop/views/__init__.py",
            "from .orders import list_orders\n",
        ),
        (
            "shop/views/orders.py",
            "def list_orders(request):\n    return None\n",
        ),
        (
            "shop/urls.py",
            "from django.urls import path\nfrom . import views\n\nurlpatterns = [path('orders/', views.list_orders)]\n",
        ),
    ];
    assert_eq!(
        declared(&files, "shop/urls.py", "views.list_orders"),
        Some(function("list_orders", "shop/views/orders.py"))
    );
}

// ── class-based views ───────────────────────────────────────────────

#[test]
fn a_class_based_view_resolves_to_its_class() {
    let by_module = [
        ("shop/views.py", VIEWS),
        (
            "shop/urls.py",
            "from django.urls import path\nfrom . import views\n\nurlpatterns = [path('orders/', views.OrderView.as_view())]\n",
        ),
    ];
    assert_eq!(
        declared(&by_module, "shop/urls.py", "views.OrderView.as_view"),
        Some(class("OrderView", "shop/views.py"))
    );

    let by_name = [
        ("shop/views.py", VIEWS),
        (
            "shop/urls.py",
            "from django.urls import path\nfrom .views import OrderView\n\nurlpatterns = [path('orders/', OrderView.as_view(template_name='x'))]\n",
        ),
    ];
    assert_eq!(
        declared(&by_name, "shop/urls.py", "OrderView.as_view"),
        Some(class("OrderView", "shop/views.py"))
    );
}

// ── refusals ────────────────────────────────────────────────────────

#[test]
fn a_wrapper_call_names_no_handler_at_all() {
    // `login_required(views.list_orders)`: what the wrapper returns is not
    // known, and a decorator declared in this project must never be taken
    // for the view it wraps.
    let files = [
        ("shop/views.py", VIEWS),
        (
            "shop/decorators.py",
            "def staff_only(view):\n    return view\n",
        ),
        (
            "shop/urls.py",
            "from django.urls import path\nfrom django.contrib.auth.decorators import login_required\nfrom . import views\nfrom .decorators import staff_only\n\nurlpatterns = [\n    path('a/', login_required(views.list_orders)),\n    path('b/', staff_only(views.list_orders)),\n    path('c/', staff_only(views.OrderView.as_view())),\n]\n",
        ),
    ];
    let analyses = analyse(&files);
    let url_conf = analyses.iter().find(|f| f.path == "shop/urls.py").unwrap();
    assert_eq!(url_conf.routes.len(), 3);
    assert!(
        url_conf.routes.iter().all(|r| r.handler.is_none()),
        "a wrapper call must not be recorded as a handler: {:?}",
        url_conf.routes
    );

    let modules = ModuleIndex::build(&analyses);
    let handlers = HandlerIndex::build(&analyses, &modules);
    assert!(handlers.is_empty());
    assert_eq!(handlers.declaration("shop/urls.py", "staff_only"), None);
}

#[test]
fn a_reference_that_does_not_lead_to_one_declaration_resolves_to_nothing() {
    // A module outside the project.
    let external = [(
        "shop/urls.py",
        "from django.urls import path\nfrom thirdparty import views\n\nurlpatterns = [path('x/', views.index)]\n",
    )];
    assert_eq!(declared(&external, "shop/urls.py", "views.index"), None);

    // A name the module does not declare.
    let missing = [
        ("shop/views.py", VIEWS),
        (
            "shop/urls.py",
            "from django.urls import path\nfrom . import views\n\nurlpatterns = [path('x/', views.no_such_view)]\n",
        ),
    ];
    assert_eq!(
        declared(&missing, "shop/urls.py", "views.no_such_view"),
        None
    );

    // Nothing in the URL conf binds the name.
    let unbound = [
        ("shop/views.py", VIEWS),
        (
            "shop/urls.py",
            "from django.urls import path\n\nurlpatterns = [path('x/', views.list_orders)]\n",
        ),
    ];
    assert_eq!(
        declared(&unbound, "shop/urls.py", "views.list_orders"),
        None
    );
}

#[test]
fn two_modules_that_could_both_be_meant_resolve_to_nothing() {
    // An absolute `shop.views` matches two files in this monorepo.
    let files = [
        (
            "a/shop/views.py",
            "def list_orders(request):\n    return None\n",
        ),
        (
            "b/shop/views.py",
            "def list_orders(request):\n    return None\n",
        ),
        (
            "c/urls.py",
            "from django.urls import path\nfrom shop import views\n\nurlpatterns = [path('orders/', views.list_orders)]\n",
        ),
    ];
    assert_eq!(declared(&files, "c/urls.py", "views.list_orders"), None);
}

#[test]
fn a_name_that_is_not_a_function_or_class_resolves_to_nothing() {
    // `orders_alias = OrderView.as_view()` is a module-scope variable.
    let files = [
        ("shop/views.py", VIEWS),
        (
            "shop/urls.py",
            "from django.urls import path\nfrom . import views\n\nurlpatterns = [path('x/', views.orders_alias)]\n",
        ),
    ];
    assert_eq!(declared(&files, "shop/urls.py", "views.orders_alias"), None);
}

#[test]
fn as_view_on_a_function_and_a_bare_class_resolve_to_nothing() {
    let files = [
        ("shop/views.py", VIEWS),
        (
            "shop/urls.py",
            "from django.urls import path\nfrom . import views\n\nurlpatterns = [\n    path('a/', views.list_orders.as_view()),\n    path('b/', views.OrderView),\n]\n",
        ),
    ];
    assert_eq!(
        declared(&files, "shop/urls.py", "views.list_orders.as_view"),
        None
    );
    assert_eq!(declared(&files, "shop/urls.py", "views.OrderView"), None);
}

#[test]
fn a_same_named_view_in_another_module_is_not_confused_with_the_imported_one() {
    let files = [
        ("shop/views.py", VIEWS),
        (
            "billing/views.py",
            "def list_orders(request):\n    return None\n",
        ),
        (
            "shop/urls.py",
            "from django.urls import path\nfrom . import views\n\nurlpatterns = [path('orders/', views.list_orders)]\n",
        ),
        (
            "billing/urls.py",
            "from django.urls import path\nfrom . import views\n\nurlpatterns = [path('orders/', views.list_orders)]\n",
        ),
    ];
    assert_eq!(
        declared(&files, "shop/urls.py", "views.list_orders"),
        Some(function("list_orders", "shop/views.py"))
    );
    assert_eq!(
        declared(&files, "billing/urls.py", "views.list_orders"),
        Some(function("list_orders", "billing/views.py"))
    );
}

// ── the chain ───────────────────────────────────────────────────────

/// Builds the graph the pipeline builds: request edges, then model edges.
fn graph_of(files: &[(&str, &str)]) -> ArchitectureGraph {
    let analyses = analyse(files);
    let modules = ModuleIndex::build(&analyses);
    let urlconfs = UrlConfIndex::build(&analyses, &modules);
    let handlers = HandlerIndex::build(&analyses, &modules);

    let routes = analyses.iter().flat_map(|file| {
        let urlconfs = &urlconfs;
        file.routes.iter().map(move |route| {
            let served = urlconfs.served(&file.path, route);
            normalize_route_declaration(served.as_ref().unwrap_or(route), &file.path, file.language)
        })
    });
    let index = RouteIndex::build(routes);

    let mut graph = ArchitectureGraph::new();
    for file in &analyses {
        for call in &file.http_calls {
            let client = normalize_client_call(call, &file.path, file.language);
            let result = match_client(&client, &index);
            add_edge_for_match_with(&mut graph, &result, None, &handlers).expect("endpoints");
        }
    }
    let orm = OrmAnalysis::build(&analyses);
    add_orm_edges(&mut graph, &orm, None).expect("endpoints");
    graph
}

/// The names along the one path that follows `kinds` from `start`.
fn walk(graph: &ArchitectureGraph, start: &str, kinds: &[EdgeKind]) -> Vec<String> {
    let mut node = graph
        .nodes()
        .find(|n| n.name() == start)
        .unwrap_or_else(|| panic!("no node named {start}"))
        .id();
    let mut names = vec![start.to_owned()];
    for kind in kinds {
        let next: Vec<_> = graph
            .edges()
            .filter(|e| e.source() == node && e.kind() == *kind)
            .map(cartograph_core::Edge::target)
            .collect();
        assert_eq!(next.len(), 1, "expected one {kind:?} edge after {names:?}");
        node = next[0];
        names.push(graph.node(node).unwrap().name().to_owned());
    }
    names
}

const CLIENT: &str = "export async function loadOrders() {\n  return fetch('/api/orders/');\n}\n";

#[test]
fn a_request_reaches_the_table_through_a_view_in_another_file() {
    let graph = graph_of(&[
        ("web/api.ts", CLIENT),
        ("shop/models.py", MODELS),
        ("shop/views.py", VIEWS),
        (
            "shop/urls.py",
            "from django.urls import path\nfrom . import views\n\nurlpatterns = [path('api/orders/', views.list_orders)]\n",
        ),
    ]);
    assert_eq!(
        walk(
            &graph,
            "loadOrders",
            &[EdgeKind::HttpCall, EdgeKind::OrmAccess, EdgeKind::Queries]
        ),
        ["loadOrders", "list_orders", "Order", "orders"]
    );
    // The request ends at the function in views.py, and no node is left in
    // urls.py named after the reference.
    let handler = graph.nodes().find(|n| n.name() == "list_orders").unwrap();
    assert_eq!(handler.location().unwrap().file(), "shop/views.py");
    assert!(graph.nodes().all(|n| n.name() != "views.list_orders"));
    assert_eq!(
        graph.nodes().filter(|n| n.name() == "list_orders").count(),
        1,
        "one handler node, not one per file"
    );
}

#[test]
fn the_chain_holds_through_a_nested_url_conf() {
    let graph = graph_of(&[
        ("web/api.ts", CLIENT),
        ("shop/models.py", MODELS),
        ("shop/views.py", VIEWS),
        (
            "shop/urls.py",
            "from django.urls import path\nfrom .views import list_orders\n\nurlpatterns = [path('orders/', list_orders)]\n",
        ),
        (
            "project/urls.py",
            "from django.urls import include, path\n\nurlpatterns = [path('api/', include('shop.urls'))]\n",
        ),
    ]);
    assert_eq!(
        walk(
            &graph,
            "loadOrders",
            &[EdgeKind::HttpCall, EdgeKind::OrmAccess, EdgeKind::Queries]
        ),
        ["loadOrders", "list_orders", "Order", "orders"]
    );
}

#[test]
fn a_class_based_view_is_reached_but_its_methods_are_not_joined_to_it() {
    // The request reaches the class. `get` is a separate node, shared by every
    // class in the file that defines one, so no edge is drawn from the class
    // to it: that would also claim the other classes' model accesses.
    let graph = graph_of(&[
        ("web/api.ts", CLIENT),
        ("shop/models.py", MODELS),
        ("shop/views.py", VIEWS),
        (
            "shop/urls.py",
            "from django.urls import path\nfrom . import views\n\nurlpatterns = [path('api/orders/', views.OrderView.as_view())]\n",
        ),
    ]);
    assert_eq!(
        walk(&graph, "loadOrders", &[EdgeKind::HttpCall]),
        ["loadOrders", "OrderView"]
    );
    let view = graph.nodes().find(|n| n.name() == "OrderView").unwrap();
    assert_eq!(view.kind(), NodeKind::Class);
    assert_eq!(view.location().unwrap().file(), "shop/views.py");
    assert_eq!(
        graph.edges().filter(|e| e.source() == view.id()).count(),
        0,
        "no edge is invented from the class to a method"
    );
}

#[test]
fn an_unresolved_reference_ends_at_its_route() {
    // Second review, S-01: a node named by the reference text would share
    // the identity of any function of that name in the URL conf.
    let graph = graph_of(&[
        ("web/api.ts", CLIENT),
        (
            "shop/urls.py",
            "from django.urls import path\nfrom thirdparty import views\n\nurlpatterns = [path('api/orders/', views.index)]\n",
        ),
    ]);
    let edge = graph
        .edges()
        .find(|e| e.kind() == EdgeKind::HttpCall)
        .expect("the request still matches the route");
    let target = graph.node(edge.target()).unwrap();
    assert_eq!(target.kind(), NodeKind::Route);
    assert_eq!(target.location().unwrap().file(), "shop/urls.py");
    assert!(graph.nodes().all(|n| n.name() != "views.index"));
}

#[test]
fn the_edge_says_where_the_handler_is_declared_and_how_it_was_found() {
    let graph = graph_of(&[
        ("web/api.ts", CLIENT),
        ("shop/views.py", VIEWS),
        (
            "shop/urls.py",
            "from django.urls import path\nfrom . import views\n\nurlpatterns = [path('api/orders/', views.list_orders)]\n",
        ),
    ]);
    let edge = graph
        .edges()
        .find(|e| e.kind() == EdgeKind::HttpCall)
        .unwrap();
    let evidence = edge.evidence().as_str();
    assert!(evidence.contains("at shop/urls.py:"), "{evidence}");
    assert!(
        evidence.contains("handler `list_orders` declared at shop/views.py:"),
        "{evidence}"
    );
    assert!(evidence.contains("`from . import views`"), "{evidence}");
}

// ── found by independent review ─────────────────────────────────────

#[test]
fn a_view_imported_twice_from_different_modules_resolves_to_nothing() {
    // R-01: Python binds the second import; taking the first sent the request
    // to the wrong module's view.
    let files = [
        (
            "shop/views_v1.py",
            "def list_orders(request):\n    return None\n",
        ),
        (
            "shop/views_v2.py",
            "def list_orders(request):\n    return None\n",
        ),
        (
            "shop/urls.py",
            "from django.urls import path\nfrom .views_v1 import list_orders\nfrom .views_v2 import list_orders\n\nurlpatterns = [path('orders/', list_orders)]\n",
        ),
    ];
    assert_eq!(declared(&files, "shop/urls.py", "list_orders"), None);
}

#[test]
fn a_view_sharing_its_name_with_a_method_resolves_to_nothing() {
    // R-02: `detail` the function and `AdminTools.detail` the method are one
    // node, so the request would inherit the method's model accesses.
    let files = [
        (
            "shop/views.py",
            "def detail(request):\n    return None\n\n\nclass AdminTools:\n    def detail(self):\n        return None\n",
        ),
        (
            "shop/urls.py",
            "from django.urls import path\nfrom . import views\n\nurlpatterns = [path('d/', views.detail)]\n",
        ),
    ];
    assert_eq!(declared(&files, "shop/urls.py", "views.detail"), None);
}

#[test]
fn a_view_whose_name_is_rebound_after_its_def_resolves_to_nothing() {
    // R-03: the route is served by whatever `login_required(_real)` returns.
    let files = [
        (
            "shop/views.py",
            "def _real(request):\n    return None\n\n\ndef list_orders(request):\n    return None\n\n\nlist_orders = login_required(_real)\n",
        ),
        (
            "shop/urls.py",
            "from django.urls import path\nfrom . import views\n\nurlpatterns = [path('orders/', views.list_orders)]\n",
        ),
    ];
    assert_eq!(declared(&files, "shop/urls.py", "views.list_orders"), None);
}

#[test]
fn a_module_alias_rebound_in_the_url_conf_resolves_to_nothing() {
    // R-04: after `views = api_views` the alias is not the imported module.
    let files = [
        (
            "shop/views.py",
            "def list_orders(request):\n    return None\n",
        ),
        (
            "shop/api_views.py",
            "def list_orders(request):\n    return None\n",
        ),
        (
            "shop/urls.py",
            "from django.urls import path\nfrom . import views\nfrom . import api_views\n\nviews = api_views\n\nurlpatterns = [path('orders/', views.list_orders)]\n",
        ),
    ];
    assert_eq!(declared(&files, "shop/urls.py", "views.list_orders"), None);
}

#[test]
fn a_refused_reference_never_shares_a_node_with_a_function_of_that_name() {
    // S-01: `list_orders` is refused (the views file declares it twice), and
    // the URL conf also has a method called `list_orders` that reads a
    // secret table. The request must not reach it.
    let graph = graph_of(&[
        ("web/api.ts", CLIENT),
        (
            "shop/models.py",
            "from django.db import models\n\n\nclass Secret(models.Model):\n    class Meta:\n        db_table = 'secrets'\n",
        ),
        (
            "shop/views.py",
            "def list_orders(request):\n    return None\n\n\nclass Tools:\n    def list_orders(self):\n        return None\n",
        ),
        (
            "shop/urls.py",
            "from django.urls import path\nfrom .views import list_orders\nfrom .models import Secret\n\n\nclass Helper:\n    def list_orders(self):\n        return Secret.objects.all()\n\n\nurlpatterns = [path('api/orders/', list_orders)]\n",
        ),
    ]);
    let edge = graph
        .edges()
        .find(|e| e.kind() == EdgeKind::HttpCall)
        .unwrap();
    let target = graph.node(edge.target()).unwrap();
    assert_eq!(target.kind(), NodeKind::Route, "got {target:?}");
    assert_eq!(
        graph.edges().filter(|e| e.source() == target.id()).count(),
        0,
        "the route node carries no model access"
    );
}

#[test]
fn a_view_imported_after_a_local_def_of_the_same_name_resolves_to_nothing() {
    // S-02: Python serves the import, which rebinds the name; the resolver
    // prefers the local def whatever the order.
    let files = [
        (
            "shop/views.py",
            "def list_orders(request):\n    return None\n",
        ),
        (
            "shop/urls.py",
            "from django.urls import path\n\n\ndef list_orders(request):\n    return None\n\n\nfrom .views import list_orders\n\nurlpatterns = [path('orders/', list_orders)]\n",
        ),
    ];
    assert_eq!(declared(&files, "shop/urls.py", "list_orders"), None);
}
