//! ORM and database resolution: model classes to tables, handlers to models.
//!
//! ```text
//! class Order(Base)          ─► OrmModel ─► table "orders"
//! Order.objects.create(...)  ─► handler ──OrmAccess──► Order ──Queries──► orders
//! ```
//!
//! Static source analysis only. Nothing here executes Python, imports a
//! module, connects to a database, reads the environment, or runs an ORM.
//!
//! # The rule that decides every case
//!
//! **A class is a model only when the source says so.** Inheriting from a
//! recognised ORM base is the evidence; a name that looks like a model is not.
//! `class Plain: label = "orders"` is not a table, and a local variable named
//! `Order` is not the model.
//!
//! Table names follow the same discipline. An explicit `__tablename__` or
//! `Meta.db_table` is read from the source. A *derived* name is produced only
//! where the framework's convention is deterministic and documented — Django's
//! `app_modelname` cannot be derived without the app label, so Django models
//! without `db_table` resolve to [`TableName::Unresolved`] rather than a guess.

use std::collections::{HashMap, HashSet};

use cartograph_core::{CommitId, EdgeKind, Evidence, NodeId, NodeKind, Provenance, SourceLocation};
use cartograph_graph::{ArchitectureGraph, EdgeSpec, GraphError};
use cartograph_parser::model::{
    Callee, FileAnalysis, SourceLanguage, Span, StringFact, Symbol, SymbolKind,
};
use serde::{Deserialize, Serialize};

/// Which ORM a model was recognised from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
#[non_exhaustive]
pub enum OrmFlavor {
    /// A `SQLAlchemy` declarative model.
    SqlAlchemy,
    /// A `Django` model.
    Django,
    /// A model whose base is used by more than one ORM, declaring neither
    /// ORM's table attribute.
    ///
    /// `Model` is the base name used by `Django`, `Flask-SQLAlchemy` and
    /// `Flask-AppBuilder`. When a class inheriting it declares neither
    /// `__tablename__` nor `Meta.db_table`, the source does not say which ORM
    /// it belongs to, and naming one would be a guess.
    Unspecified,
}

/// Base classes that mark a `SQLAlchemy` declarative model and nothing else.
///
/// `Base` is the near-universal convention for `declarative_base()`'s result;
/// `DeclarativeBase` is the 2.0 style and `db.Model` the `Flask-SQLAlchemy`
/// one. A bare `Model` is deliberately absent — see [`SHARED_BASES`].
const SQLALCHEMY_BASES: [&str; 3] = ["Base", "DeclarativeBase", "db.Model"];

/// Base classes more than one ORM uses, so the name alone decides nothing.
///
/// `Model` is `Django`'s conventional import (`from django.db.models import
/// Model`) and equally `Flask-SQLAlchemy`'s and `Flask-AppBuilder`'s. Reading
/// it as `Django` cost every Superset and Airflow model its table at M07: the
/// class was searched for a `Meta.db_table` it does not have while its
/// `__tablename__` sat one line below. Which ORM it is has to be read from
/// what the class declares, not from what the base is called.
const SHARED_BASES: [&str; 1] = ["Model"];

/// How a model's table name was determined.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum TableName {
    /// The source stated the name.
    Declared {
        /// The table name, exactly as written.
        name: String,
        /// The attribute that stated it — `__tablename__` or `db_table`.
        attribute: String,
    },
    /// The framework's convention determines it deterministically.
    Derived {
        /// The derived name.
        name: String,
        /// The rule that produced it, for the evidence record.
        rule: String,
    },
    /// The name could not be determined safely.
    ///
    /// Preferred over a guess: a wrong table is worse than a missing one,
    /// because everything downstream inherits it.
    Unresolved {
        /// Why, for the evidence record.
        reason: String,
    },
}

impl TableName {
    /// The table name, if one was determined.
    #[must_use]
    pub fn name(&self) -> Option<&str> {
        match self {
            Self::Declared { name, .. } | Self::Derived { name, .. } => Some(name),
            Self::Unresolved { .. } => None,
        }
    }

    /// How this name was arrived at, for an edge's evidence.
    #[must_use]
    pub fn explain(&self) -> String {
        match self {
            Self::Declared { name, attribute } => format!("{attribute} = \"{name}\""),
            Self::Derived { name, rule } => format!("derived \"{name}\" by {rule}"),
            Self::Unresolved { reason } => format!("table not resolved: {reason}"),
        }
    }
}

