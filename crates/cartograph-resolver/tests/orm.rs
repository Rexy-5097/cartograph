//! ORM and database resolution: what resolves, and what must refuse.
//!
//! Fifteen mandated refusal cases assert **no edge is produced**, not merely an
//! empty collection. A resolver that maps every class to a table is worse than
//! useless, because a wrong table poisons everything downstream.

use cartograph_core::{EdgeKind, NodeKind, Provenance};
use cartograph_graph::ArchitectureGraph;
use cartograph_parser::Analyzer;
use cartograph_parser::model::FileAnalysis;
use cartograph_resolver::{AccessKind, OrmAnalysis, OrmFlavor, TableName, add_orm_edges};

fn analyze(files: &[(&str, &str)]) -> Vec<FileAnalysis> {
    let mut a = Analyzer::new().expect("grammars load");
    files
        .iter()
        .map(|(p, s)| a.analyze_source(p, s).expect("fixture parses"))
        .collect()
}

fn orm(files: &[(&str, &str)]) -> OrmAnalysis {
    OrmAnalysis::build(&analyze(files))
}

/// Builds the graph and returns (edge count, graph).
fn graph_of(files: &[(&str, &str)]) -> (usize, ArchitectureGraph) {
    let analysis = orm(files);
    let mut g = ArchitectureGraph::new();
    let n = add_orm_edges(&mut g, &analysis, None).expect("graph accepts its own nodes");
    (n, g)
}

const SQLALCHEMY_MODELS: &str = r#"
from sqlalchemy.orm import DeclarativeBase

class Base(DeclarativeBase):
    pass

class Order(Base):
    __tablename__ = "orders"

class LineItem(Base):
    __tablename__ = "order_records"
"#;

const DJANGO_MODELS: &str = r#"
from django.db import models

class Invoice(models.Model):
    class Meta:
        db_table = "invoices"

class Receipt(models.Model):
    pass
"#;

// ── Slices 1–2: SQLAlchemy ──────────────────────────────────────────

#[test]
fn sqlalchemy_models_are_discovered_by_their_base() {
    let a = orm(&[("api/models.py", SQLALCHEMY_MODELS)]);
    let order = a.models.get("Order").expect("Order discovered");
    assert_eq!(order.flavor, OrmFlavor::SqlAlchemy);
    assert_eq!(order.base, "Base");
}

#[test]
fn an_explicit_tablename_is_read_exactly() {
    let a = orm(&[("api/models.py", SQLALCHEMY_MODELS)]);
    assert_eq!(a.models["Order"].table.name(), Some("orders"));
    assert_eq!(
        a.models["LineItem"].table.name(),
        Some("order_records"),
        "the declared name wins; it is not derived from the class name"
    );
    assert!(matches!(
        a.models["Order"].table,
        TableName::Declared { ref attribute, .. } if attribute == "__tablename__"
    ));
}

#[test]
fn a_sqlalchemy_model_without_a_tablename_is_unresolved_not_guessed() {
    let a = orm(&[(
        "api/models.py",
        r"
from sqlalchemy.orm import DeclarativeBase

class Base(DeclarativeBase):
    pass

class Order(Base):
    pass
",
    )]);
    assert_eq!(
        a.models["Order"].table.name(),
        None,
        "deriving \"orders\" from the class name would be a guess"
    );
    assert!(matches!(
        a.models["Order"].table,
        TableName::Unresolved { .. }
    ));
}

// ── Slices 3–4: Django ──────────────────────────────────────────────

#[test]
fn django_models_are_discovered_and_meta_db_table_is_read() {
    let a = orm(&[("api/models.py", DJANGO_MODELS)]);
    let invoice = a.models.get("Invoice").expect("Invoice discovered");
    assert_eq!(invoice.flavor, OrmFlavor::Django);
    assert_eq!(invoice.table.name(), Some("invoices"));
}

#[test]
fn a_django_model_without_db_table_is_unresolved_because_the_app_label_is_unknown() {
    let a = orm(&[("api/models.py", DJANGO_MODELS)]);
    assert_eq!(
        a.models["Receipt"].table.name(),
        None,
        "Django's default is app_model; the app label is not in this file"
    );
}

#[test]
fn the_meta_collision_never_regresses() {
    // The exact construction that exposed the defect. Every Django model nests
    // a class called `Meta`, so matching the attribute by its owner's *name*
    // gave model B model A's table. The assertion is not merely that B has no
    // name — it is that B produces NO EDGE.
    let files = [(
        "api/models.py",
        r#"
from django.db import models

class ModelA(models.Model):
    class Meta:
        db_table = "invoice_records"

class ModelB(models.Model):
    class Meta:
        db_table = settings.TABLE
"#,
    )];
    let a = orm(&files);
    assert_eq!(a.models["ModelA"].table.name(), Some("invoice_records"));
    assert_eq!(
        a.models["ModelB"].table.name(),
        None,
        "ModelB must not inherit ModelA's table"
    );
    assert!(
        matches!(a.models["ModelB"].table, TableName::Unresolved { ref reason }
                 if reason.contains("not a string literal")),
        "the refusal must be classified, not merely empty: {:?}",
        a.models["ModelB"].table
    );

    let (_, graph) = graph_of(&files);
    let tables: Vec<&str> = graph
        .nodes()
        .filter(|n| n.kind() == NodeKind::Table)
        .map(cartograph_core::Node::name)
        .collect();
    assert_eq!(
        tables,
        ["invoice_records"],
        "exactly one table node; ModelB produced NO EDGE"
    );
    assert_eq!(
        graph
            .edges()
            .filter(|e| e.kind() == EdgeKind::Queries)
            .count(),
        1
    );
}

#[test]
fn a_dynamic_db_table_expression_is_refused() {
    // The defect this test was written for: `Meta` is a universal Django class
    // name, so matching the attribute by name alone attributed the first
    // db_table in the file to every model.
    let a = orm(&[(
        "api/models.py",
        r#"
from django.db import models

class Invoice(models.Model):
    class Meta:
        db_table = "invoices"

class Dynamic(models.Model):
    class Meta:
        db_table = settings.TABLE_NAME
"#,
    )]);
    assert_eq!(a.models["Invoice"].table.name(), Some("invoices"));
    assert_eq!(
        a.models["Dynamic"].table.name(),
        None,
        "a dynamic db_table must not inherit another model's table"
    );
}

#[test]
fn a_malformed_meta_produces_no_table() {
    let a = orm(&[(
        "api/models.py",
        r"
from django.db import models

class Broken(models.Model):
    class Meta:
        pass
",
    )]);
    assert_eq!(a.models["Broken"].table.name(), None);
}

#[test]
fn a_conditionally_declared_table_name_is_not_read() {
    // `if DEBUG: __tablename__ = "..."` is not statically determined — the
    // attribute only exists on one branch. The parser records class-body
    // assignments, not assignments nested inside control flow, so this
    // resolves to Unresolved rather than picking a branch.
    let a = orm(&[(
        "api/models.py",
        r#"
from sqlalchemy.orm import DeclarativeBase

class Base(DeclarativeBase):
    pass

class Conditional(Base):
    if DEBUG:
        __tablename__ = "orders_debug"
    else:
        __tablename__ = "orders"
"#,
    )]);
    assert_eq!(
        a.models["Conditional"].table.name(),
        None,
        "choosing a branch would be a guess about configuration"
    );
}

#[test]
fn a_non_model_carrying_a_tablename_is_still_not_a_model() {
    // The attribute is extracted — it genuinely is a class attribute — but the
    // class has no recognised ORM base, so it is not a model and declares no
    // table.
    let a = orm(&[(
        "api/models.py",
        r#"
class NotAModel:
    name = "orders"
    __tablename__ = "orders"
"#,
    )]);
    assert!(
        !a.models.contains_key("NotAModel"),
        "a __tablename__ on an unrelated class is not a table declaration"
    );
    assert!(a.models.is_empty());
}

// ── NEGATIVE: what is not a model ───────────────────────────────────

#[test]
fn an_ordinary_class_is_not_a_model() {
    let a = orm(&[(
        "api/models.py",
        r#"
class Plain:
    label = "orders"

class Helper(object):
    __tablename__ = "orders"
"#,
    )]);
    assert!(
        a.models.is_empty(),
        "a class is a model only when it inherits a recognised ORM base: {:?}",
        a.models.keys().collect::<Vec<_>>()
    );
}