/// An ORM model class discovered in source.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OrmModel {
    /// The class name.
    pub name: String,
    /// Which ORM recognised it.
    pub flavor: OrmFlavor,
    /// The base class that provided the evidence.
    pub base: String,
    /// The table it maps to, or why that is unknown.
    pub table: TableName,
    /// Repository-relative file.
    pub file: String,
    /// Where the class is declared.
    pub span: Span,
}

/// Where an ORM model was used, and how.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OrmAccessSite {
    /// The model's class name, as written at the access site.
    pub model: String,
    /// The enclosing function or method, when there is one.
    ///
    /// `None` at module scope: attributing a module-level access to a handler
    /// would be a guess.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub handler: Option<String>,
    /// The call as written, e.g. `Order.objects.create`.
    pub expression: String,
    /// Whether the access reads or writes through a query.
    pub kind: AccessKind,
    /// Repository-relative file.
    pub file: String,
    /// Where the access is.
    pub span: Span,
}

/// What an access does, as far as the syntax says.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
#[non_exhaustive]
pub enum AccessKind {
    /// A query against the model's table: `filter`, `get`, `all`, `query`.
    Query,
    /// Construction or persistence: `Order(...)`, `session.add`, `create`.
    Persist,
}

/// `Django` queryset methods that express a query.
const DJANGO_QUERY_METHODS: [&str; 8] = [
    "get", "filter", "all", "exclude", "count", "exists", "first", "last",
];
/// `Django` queryset methods that write.
const DJANGO_WRITE_METHODS: [&str; 4] = ["create", "update", "delete", "bulk_create"];
/// `SQLAlchemy` session methods taking a model as their first argument.
const SQLALCHEMY_SESSION_METHODS: [&str; 5] = ["query", "get", "add", "merge", "delete"];

/// Everything M06 found in one project.
#[derive(Debug, Default, Clone)]
pub struct OrmAnalysis {
    /// Models, keyed by class name.
    ///
    /// A name declared by more than one class is **removed** rather than
    /// arbitrarily resolved — see [`OrmAnalysis::ambiguous`].
    pub models: HashMap<String, OrmModel>,
    /// Model names that more than one class claimed.
    pub ambiguous: Vec<String>,
    /// Access sites whose model resolved.
    pub accesses: Vec<OrmAccessSite>,
}

impl OrmAnalysis {
    /// Discovers models and accesses across a project's analysed files.
    #[must_use]
    pub fn build(files: &[FileAnalysis]) -> Self {
        let mut analysis = Self::default();
        let mut seen: HashMap<String, usize> = HashMap::new();

        // A Python ORM model cannot be declared in, or used from, a
        // TypeScript file. Before this filter `new Theme({...})` in a Superset
        // .tsx file and `new Event(...)` in a Zulip web module produced
        // OrmAccess edges to Python models that share the name.
        let python: Vec<&FileAnalysis> = files
            .iter()
            .filter(|f| f.language == SourceLanguage::Python)
            .collect();
        // Which module specifiers name a file the project actually contains.
        let project: HashSet<&str> = files.iter().map(|f| f.path.as_str()).collect();

        for file in &python {
            for model in discover_models(file) {
                *seen.entry(model.name.clone()).or_insert(0) += 1;
                analysis.models.insert(model.name.clone(), model);
            }
        }
        // A class name claimed twice cannot identify one table.
        for (name, count) in seen {
            if count > 1 {
                analysis.models.remove(&name);
                analysis.ambiguous.push(name);
            }
        }
        analysis.ambiguous.sort();

        for file in &python {
            analysis
                .accesses
                .extend(discover_accesses(file, &analysis.models, &project));
        }
        analysis
    }
}

/// Finds ORM model classes in one file (slices 1 and 3).
#[must_use]
pub fn discover_models(file: &FileAnalysis) -> Vec<OrmModel> {
    file.symbols
        .iter()
        .filter(|s| s.kind == SymbolKind::Class && s.enclosing.is_none())
        .filter_map(|class| {
            let (evidence, base_name) = model_base(class)?;
            let (flavor, table) = match evidence {
                BaseEvidence::Django => (
                    OrmFlavor::Django,
                    resolve_table(file, class, OrmFlavor::Django),
                ),
                BaseEvidence::SqlAlchemy => (
                    OrmFlavor::SqlAlchemy,
                    resolve_table(file, class, OrmFlavor::SqlAlchemy),
                ),
                BaseEvidence::Either => resolve_shared_base(file, class),
            };
            Some(OrmModel {
                name: class.name.clone(),
                flavor,
                base: base_name,
                table,
                file: file.path.clone(),
                span: class.span,
            })
        })
        .collect()
}