#[test]
fn a_similarly_named_base_does_not_make_a_model() {
    let a = orm(&[(
        "api/models.py",
        r#"
class OrderBase:
    pass

class Order(OrderBase):
    __tablename__ = "orders"
"#,
    )]);
    assert!(
        a.models.is_empty(),
        "`OrderBase` is not `Base`; a __tablename__ on it declares nothing"
    );
}

#[test]
fn a_string_containing_a_model_name_creates_nothing() {
    let a = orm(&[(
        "api/handlers.py",
        r#"
def log():
    message = "Order was created in orders"
    return message
"#,
    )]);
    assert!(a.models.is_empty() && a.accesses.is_empty());
}

#[test]
fn two_classes_claiming_one_name_are_ambiguous_and_produce_no_edge() {
    let (edges, graph) = graph_of(&[
        (
            "billing/models.py",
            "from sqlalchemy.orm import DeclarativeBase\nclass Base(DeclarativeBase):\n    pass\nclass Order(Base):\n    __tablename__ = \"billing_orders\"\n",
        ),
        (
            "shipping/models.py",
            "from sqlalchemy.orm import DeclarativeBase\nclass Base(DeclarativeBase):\n    pass\nclass Order(Base):\n    __tablename__ = \"shipping_orders\"\n",
        ),
    ]);
    let a = orm(&[
        (
            "billing/models.py",
            "from sqlalchemy.orm import DeclarativeBase\nclass Base(DeclarativeBase):\n    pass\nclass Order(Base):\n    __tablename__ = \"billing_orders\"\n",
        ),
        (
            "shipping/models.py",
            "from sqlalchemy.orm import DeclarativeBase\nclass Base(DeclarativeBase):\n    pass\nclass Order(Base):\n    __tablename__ = \"shipping_orders\"\n",
        ),
    ]);

    assert!(a.ambiguous.contains(&"Order".to_string()));
    assert!(
        !a.models.contains_key("Order"),
        "an ambiguous name resolves to nothing"
    );
    assert_eq!(
        graph
            .edges()
            .filter(|e| e.target() != e.source())
            .filter(|e| {
                graph.node(e.target()).map(cartograph_core::Node::name) == Some("billing_orders")
            })
            .count(),
        0
    );
    let _ = edges;
}

// ── Slice 5: handler → model access ─────────────────────────────────

#[test]
fn a_django_queryset_call_is_an_access_from_its_handler() {
    let a = orm(&[
        ("api/models.py", DJANGO_MODELS),
        (
            "api/handlers.py",
            r"
def list_invoices():
    return Invoice.objects.filter(paid=True)
",
        ),
    ]);
    let access = a.accesses.first().expect("one access");
    assert_eq!(access.model, "Invoice");
    assert_eq!(access.handler.as_deref(), Some("list_invoices"));
    assert_eq!(access.kind, AccessKind::Query);
    assert_eq!(access.expression, "Invoice.objects.filter");
}

#[test]
fn a_write_method_is_recorded_as_persistence() {
    let a = orm(&[
        ("api/models.py", DJANGO_MODELS),
        (
            "api/handlers.py",
            "def make():\n    return Invoice.objects.create(x=1)\n",
        ),
    ]);
    assert_eq!(a.accesses[0].kind, AccessKind::Persist);
}

#[test]
fn model_construction_is_an_access() {
    let a = orm(&[
        ("api/models.py", SQLALCHEMY_MODELS),
        (
            "api/handlers.py",
            "def create_order(p):\n    return Order(**p)\n",
        ),
    ]);
    assert_eq!(a.accesses[0].model, "Order");
    assert_eq!(a.accesses[0].kind, AccessKind::Persist);
}

#[test]
fn an_unrecognised_manager_method_produces_no_access() {
    let a = orm(&[
        ("api/models.py", DJANGO_MODELS),
        (
            "api/handlers.py",
            "def weird():\n    return Invoice.objects.nonstandard()\n",
        ),
    ]);
    // Positive control: the model itself resolved, so the refusal is about the
    // method rather than a failure to find the model.
    assert!(a.models.contains_key("Invoice"), "the model was discovered");
    assert!(
        a.accesses.is_empty(),
        "the supported subset is deliberate; an unknown method is not evidence"
    );
}

#[test]
fn an_access_on_a_non_model_produces_nothing() {
    let a = orm(&[
        ("api/models.py", DJANGO_MODELS),
        (
            "api/handlers.py",
            "def f():\n    return Plain.objects.get(1)\n",
        ),
    ]);
    assert!(a.accesses.is_empty());
}

#[test]
fn a_local_binding_shadowing_a_model_disqualifies_the_access() {
    let a = orm(&[
        ("api/models.py", SQLALCHEMY_MODELS),
        (
            "api/handlers.py",
            r"
Order = make_fake()

def create():
    return Order(x=1)
",
        ),
    ]);
    // Positive control: the model resolved and the call exists; only the
    // shadowing disqualifies the access.
    assert!(a.models.contains_key("Order"), "the model was discovered");
    assert!(
        a.accesses.is_empty(),
        "the analysis cannot tell which binding the call site sees"
    );
}

#[test]
fn a_module_scope_access_names_no_handler_and_creates_no_edge() {
    let files = [
        ("api/models.py", DJANGO_MODELS),
        ("api/handlers.py", "rows = Invoice.objects.all()\n"),
    ];
    let a = orm(&files);
    assert_eq!(a.accesses[0].handler, None);

    let (_, graph) = graph_of(&files);
    assert_eq!(
        graph
            .edges()
            .filter(|e| e.kind() == EdgeKind::OrmAccess)
            .count(),
        0,
        "attributing a module-level access to a handler would be a guess"
    );
}

// ── Slices 6–7: edges ───────────────────────────────────────────────

#[test]
fn a_resolved_model_produces_a_queries_edge_to_its_table() {
    let (_, graph) = graph_of(&[("api/models.py", SQLALCHEMY_MODELS)]);
    let edge = graph
        .edges()
        .find(|e| e.kind() == EdgeKind::Queries)
        .expect("a model→table edge");

    assert_eq!(edge.provenance(), Provenance::OrmResolution);
    assert_eq!(graph.node(edge.target()).unwrap().kind(), NodeKind::Table);
    assert!(edge.evidence().as_str().contains("__tablename__"));
}

#[test]
fn an_unresolved_table_produces_no_table_edge() {
    let (_, graph) = graph_of(&[(
        "api/models.py",
        "from django.db import models\nclass Receipt(models.Model):\n    pass\n",
    )]);
    assert_eq!(
        graph
            .edges()
            .filter(|e| e.kind() == EdgeKind::Queries)
            .count(),
        0,
        "NO EDGE IS PRODUCED when the table is unknown"
    );
}

#[test]
fn repeated_accesses_from_one_handler_produce_one_edge() {
    let (_, graph) = graph_of(&[
        ("api/models.py", DJANGO_MODELS),
        (
            "api/handlers.py",
            r"
def report():
    a = Invoice.objects.filter(x=1)
    b = Invoice.objects.filter(x=2)
    c = Invoice.objects.all()
    return a, b, c
",
        ),
    ]);
    assert_eq!(
        graph
            .edges()
            .filter(|e| e.kind() == EdgeKind::OrmAccess)
            .count(),
        1,
        "the same fact stated three times is one fact"
    );
}

#[test]
fn access_and_table_relationships_stay_distinct() {
    let (_, graph) = graph_of(&[
        ("api/models.py", DJANGO_MODELS),
        (
            "api/handlers.py",
            "def f():\n    return Invoice.objects.all()\n",
        ),
    ]);
    let access = graph
        .edges()
        .filter(|e| e.kind() == EdgeKind::OrmAccess)
        .count();
    let queries = graph
        .edges()
        .filter(|e| e.kind() == EdgeKind::Queries)
        .count();
    assert_eq!(
        (access, queries),
        (1, 1),
        "two relationships, not one merged claim"
    );
}

#[test]
fn every_orm_edge_carries_evidence_and_a_location() {
    let (_, graph) = graph_of(&[
        ("api/models.py", DJANGO_MODELS),
        (
            "api/handlers.py",
            "def f():\n    return Invoice.objects.all()\n",
        ),
    ]);
    for edge in graph.edges() {
        assert!(!edge.evidence().as_str().is_empty());
        assert_eq!(edge.provenance(), Provenance::OrmResolution);
        assert!(!edge.location().file().is_empty());
    }
}