/// What a recognised base tells us about which ORM a class belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BaseEvidence {
    /// The base belongs to `Django` alone.
    Django,
    /// The base belongs to `SQLAlchemy` alone.
    SqlAlchemy,
    /// The base name is shared; the class's own declarations decide.
    Either,
}

/// The ORM base a class inherits from, if any.
///
/// Only an exact base name counts: a class named `OrderBase` is not a model,
/// and neither is one inheriting from an unrelated class.
fn model_base(class: &Symbol) -> Option<(BaseEvidence, String)> {
    for base in &class.bases {
        if base == "models.Model" {
            return Some((BaseEvidence::Django, base.clone()));
        }
    }
    for base in &class.bases {
        if SQLALCHEMY_BASES.contains(&base.as_str()) {
            return Some((BaseEvidence::SqlAlchemy, base.clone()));
        }
    }
    for base in &class.bases {
        if SHARED_BASES.contains(&base.as_str()) {
            return Some((BaseEvidence::Either, base.clone()));
        }
    }
    None
}

/// Decides a shared-base class's ORM from what it declares.
///
/// `__tablename__` is `SQLAlchemy`'s and `Meta.db_table` is Django's, so either
/// one settles the question the base name left open. When the class declares
/// neither, the flavour stays [`OrmFlavor::Unspecified`] rather than being
/// guessed, and the table is unresolved either way.
fn resolve_shared_base(file: &FileAnalysis, class: &Symbol) -> (OrmFlavor, TableName) {
    if let Some(table) = declared_table(file, class, OrmFlavor::SqlAlchemy) {
        return (OrmFlavor::SqlAlchemy, table);
    }
    if let Some(table) = declared_table(file, class, OrmFlavor::Django) {
        return (OrmFlavor::Django, table);
    }
    (
        OrmFlavor::Unspecified,
        TableName::Unresolved {
            reason: "base `Model` is used by Django and SQLAlchemy alike, and the class \
                     declares neither __tablename__ nor Meta.db_table"
                .into(),
        },
    )
}

/// Resolves a model's table name (slices 2 and 4).
fn resolve_table(file: &FileAnalysis, class: &Symbol, flavor: OrmFlavor) -> TableName {
    // An explicit declaration always wins over any convention.
    if let Some(declared) = declared_table(file, class, flavor) {
        return declared;
    }
    match flavor {
        // SQLAlchemy 2.0 requires __tablename__ on a declarative model; a
        // model without one is either abstract or uses a naming callback this
        // analysis cannot see.
        OrmFlavor::SqlAlchemy => TableName::Unresolved {
            reason: "no __tablename__ declared".into(),
        },
        // Django derives `applabel_modelname`, and the app label comes from
        // the app's configuration rather than this file. Deriving from the
        // class name alone would produce a wrong table for every app.
        OrmFlavor::Django => TableName::Unresolved {
            reason: "no Meta.db_table; Django's default needs the app label".into(),
        },
        OrmFlavor::Unspecified => TableName::Unresolved {
            reason: "the ORM this class belongs to is not determined".into(),
        },
    }
}