#[test]
fn the_handler_to_table_chain_is_walkable() {
    let (_, graph) = graph_of(&[
        ("api/models.py", SQLALCHEMY_MODELS),
        (
            "api/handlers.py",
            "def create_order(p):\n    return Order(**p)\n",
        ),
    ]);

    let handler = graph
        .nodes()
        .find(|n| n.name() == "create_order")
        .expect("handler node");
    let to_model = graph
        .edges_from(handler.id())
        .next()
        .expect("handler→model");
    let model = graph.node(to_model.target()).unwrap();
    assert_eq!(model.name(), "Order");

    let to_table = graph.edges_from(model.id()).next().expect("model→table");
    let table = graph.node(to_table.target()).unwrap();
    assert_eq!(table.name(), "orders");
    assert_eq!(table.kind(), NodeKind::Table);
}

// ── Raw SQL ─────────────────────────────────────────────────────────

#[test]
fn raw_sql_is_not_linked_to_a_model() {
    let a = orm(&[
        ("api/models.py", SQLALCHEMY_MODELS),
        (
            "api/handlers.py",
            r#"
def raw():
    return execute("INSERT INTO orders (id) VALUES (1)")
"#,
        ),
    ]);
    // Positive control: `Order` is a discovered model, so the refusal is about
    // the SQL string rather than a missing model.
    assert!(a.models.contains_key("Order"), "the model was discovered");
    assert!(
        a.accesses.is_empty(),
        "a table name inside SQL text is not evidence of an ORM relationship"
    );
}

// ── M07 regression: defects found on real repositories ───────────────
//
// Each of these reproduces something the M07 corpus produced, reduced to the
// smallest source that still shows it. They run against fixtures rather than
// the repositories, so the lesson survives without the corpus.

/// Superset, `superset/connectors/sqla/models.py:1039`.
///
/// 32 of Superset's 33 models and 61 of Airflow's declare `__tablename__`
/// while inheriting a bare `Model` from Flask-AppBuilder. Reading `Model` as
/// Django sent table resolution looking for a `Meta.db_table` that is not
/// there: Superset resolved 3 tables out of 33.
#[test]
fn a_shared_model_base_with_a_tablename_is_sqlalchemy() {
    let analysis = orm(&[(
        "superset/models.py",
        r#"
class TableColumn(AuditMixinNullable, ImportExportMixin, Model):
    __tablename__ = "table_columns"
"#,
    )]);
    let model = analysis.models.get("TableColumn").expect("a model");
    assert_eq!(model.flavor, OrmFlavor::SqlAlchemy, "{model:?}");
    assert_eq!(
        model.table,
        TableName::Declared {
            name: "table_columns".into(),
            attribute: "__tablename__".into()
        }
    );
}

#[test]
fn a_shared_model_base_with_meta_db_table_is_django() {
    let analysis = orm(&[(
        "app/models.py",
        r#"
class Invoice(Model):
    class Meta:
        db_table = "invoice_records"
"#,
    )]);
    let model = analysis.models.get("Invoice").expect("a model");
    assert_eq!(model.flavor, OrmFlavor::Django);
    assert_eq!(model.table.name(), Some("invoice_records"));
}

#[test]
fn a_shared_model_base_declaring_neither_table_attribute_says_so() {
    // `Model` alone does not identify an ORM, and neither declaration is
    // present. Naming one would be a guess, and the table stays unresolved.
    let analysis = orm(&[(
        "app/models.py",
        r#"
class Ambiguous(Model):
    pass
"#,
    )]);
    let model = analysis.models.get("Ambiguous").expect("a model");
    assert_eq!(model.flavor, OrmFlavor::Unspecified);
    assert!(model.table.name().is_none(), "{:?}", model.table);
    let TableName::Unresolved { reason } = &model.table else {
        panic!("expected Unresolved, got {:?}", model.table);
    };
    assert!(
        reason.contains("Django") && reason.contains("SQLAlchemy"),
        "the reason must name the ambiguity: {reason}"
    );
}

/// Superset, `superset-frontend/packages/superset-core/src/theme/Theme.tsx:101`,
/// and Zulip, `web/src/desktop_notifications.ts:41`.
///
/// A Python ORM model cannot be constructed from TypeScript. Before the
/// language filter, `new Theme({...})` in a .tsx file produced an OrmAccess
/// edge to Superset's Python `Theme` model — 33 such edges across the corpus.
#[test]
fn a_typescript_call_never_becomes_an_orm_access() {
    let (edges, graph) = graph_of(&[
        (
            "app/models.py",
            r#"
from sqlalchemy.orm import DeclarativeBase

class Base(DeclarativeBase):
    pass

class Theme(Base):
    __tablename__ = "themes"
"#,
        ),
        (
            "web/src/theme.tsx",
            r#"
export function build(config) {
  return new Theme({ config });
}
"#,
        ),
    ]);

    let accesses: Vec<_> = graph
        .edges()
        .filter(|e| e.kind() == EdgeKind::OrmAccess)
        .collect();
    assert!(
        accesses.is_empty(),
        "a TypeScript call produced an ORM access: {:?}",
        accesses
            .iter()
            .map(|e| e.evidence().as_str())
            .collect::<Vec<_>>()
    );
    // The model itself is still resolved; only the bogus access is gone.
    assert_eq!(edges, 1, "the model to table edge must survive");
}

/// Airflow, `airflow-core/docs/img/diagram_*_architecture.py`.
///
/// Those scripts call `User("DAG Author")` having imported `User` from the
/// `diagrams` package. Matching on the name alone attributed 218 of them to
/// the Flask-AppBuilder `User` model.
#[test]
fn a_name_imported_from_outside_the_project_is_not_the_model() {
    let analysis = orm(&[
        (
            "app/models.py",
            r#"
from django.db import models

class User(models.Model):
    class Meta:
        db_table = "users"
"#,
        ),
        (
            "docs/diagram.py",
            r#"
from diagrams.onprem.client import User

def draw():
    author = User("DAG Author")
"#,
        ),
    ]);
    assert!(
        analysis.accesses.is_empty(),
        "a third-party symbol was attributed to the project's model: {:?}",
        analysis.accesses
    );
}

#[test]
fn a_model_imported_from_a_project_module_is_still_attributed() {
    // The control for the test above. If this stops passing, the import rule
    // has traded false positives for false negatives.
    let analysis = orm(&[
        (
            "app/models.py",
            r#"
from django.db import models

class User(models.Model):
    class Meta:
        db_table = "users"
"#,
        ),
        (
            "app/service.py",
            r#"
from app.models import User

def load():
    return User.objects.filter(active=True)
"#,
        ),
    ]);
    assert_eq!(analysis.accesses.len(), 1, "{:?}", analysis.accesses);
    assert_eq!(analysis.accesses[0].model, "User");
    assert_eq!(analysis.accesses[0].kind, AccessKind::Query);
}

#[test]
fn a_model_re_exported_by_another_project_module_is_still_attributed() {
    // Onyx imports SearchSettings from onyx.db.search_settings, which
    // re-exports it from onyx.db.models. The rule asks whether the module is
    // the project's, not whether it is the declaring one.
    let analysis = orm(&[
        (
            "backend/onyx/db/models.py",
            r#"
from sqlalchemy.orm import DeclarativeBase

class Base(DeclarativeBase):
    pass

class SearchSettings(Base):
    __tablename__ = "search_settings"
"#,
        ),
        (
            "backend/onyx/db/search_settings.py",
            "from onyx.db.models import SearchSettings\n",
        ),
        (
            "backend/onyx/service.py",
            r#"
from onyx.db.search_settings import SearchSettings

def load(row):
    return SearchSettings(**row)
"#,
        ),
    ]);
    let sites: Vec<_> = analysis
        .accesses
        .iter()
        .filter(|a| a.file == "backend/onyx/service.py")
        .collect();
    assert_eq!(sites.len(), 1, "{:?}", analysis.accesses);
}

#[test]
fn a_relative_import_is_treated_as_project_local() {
    let analysis = orm(&[
        (
            "app/models.py",
            r#"
from django.db import models

class Order(models.Model):
    class Meta:
        db_table = "orders"
"#,
        ),
        (
            "app/views.py",
            r#"
from .models import Order

def create():
    return Order.objects.create()
"#,
        ),
    ]);
    assert_eq!(analysis.accesses.len(), 1, "{:?}", analysis.accesses);
}