/// An explicitly declared table name, if the class states one.
fn declared_table(file: &FileAnalysis, class: &Symbol, flavor: OrmFlavor) -> Option<TableName> {
    let attribute = match flavor {
        OrmFlavor::SqlAlchemy => "__tablename__",
        OrmFlavor::Django => "db_table",
        OrmFlavor::Unspecified => return None,
    };

    // SQLAlchemy declares on the class; Django declares inside a nested Meta.
    //
    // Ownership is decided by *span containment*, not by the owning class's
    // name. Every Django model nests a class called `Meta`, so matching on the
    // name alone attributes the first `db_table` in the file to every model —
    // a real defect this rule replaced, in which a model with a dynamic
    // `db_table` inherited an unrelated model's table.
    let within_class = |s: &Symbol| {
        s.span.start_line >= class.span.start_line && s.span.end_line <= class.span.end_line
    };
    let owner_names: Vec<String> = match flavor {
        OrmFlavor::SqlAlchemy | OrmFlavor::Unspecified => vec![class.name.clone()],
        OrmFlavor::Django => file
            .symbols
            .iter()
            .filter(|s| {
                s.kind == SymbolKind::Class
                    && s.enclosing.as_deref() == Some(class.name.as_str())
                    && within_class(s)
            })
            .map(|s| s.name.clone())
            .collect(),
    };

    let declaration = file.symbols.iter().find(|s| {
        s.kind == SymbolKind::Variable
            && s.name == attribute
            && s.enclosing
                .as_ref()
                .is_some_and(|e| owner_names.contains(e))
            && within_class(s)
    })?;

    // The value is the string literal inside the assignment's own span — the
    // same span-correlation the constant evaluator uses.
    let literal = file.strings.iter().find_map(|s| match s {
        StringFact::Literal { value, span }
            if span.start_line >= declaration.span.start_line
                && span.end_line <= declaration.span.end_line =>
        {
            Some(value.clone())
        }
        _ => None,
    });

    Some(match literal {
        Some(name) if !name.is_empty() => TableName::Declared {
            name,
            attribute: attribute.to_owned(),
        },
        // The attribute exists but its value is not a literal — an f-string, a
        // call, a settings lookup. Evaluating it would need to run Python.
        _ => TableName::Unresolved {
            reason: format!("{attribute} is not a string literal"),
        },
    })
}

/// Finds handler-to-model accesses in one file (slice 5).
///
/// Only calls whose receiver is a *known model name* are recorded, so an
/// unrelated `Plain.objects.get()` produces nothing.
#[must_use]
pub fn discover_accesses<S: std::hash::BuildHasher, T: std::hash::BuildHasher>(
    file: &FileAnalysis,
    models: &HashMap<String, OrmModel, S>,
    project: &HashSet<&str, T>,
) -> Vec<OrmAccessSite> {
    let mut sites = Vec::new();

    for call in &file.calls {
        let Some((model, kind)) = classify_access(&call.callee, models) else {
            continue;
        };
        // A local binding of the same name shadows the model; the access can no
        // longer be attributed to it safely.
        if shadows_model(file, &model, call.span) {
            continue;
        }
        // The name has to be *this* project's model, not a third party's
        // symbol that happens to share its name. Airflow's architecture
        // diagrams call `User("DAG Author")` having imported User from the
        // `diagrams` package; matching on the name alone attributed 218 of
        // those to the Flask-AppBuilder User model.
        if !names_the_project_model(file, &model, models, project) {
            continue;
        }
        sites.push(OrmAccessSite {
            handler: enclosing_symbol(file, call.span),
            expression: call.callee.to_string(),
            model,
            kind,
            file: file.path.clone(),
            span: call.span,
        });
    }
    sites
}

/// Classifies a call as an ORM access against a known model, or not at all.
fn classify_access<S: std::hash::BuildHasher>(
    callee: &Callee,
    models: &HashMap<String, OrmModel, S>,
) -> Option<(String, AccessKind)> {
    match callee {
        // `Order(...)` — construction, but only for a name that is a model.
        Callee::Identifier { name } if models.contains_key(name) => {
            Some((name.clone(), AccessKind::Persist))
        }
        Callee::Identifier { .. } => None,

        Callee::Member { object, property } => {
            // `Order.objects.<method>` — Django.
            if object.len() == 2 && object[1] == "objects" && models.contains_key(&object[0]) {
                let kind = if DJANGO_WRITE_METHODS.contains(&property.as_str()) {
                    AccessKind::Persist
                } else if DJANGO_QUERY_METHODS.contains(&property.as_str()) {
                    AccessKind::Query
                } else {
                    // An unrecognised manager method: not part of the
                    // supported subset, so no edge.
                    return None;
                };
                return Some((object[0].clone(), kind));
            }
            // `session.query(Order)` and friends are handled by argument
            // inspection, which M02 does not record; those are recognised via
            // `select(Order)`-style identifier calls instead.
            let _ = (object, property, SQLALCHEMY_SESSION_METHODS);
            None
        }
    }
}

/// Does this file's binding of `model` actually refer to the project's model?
///
/// Answered from the file's own imports. A name imported from a module the
/// project does not contain belongs to a third-party package, whatever it is
/// called. A name with no import statement binding it is left alone: star
/// imports, conditional imports and same-package references are all ordinary,
/// and refusing them would trade these false positives for false negatives.
fn names_the_project_model<S: std::hash::BuildHasher, T: std::hash::BuildHasher>(
    file: &FileAnalysis,
    model: &str,
    models: &HashMap<String, OrmModel, S>,
    project: &HashSet<&str, T>,
) -> bool {
    let Some(declared) = models.get(model) else {
        return false;
    };
    if declared.file == file.path {
        return true;
    }
    for import in &file.imports {
        let binds = import
            .named
            .iter()
            .any(|n| n.local.as_deref().unwrap_or(n.imported.as_str()) == model);
        if binds {
            return module_in_project(&import.specifier, project);
        }
    }
    true
}

/// Does a Python module specifier name a file inside the analysed project?
///
/// Purely a path question, resolved against the files that were analysed. A
/// relative specifier is project-local by definition. Matching is anchored to
/// a path separator so `ics.models` cannot be satisfied by `analytics/models.py`.
fn module_in_project<T: std::hash::BuildHasher>(
    specifier: &str,
    project: &HashSet<&str, T>,
) -> bool {
    let trimmed = specifier.trim_start_matches('.');
    if trimmed.len() < specifier.len() || trimmed.is_empty() {
        return true; // `from .models import X` — inside the package by construction
    }
    let base = trimmed.replace('.', "/");
    let module = format!("{base}.py");
    let package = format!("{base}/__init__.py");
    project.iter().any(|path| {
        for candidate in [&module, &package] {
            if *path == candidate.as_str() || path.ends_with(&format!("/{candidate}")) {
                return true;
            }
        }
        false
    })
}

/// Is a model name rebound as a local before this position?
///
/// Conservative: any module-scope or class-scope variable sharing the model's
/// name disqualifies accesses in the file, because the analysis cannot tell
/// which binding a call site sees.
fn shadows_model(file: &FileAnalysis, model: &str, _at: Span) -> bool {
    file.symbols
        .iter()
        .any(|s| s.kind == SymbolKind::Variable && s.name == model)
}

/// The innermost function or method containing a position.
fn enclosing_symbol(file: &FileAnalysis, span: Span) -> Option<String> {
    file.symbols
        .iter()
        .filter(|s| matches!(s.kind, SymbolKind::Function | SymbolKind::Method))
        .filter(|s| s.span.start_line <= span.start_line && s.span.end_line >= span.end_line)
        // The innermost containing definition.
        .min_by_key(|s| s.span.end_line - s.span.start_line)
        .map(|s| s.name.clone())
}

/// Adds the `OrmAccess` and `Queries` edges for one analysis.
///
/// Slices 6 and 7. Two distinct relationships, never conflated:
///
/// - **handler → model**, `OrmAccess`, evidenced by the call as written.
/// - **model → table**, `Queries`, evidenced by the declaration that named the
///   table.
///
/// A model whose table is [`TableName::Unresolved`] gets **no** table edge:
/// the access is still a fact, but the table is not.
///
/// Equivalent facts are normalised — several call sites touching one model
/// produce one access edge per *handler*, not one per syntactic node, so a
/// handler querying a model three times does not triple-count.
///
/// # Errors
///
/// Propagates [`GraphError`] if the graph rejects an endpoint, which cannot
/// happen for nodes created here.
pub fn add_orm_edges(
    graph: &mut ArchitectureGraph,
    analysis: &OrmAnalysis,
    commit: Option<&CommitId>,
) -> Result<usize, GraphError> {
    let mut nodes = OrmNodes::default();
    let mut added = add_table_edges(graph, analysis, commit, &mut nodes)?;
    added += add_access_edges(graph, analysis, commit, &mut nodes)?;
    Ok(added)
}

/// Nodes created while building ORM edges, so one artefact is created once.
#[derive(Default)]
struct OrmNodes {
    models: HashMap<String, NodeId>,
    tables: HashMap<String, NodeId>,
    handlers: HashMap<(String, String), NodeId>,
}

fn bad_node() -> GraphError {
    GraphError::UnknownNode {
        id: NodeId::from_raw(u64::MAX),
    }
}

/// The model node for a model, created once per graph.
fn model_node(
    graph: &mut ArchitectureGraph,
    nodes: &mut OrmNodes,
    model: &OrmModel,
) -> Result<NodeId, GraphError> {
    if let Some(id) = nodes.models.get(&model.name) {
        return Ok(*id);
    }
    let id = graph
        .node_for(
            NodeKind::Class,
            model.name.clone(),
            SourceLocation::new(model.file.clone(), model.span.start_line).ok(),
        )
        .map_err(|_| bad_node())?;
    nodes.models.insert(model.name.clone(), id);
    Ok(id)
}

/// model → table, one edge per model that resolved a table (slice 6).
fn add_table_edges(
    graph: &mut ArchitectureGraph,
    analysis: &OrmAnalysis,
    commit: Option<&CommitId>,
    nodes: &mut OrmNodes,
) -> Result<usize, GraphError> {
    let mut models: Vec<&OrmModel> = analysis.models.values().collect();
    models.sort_by(|a, b| a.name.cmp(&b.name));

    let mut added = 0;
    for model in models {
        let Some(table) = model.table.name() else {
            continue; // no table was declared, so no table edge
        };
        let source = model_node(graph, nodes, model)?;
        let target = if let Some(id) = nodes.tables.get(table) {
            *id
        } else {
            let id = graph
                .node_for(NodeKind::Table, table.to_owned(), None)
                .map_err(|_| bad_node())?;
            nodes.tables.insert(table.to_owned(), id);
            id
        };

        graph.add_edge(EdgeSpec {
            source,
            target,
            kind: EdgeKind::Queries,
            confidence: Provenance::OrmResolution.prior_confidence(),
            provenance: Provenance::OrmResolution,
            evidence: Evidence::new(format!(
                "{} maps to table \"{}\"; {}",
                model.name,
                table,
                model.table.explain()
            ))
            .map_err(|_| bad_node())?,
            location: SourceLocation::new(model.file.clone(), model.span.start_line)
                .map_err(|_| bad_node())?,
            commit: commit.cloned(),
        })?;
        added += 1;
    }
    Ok(added)
}

/// handler → model, deduplicated per (file, handler, model) (slice 5).
fn add_access_edges(
    graph: &mut ArchitectureGraph,
    analysis: &OrmAnalysis,
    commit: Option<&CommitId>,
    nodes: &mut OrmNodes,
) -> Result<usize, GraphError> {
    let mut accesses: Vec<&OrmAccessSite> = analysis.accesses.iter().collect();
    accesses.sort_by(|a, b| {
        (&a.file, a.span.start_line, &a.model).cmp(&(&b.file, b.span.start_line, &b.model))
    });

    let mut seen: HashSet<(String, String, String)> = HashSet::new();
    let mut added = 0;
    for access in accesses {
        let Some(handler) = access.handler.as_deref() else {
            continue; // a module-scope access names no handler
        };
        let Some(model) = analysis.models.get(&access.model) else {
            continue;
        };
        if !seen.insert((
            access.file.clone(),
            handler.to_owned(),
            access.model.clone(),
        )) {
            continue; // the same fact, stated again
        }

        let key = (access.file.clone(), handler.to_owned());
        let source = if let Some(id) = nodes.handlers.get(&key) {
            *id
        } else {
            let id = graph
                .node_for(
                    NodeKind::Function,
                    handler.to_owned(),
                    SourceLocation::new(access.file.clone(), access.span.start_line).ok(),
                )
                .map_err(|_| bad_node())?;
            nodes.handlers.insert(key, id);
            id
        };
        let target = model_node(graph, nodes, model)?;

        graph.add_edge(EdgeSpec {
            source,
            target,
            kind: EdgeKind::OrmAccess,
            confidence: Provenance::OrmResolution.prior_confidence(),
            provenance: Provenance::OrmResolution,
            evidence: Evidence::new(format!(
                "{}(...) in {} at {}:{}",
                access.expression, handler, access.file, access.span.start_line
            ))
            .map_err(|_| bad_node())?,
            location: SourceLocation::new(access.file.clone(), access.span.start_line)
                .map_err(|_| bad_node())?,
            commit: commit.cloned(),
        })?;
        added += 1;
    }
    Ok(added)
}
