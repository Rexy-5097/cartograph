//! Python fact extraction.
//!
//! Peer of [`crate::typescript`]: a different grammar producing the *same*
//! language-neutral facts. Nothing tree-sitter-shaped crosses this boundary,
//! and nothing here resolves anything — imports stay unresolved strings, calls
//! stay syntactic, f-strings stay symbolic, and route declarations stay
//! observations that no resolver has looked at yet.
//!
//! # Recognition heuristics, documented
//!
//! - **Call sites** are recorded only for a plain identifier or an unbroken
//!   attribute chain (`pkg.mod.fn(…)`). Subscripts and calls-of-calls are
//!   skipped: an unreliable fact is worse than an absent one.
//! - **HTTP-looking calls** follow M01's discipline. `<obj>.<verb>(…)` is
//!   observed when `<obj>` is a known client (`requests`, `httpx`, `session`,
//!   `client`, `http`, `aiohttp`) *or* the first argument looks like a path
//!   (`/…`, `http://`, `https://`). So `config.get("timeout")` and
//!   `os.environ.get("HOME")` are not HTTP; `api.post("/orders")` is.
//! - **Routes** are recognised from verb decorators (`@app.get`), method-list
//!   decorators (`@app.route`) and URL-conf entries (`path(...)`). The
//!   observation records the *syntax* matched, not a framework: Flask 2.x and
//!   `FastAPI` share verb-decorator syntax exactly. Method and path come from
//!   the source only; an undeclared method stays empty rather than defaulting.
//! - **Variables** are module scope only, matching the TypeScript extractor.
//! - **Exports** are [`ExportStatus::NotApplicable`] unless the module declares
//!   `__all__`, which is Python's genuine export list.

use tree_sitter::{Node, Parser};

use crate::diagnostics::{Diagnostic, DiagnosticKind, Severity};
use crate::error::ParserError;
use crate::model::{
    CallSite, Callee, ConcatPart, ExportStatus, FileAnalysis, HttpCallObservation, HttpMethodHint,
    Import, NamedImport, ParseStatus, RouteDeclarationStyle, RouteObservation, RouterDeclaration,
    RouterInclusion, SourceLanguage, StringFact, Symbol, SymbolKind, TemplatePart, UrlObservation,
};
use crate::syntax::{span_of, text_of};

/// Per-file cap on recorded diagnostics; matches the TypeScript extractor.
const DIAGNOSTIC_CAP: usize = 25;

/// Objects whose verb-named methods are observed as HTTP regardless of the
/// argument's shape.
const HTTP_CLIENT_OBJECTS: [&str; 6] =
    ["requests", "httpx", "session", "client", "http", "aiohttp"];

/// Decorator receivers that declare routes in FastAPI-style frameworks.
const ROUTE_DECORATOR_OBJECTS: [&str; 4] = ["app", "router", "api", "blueprint"];

pub(crate) struct PythonExtractor {
    parser: Parser,
}

impl PythonExtractor {
    pub(crate) fn new() -> Result<Self, ParserError> {
        let mut parser = Parser::new();
        parser
            .set_language(&tree_sitter_python::LANGUAGE.into())
            .map_err(|e| ParserError::Grammar {
                language: "python",
                detail: e.to_string(),
            })?;
        Ok(Self { parser })
    }

    pub(crate) fn extract(&mut self, path: &str, source: &str) -> FileAnalysis {
        let mut analysis = FileAnalysis {
            path: path.to_owned(),
            language: SourceLanguage::Python,
            status: ParseStatus::Complete,
            imports: Vec::new(),
            symbols: Vec::new(),
            calls: Vec::new(),
            strings: Vec::new(),
            http_calls: Vec::new(),
            routes: Vec::new(),
            routers: Vec::new(),
            router_inclusions: Vec::new(),
            module_aliases: Vec::new(),
            diagnostics: Vec::new(),
        };

        let Some(tree) = self.parser.parse(source, None) else {
            analysis.status = ParseStatus::Failed;
            analysis.diagnostics.push(Diagnostic {
                severity: Severity::Error,
                kind: DiagnosticKind::SyntaxError,
                message: "parser returned no tree".to_owned(),
                span: None,
            });
            return analysis;
        };

        let root = tree.root_node();
        let mut walker = Walker {
            source,
            analysis: &mut analysis,
            scope: Vec::new(),
            concat_depth: 0,
            exported_names: Vec::new(),
        };
        walker.walk(root);
        let exported = std::mem::take(&mut walker.exported_names);
        apply_dunder_all(&mut analysis, &exported);
        collect_error_diagnostics(root, &mut analysis);

        if analysis
            .diagnostics
            .iter()
            .any(|d| d.severity == Severity::Error)
        {
            analysis.status = ParseStatus::CompleteWithErrors;
        }
        analysis
    }
}

struct Walker<'a> {
    source: &'a str,
    analysis: &'a mut FileAnalysis,
    /// Names of enclosing named symbols (class, function, …).
    scope: Vec<String>,
    /// > 0 while inside a `+` chain already represented by a Concatenation.
    concat_depth: u32,
    /// Names listed in a module-level `__all__`.
    exported_names: Vec<String>,
}

impl Walker<'_> {
    fn walk(&mut self, node: Node<'_>) {
        match node.kind() {
            "import_statement" | "import_from_statement" => {
                self.extract_import(node);
                return; // specifier strings belong to the Import, not to `strings`
            }
            "decorated_definition" => {
                self.extract_decorated(node);
                return;
            }
            "function_definition" => {
                self.extract_function(node, Vec::new());
                return;
            }
            "class_definition" => {
                self.extract_class(node, Vec::new());
                return;
            }
            "assignment" => self.extract_assignment(node),
            "call" => self.extract_call(node),
            "string" => {
                if self.concat_depth == 0 {
                    self.extract_string(node);
                }
                // Descend anyway: f-string substitutions may contain calls.
            }
            // The guard has a side effect (recording the fact) by design: the
            // arm only fires when a concatenation was actually recorded.
            "binary_operator" if self.concat_depth == 0 && self.extract_concatenation(node) => {
                self.concat_depth += 1;
                self.walk_children(node);
                self.concat_depth -= 1;
                return;
            }
            _ => {}
        }
        self.walk_children(node);
    }

    fn walk_children(&mut self, node: Node<'_>) {
        let mut cursor = node.walk();
        let children: Vec<_> = node.named_children(&mut cursor).collect();
        for child in children {
            self.walk(child);
        }
    }

    fn text(&self, node: Node<'_>) -> String {
        text_of(node, self.source)
    }

    // ── Imports ──────────────────────────────────────────────────────

    fn extract_import(&mut self, node: Node<'_>) {
        let span = span_of(node);

        if node.kind() == "import_statement" {
            // `import a.b`, `import a as b`, `import a, b`
            let mut cursor = node.walk();
            for child in node.named_children(&mut cursor) {
                let (module, alias) = match child.kind() {
                    "dotted_name" | "identifier" => (self.text(child), None),
                    "aliased_import" => {
                        let name = child
                            .child_by_field_name("name")
                            .map(|n| self.text(n))
                            .unwrap_or_default();
                        let alias = child.child_by_field_name("alias").map(|n| self.text(n));
                        (name, alias)
                    }
                    _ => continue,
                };
                if module.is_empty() {
                    continue;
                }
                self.analysis.imports.push(Import {
                    specifier: module,
                    // `import x as y` binds the module itself, which is what a
                    // namespace import means in the shared model.
                    default_name: None,
                    namespace_name: alias,
                    named: Vec::new(),
                    type_only: false,
                    span,
                });
            }
            return;
        }

        // `from <module> import <names>` — including relative forms.
        let module_node = node.child_by_field_name("module_name");
        let specifier = module_node.map(|n| self.text(n)).unwrap_or_default();
        // A bare `from . import x` has relative dots but no module name.
        let specifier = if specifier.is_empty() {
            relative_prefix(node, self.source)
        } else {
            specifier
        };

        let mut import = Import {
            specifier,
            default_name: None,
            namespace_name: None,
            named: Vec::new(),
            type_only: false,
            span,
        };

        let mut cursor = node.walk();
        for child in node.named_children(&mut cursor) {
            if Some(child) == module_node {
                continue;
            }
            match child.kind() {
                "wildcard_import" => {
                    // `from x import *`: the shared model's namespace binding
                    // is the closest true statement — every public name is
                    // bound, and no individual name is known.
                    import.namespace_name = Some("*".to_owned());
                }
                "dotted_name" | "identifier" => {
                    import.named.push(NamedImport {
                        imported: self.text(child),
                        local: None,
                    });
                }
                "aliased_import" => {
                    let imported = child
                        .child_by_field_name("name")
                        .map(|n| self.text(n))
                        .unwrap_or_default();
                    let local = child.child_by_field_name("alias").map(|n| self.text(n));
                    if !imported.is_empty() {
                        import.named.push(NamedImport { imported, local });
                    }
                }
                _ => {}
            }
        }
        self.analysis.imports.push(import);
    }

    // ── Symbols and decorators ───────────────────────────────────────

    fn extract_decorated(&mut self, node: Node<'_>) {
        let mut decorators = Vec::new();
        let mut cursor = node.walk();
        for child in node.named_children(&mut cursor) {
            if child.kind() == "decorator" {
                if let Some(name) = self.decorator_callee(child) {
                    decorators.push(name);
                }
                self.extract_route_decorator(child, node);
            }
        }

        if let Some(definition) = node.child_by_field_name("definition") {
            match definition.kind() {
                "function_definition" => self.extract_function(definition, decorators),
                "class_definition" => self.extract_class(definition, decorators),
                _ => self.walk(definition),
            }
        }
    }

    /// `@app.get("/x")` → `app.get`; `@staticmethod` → `staticmethod`.
    fn decorator_callee(&self, decorator: Node<'_>) -> Option<String> {
        let mut cursor = decorator.walk();
        let inner = decorator.named_children(&mut cursor).next()?;
        let target = if inner.kind() == "call" {
            inner.child_by_field_name("function")?
        } else {
            inner
        };
        match target.kind() {
            "identifier" | "attribute" | "dotted_name" => Some(self.text(target)),
            _ => None,
        }
    }

    fn extract_function(&mut self, node: Node<'_>, decorators: Vec<String>) {
        let Some(name_node) = node.child_by_field_name("name") else {
            self.walk_children(node);
            return;
        };
        let name = self.text(name_node);
        // A `def` directly inside a class body is a method.
        let kind = if Self::in_class_body(node) {
            SymbolKind::Method
        } else {
            SymbolKind::Function
        };
        let is_async =
            node.prev_sibling().is_some_and(|s| s.kind() == "async") || node_has_async(node);

        self.push_symbol(kind, &name, is_async, decorators, node);
        self.scope.push(name);
        if let Some(body) = node.child_by_field_name("body") {
            self.walk_children(body);
        }
        self.scope.pop();
    }

    fn extract_class(&mut self, node: Node<'_>, decorators: Vec<String>) {
        let Some(name_node) = node.child_by_field_name("name") else {
            self.walk_children(node);
            return;
        };
        let name = self.text(name_node);
        let bases = self.class_bases(node);
        self.push_symbol_with_bases(SymbolKind::Class, &name, false, decorators, bases, node);
        self.scope.push(name);
        if let Some(body) = node.child_by_field_name("body") {
            self.walk_children(body);
        }
        self.scope.pop();
    }

    /// Is this assignment a class-body attribute?
    ///
    /// An assignment nests one level deeper than a definition — through an
    /// `expression_statement` — so it needs its own check rather than reusing
    /// [`Self::in_class_body`].
    fn is_class_attribute(node: Node<'_>) -> bool {
        let mut current = node;
        // Walk out of the statement wrapper, then the block, to the owner.
        for _ in 0..3 {
            let Some(parent) = current.parent() else {
                return false;
            };
            if parent.kind() == "class_definition" {
                return true;
            }
            current = parent;
        }
        false
    }

    /// Is this definition's immediate parent a class body?
    fn in_class_body(node: Node<'_>) -> bool {
        let mut current = node;
        // Step over a wrapping decorated_definition.
        if let Some(parent) = current.parent() {
            if parent.kind() == "decorated_definition" {
                current = parent;
            }
        }
        current
            .parent()
            .and_then(|b| b.parent())
            .is_some_and(|p| p.kind() == "class_definition")
    }

    fn extract_assignment(&mut self, node: Node<'_>) {
        // Module scope, or a class body. A class-body assignment is
        // declarative metadata — `__tablename__ = "orders"`, Django's
        // `db_table` — and reading it is the difference between resolving a
        // table and guessing which string in a class happens to be one.
        //
        // Assignments inside a function are still skipped: a local's value
        // depends on control flow this analysis does not model.
        if !self.scope.is_empty() && !Self::is_class_attribute(node) {
            return;
        }
        let Some(left) = node.child_by_field_name("left") else {
            return;
        };
        if left.kind() != "identifier" {
            return; // tuple/attribute targets are not recorded in M02
        }
        let name = self.text(left);

        self.extract_router_declaration(node, &name);

        if name == "__all__" {
            if let Some(right) = node.child_by_field_name("right") {
                self.collect_dunder_all(right);
            }
        }
        self.push_symbol(SymbolKind::Variable, &name, false, Vec::new(), node);
    }

    fn collect_dunder_all(&mut self, node: Node<'_>) {
        if !matches!(node.kind(), "list" | "tuple") {
            return;
        }
        let mut cursor = node.walk();
        for child in node.named_children(&mut cursor) {
            if child.kind() == "string" {
                let value = string_content(child, self.source);
                if !value.is_empty() {
                    self.exported_names.push(value);
                }
            }
        }
    }

    /// The base classes named in a class declaration's argument list.
    fn class_bases(&self, node: Node<'_>) -> Vec<String> {
        let Some(args) = node.child_by_field_name("superclasses") else {
            return Vec::new();
        };
        let mut cursor = args.walk();
        args.named_children(&mut cursor)
            .filter(|c| matches!(c.kind(), "identifier" | "attribute"))
            .map(|c| self.text(c))
            .collect()
    }

    fn push_symbol(
        &mut self,
        kind: SymbolKind,
        name: &str,
        is_async: bool,
        decorators: Vec<String>,
        node: Node<'_>,
    ) {
        self.push_symbol_with_bases(kind, name, is_async, decorators, Vec::new(), node);
    }

    #[allow(clippy::too_many_arguments)]
    fn push_symbol_with_bases(
        &mut self,
        kind: SymbolKind,
        name: &str,
        is_async: bool,
        decorators: Vec<String>,
        bases: Vec<String>,
        node: Node<'_>,
    ) {
        self.analysis.symbols.push(Symbol {
            kind,
            name: name.to_owned(),
            is_async,
            enclosing: self.scope.last().cloned(),
            // Python has no export statement; `__all__` upgrades this later.
            export: ExportStatus::NotApplicable,
            decorators,
            bases,
            span: span_of(node),
        });
    }

    // ── Calls ────────────────────────────────────────────────────────

    fn extract_call(&mut self, node: Node<'_>) {
        let Some(function) = node.child_by_field_name("function") else {
            return;
        };
        let Some(callee) = self.callee_shape(function) else {
            return; // not reliably identifiable — skip, don't guess
        };

        let arguments = node.child_by_field_name("arguments");
        let argument_count = arguments.map_or(0, |a| {
            let mut cursor = a.walk();
            u32::try_from(a.named_children(&mut cursor).count()).unwrap_or(u32::MAX)
        });

        self.analysis.calls.push(CallSite {
            callee: callee.clone(),
            // Python has no `new`: instantiation is an ordinary call, and
            // claiming otherwise would need to know what the name refers to.
            is_new: false,
            argument_count,
            span: span_of(node),
        });

        self.extract_django_route(&callee, arguments, node);
        self.extract_router_inclusion(&callee, arguments, node);
        self.observe_http(&callee, arguments, node);
    }

    /// `router = APIRouter(prefix="/pools")`.
    ///
    /// Any constructor whose name ends in `Router` counts, because projects
    /// subclass it: Airflow's routes are declared on `AirflowRouter`, and
    /// requiring the exact name `APIRouter` would miss every one of them.
    /// The suffix is the evidence, and a constructor that is not called at all
    /// declares nothing.
    fn extract_router_declaration(&mut self, assignment: Node<'_>, name: &str) {
        let Some(right) = assignment.child_by_field_name("right") else {
            return;
        };
        if right.kind() != "call" {
            return;
        }
        let Some(function) = right.child_by_field_name("function") else {
            return;
        };
        let constructor = match self.callee_shape(function) {
            Some(Callee::Identifier { name }) => name,
            Some(Callee::Member { property, .. }) => property,
            None => return,
        };
        if !constructor.ends_with("Router") {
            return;
        }
        let arguments = right.child_by_field_name("arguments");
        let (prefix, prefix_unresolved) = self.keyword_string(arguments, "prefix");
        self.analysis.routers.push(RouterDeclaration {
            name: name.to_owned(),
            prefix,
            prefix_unresolved,
            constructor,
            span: span_of(assignment),
        });
    }

    /// `parent.include_router(child, prefix="/x")`.
    ///
    /// Records the mounting, not a resolved path. Which routers these names
    /// refer to, and what prefix the chain composes to, is the resolver's
    /// question — this file may not even declare them.
    fn extract_router_inclusion(
        &mut self,
        callee: &Callee,
        arguments: Option<Node<'_>>,
        call: Node<'_>,
    ) {
        let Callee::Member { object, property } = callee else {
            return;
        };
        if property != "include_router" {
            return;
        }
        let Some(args) = arguments else { return };
        let mut cursor = args.walk();
        let positional: Vec<_> = args
            .named_children(&mut cursor)
            .filter(|n| n.kind() != "keyword_argument")
            .collect();
        // The child router must be named, not built inline: an expression here
        // is a router this analysis cannot identify.
        let Some(child) = positional
            .first()
            .filter(|n| matches!(n.kind(), "identifier" | "attribute"))
        else {
            return;
        };
        let (prefix, prefix_unresolved) = self.keyword_string(arguments, "prefix");
        self.analysis.router_inclusions.push(RouterInclusion {
            parent: object.join("."),
            child: self.text(*child),
            prefix,
            prefix_unresolved,
            span: span_of(call),
        });
    }

    /// A `keyword=` argument's string literal, and whether it was present but
    /// not literal.
    ///
    /// The two are different facts. Absent means "no prefix"; present but
    /// dynamic means "a prefix exists and this analysis cannot read it", and
    /// composing that as empty would produce a wrong path.
    fn keyword_string(&self, arguments: Option<Node<'_>>, keyword: &str) -> (Option<String>, bool) {
        let Some(args) = arguments else {
            return (None, false);
        };
        let mut cursor = args.walk();
        for arg in args.named_children(&mut cursor) {
            if arg.kind() != "keyword_argument" {
                continue;
            }
            let named = arg
                .child_by_field_name("name")
                .is_some_and(|n| self.text(n) == keyword);
            if !named {
                continue;
            }
            let Some(value) = arg.child_by_field_name("value") else {
                return (None, true);
            };
            if value.kind() == "string" {
                return (Some(string_content(value, self.source)), false);
            }
            return (None, true);
        }
        (None, false)
    }

    /// A plain identifier or unbroken attribute chain, or `None`.
    fn callee_shape(&self, node: Node<'_>) -> Option<Callee> {
        match node.kind() {
            "identifier" => Some(Callee::Identifier {
                name: self.text(node),
            }),
            "attribute" => {
                let property = self.text(node.child_by_field_name("attribute")?);
                let mut object = Vec::new();
                let mut current = node.child_by_field_name("object")?;
                loop {
                    match current.kind() {
                        "identifier" => {
                            object.push(self.text(current));
                            break;
                        }
                        "attribute" => {
                            object.push(self.text(current.child_by_field_name("attribute")?));
                            current = current.child_by_field_name("object")?;
                        }
                        _ => return None, // subscript, call, literal receiver
                    }
                }
                object.reverse();
                Some(Callee::Member { object, property })
            }
            _ => None,
        }
    }

    // ── Routes ───────────────────────────────────────────────────────

    /// FastAPI/Flask decorator routes: `@app.get("/x")`, `@app.route("/x", methods=[…])`.
    fn extract_route_decorator(&mut self, decorator: Node<'_>, decorated: Node<'_>) {
        let mut cursor = decorator.walk();
        let Some(call) = decorator.named_children(&mut cursor).next() else {
            return;
        };
        if call.kind() != "call" {
            return; // a bare `@staticmethod` declares no route
        }
        let Some(function) = call.child_by_field_name("function") else {
            return;
        };
        let Some(Callee::Member { object, property }) = self.callee_shape(function) else {
            return;
        };
        let arguments = call.child_by_field_name("arguments");
        let Some(path_node) = arguments.and_then(|a| {
            let mut c = a.walk();
            a.named_children(&mut c)
                .find(|n| n.kind() != "keyword_argument")
        }) else {
            return; // no positional path argument: nothing to observe
        };

        // `@app.get`, `@router.post`, `@bp.route`, … — a single receiver whose
        // name is a known route registrar.
        let known_registrar = object.len() == 1
            && (ROUTE_DECORATOR_OBJECTS.contains(&object[0].as_str())
                || object[0].ends_with("_router")
                || object[0].ends_with("_app")
                || object[0] == "bp");
        // A blueprint may be bound to any name at all. Superset registers four
        // routes on `health_blueprint`, and requiring the name to be on a list
        // lost every one of them at M07. The discriminating evidence is the
        // argument, not the receiver: a decorator named for an HTTP verb whose
        // first positional argument is a literal URL path is a route
        // declaration whatever the object is called. This is the same rule the
        // TypeScript extractor uses to decide that `svc.get("/orders")` is an
        // HTTP call while `cache.get(key)` is not.
        if !known_registrar && !self.is_path_literal(path_node) {
            return;
        }

        let (style, methods) = if property == "route" {
            // Methods live in a `methods=[…]` keyword argument.
            (
                RouteDeclarationStyle::RouteDecorator,
                self.route_decorator_methods(arguments),
            )
        } else if let Some(verb) = HttpMethodHint::from_name(&property) {
            (RouteDeclarationStyle::VerbDecorator, vec![verb])
        } else {
            return; // `@app.middleware("http")` and friends declare no route
        };

        let handler = decorated
            .child_by_field_name("definition")
            .and_then(|d| d.child_by_field_name("name"))
            .map(|n| self.text(n));

        self.analysis.routes.push(RouteObservation {
            style,
            methods,
            receiver: Some(object.join(".")),
            path: self.url_observation(path_node),
            handler,
            span: span_of(decorator),
        });
    }

    /// Is this argument a string literal that looks like a URL path?
    ///
    /// Deliberately narrow: a leading slash. A decorator argument that is a
    /// variable, an f-string or a dotted name is not evidence of a route, and
    /// `@mock.patch("app.core.settings")` must never read as one.
    fn is_path_literal(&self, node: Node<'_>) -> bool {
        if !matches!(node.kind(), "string" | "concatenated_string") {
            return false;
        }
        string_content(node, self.source).starts_with('/')
    }

    /// `methods=["GET", "POST"]` → the declared methods. Absent or non-literal
    /// stays empty: the source did not say (RULE 009). Flask implies GET for a
    /// bare `@app.route("/x")`, but that is framework semantics — inference,
    /// not observation.
    fn route_decorator_methods(&self, arguments: Option<Node<'_>>) -> Vec<HttpMethodHint> {
        let Some(args) = arguments else {
            return Vec::new();
        };
        let mut cursor = args.walk();
        for arg in args.named_children(&mut cursor) {
            if arg.kind() != "keyword_argument" {
                continue;
            }
            let is_methods = arg
                .child_by_field_name("name")
                .is_some_and(|n| self.text(n) == "methods");
            if !is_methods {
                continue;
            }
            let Some(value) = arg.child_by_field_name("value") else {
                return Vec::new();
            };
            if !matches!(value.kind(), "list" | "tuple") {
                return Vec::new(); // a variable: unknown, not guessed
            }
            let mut list_cursor = value.walk();
            return value
                .named_children(&mut list_cursor)
                .filter(|n| n.kind() == "string")
                .filter_map(|n| HttpMethodHint::from_name(&string_content(n, self.source)))
                .collect();
        }
        Vec::new()
    }

    /// Django URL conf: `path("orders/", create_order)` / `re_path(r"^x$", view)`.
    fn extract_django_route(
        &mut self,
        callee: &Callee,
        arguments: Option<Node<'_>>,
        call: Node<'_>,
    ) {
        let name = match callee {
            Callee::Identifier { name } => name.as_str(),
            // `django.urls.path(...)` also appears qualified.
            Callee::Member { property, .. } => property.as_str(),
        };
        if !matches!(name, "path" | "re_path") {
            return;
        }
        let Some(args) = arguments else { return };
        let mut cursor = args.walk();
        let positional: Vec<_> = args
            .named_children(&mut cursor)
            .filter(|n| n.kind() != "keyword_argument")
            .collect();
        let Some(&path_node) = positional.first() else {
            return;
        };
        // A route needs a string-ish pattern; `path(x, y)` with a variable
        // pattern is too weak to be worth recording as a route observation.
        if !matches!(path_node.kind(), "string" | "concatenated_string") {
            return;
        }

        let handler = positional.get(1).and_then(|h| match h.kind() {
            // `create_order` or `views.create_order`, kept as written.
            "identifier" | "attribute" => Some(self.text(*h)),
            // `LegacyView.as_view()` — record the callable being invoked.
            "call" => h
                .child_by_field_name("function")
                .map(|f| text_of(f, self.source)),
            _ => None,
        });

        self.analysis.routes.push(RouteObservation {
            style: RouteDeclarationStyle::UrlConfEntry,
            // A url-conf entry is not registered on a router object.
            receiver: None,
            // A URL conf entry declares no HTTP method; the view decides.
            methods: Vec::new(),
            path: self.url_observation(path_node),
            handler,
            span: span_of(call),
        });
    }

    // ── HTTP observations ────────────────────────────────────────────

    fn observe_http(&mut self, callee: &Callee, arguments: Option<Node<'_>>, call: Node<'_>) {
        let Callee::Member { object, property } = callee else {
            return; // bare `get(...)` is not evidence of an HTTP client
        };
        let Some(verb) = HttpMethodHint::from_name(property) else {
            return;
        };
        let Some(args) = arguments else { return };
        let mut cursor = args.walk();
        let Some(first) = args
            .named_children(&mut cursor)
            .find(|n| n.kind() != "keyword_argument")
        else {
            return;
        };

        let known_client = object
            .last()
            .is_some_and(|o| HTTP_CLIENT_OBJECTS.contains(&o.as_str()));
        if !known_client && !self.looks_like_path(first) {
            return;
        }

        self.analysis.http_calls.push(HttpCallObservation {
            callee: callee.to_string(),
            method_hint: Some(verb),
            url: self.url_observation(first),
            enclosing: self.scope.last().cloned(),
            span: span_of(call),
        });
    }

    fn looks_like_path(&self, node: Node<'_>) -> bool {
        let head = match node.kind() {
            "string" => match self.url_observation(node) {
                UrlObservation::Literal { value } => value,
                UrlObservation::Template { parts } => match parts.first() {
                    Some(TemplatePart::Text { value }) => value.clone(),
                    _ => return false,
                },
                UrlObservation::Dynamic { .. } => return false,
            },
            _ => return false,
        };
        head.starts_with('/') || head.starts_with("http://") || head.starts_with("https://")
    }

    /// A string node as a URL/path observation; anything else stays dynamic.
    fn url_observation(&self, node: Node<'_>) -> UrlObservation {
        match node.kind() {
            "string" => {
                let parts = template_parts(node, self.source);
                if parts
                    .iter()
                    .any(|p| matches!(p, TemplatePart::Expression { .. }))
                {
                    UrlObservation::Template { parts }
                } else {
                    UrlObservation::Literal {
                        value: string_content(node, self.source),
                    }
                }
            }
            _ => UrlObservation::Dynamic {
                source: self.text(node),
            },
        }
    }

    // ── Strings ──────────────────────────────────────────────────────

    fn extract_string(&mut self, node: Node<'_>) {
        let parts = template_parts(node, self.source);
        let has_substitution = parts
            .iter()
            .any(|p| matches!(p, TemplatePart::Expression { .. }));
        let fact = if has_substitution {
            // An f-string: structure preserved, expressions never evaluated.
            StringFact::Template {
                parts,
                span: span_of(node),
            }
        } else {
            StringFact::Literal {
                value: string_content(node, self.source),
                span: span_of(node),
            }
        };
        self.analysis.strings.push(fact);
    }

    /// Flattens a `+` chain; records it if any operand is a string literal.
    fn extract_concatenation(&mut self, node: Node<'_>) -> bool {
        if !is_plus_operator(node, self.source) {
            return false;
        }
        let mut parts = Vec::new();
        let mut has_string = false;
        self.flatten_concat(node, &mut parts, &mut has_string);
        if !has_string {
            return false;
        }
        self.analysis.strings.push(StringFact::Concatenation {
            parts,
            span: span_of(node),
        });
        true
    }

    fn flatten_concat(&self, node: Node<'_>, parts: &mut Vec<ConcatPart>, has_string: &mut bool) {
        if is_plus_operator(node, self.source) {
            if let (Some(left), Some(right)) = (
                node.child_by_field_name("left"),
                node.child_by_field_name("right"),
            ) {
                self.flatten_concat(left, parts, has_string);
                self.flatten_concat(right, parts, has_string);
                return;
            }
        }
        if node.kind() == "string" {
            *has_string = true;
            parts.push(ConcatPart::Literal {
                value: string_content(node, self.source),
            });
        } else {
            parts.push(ConcatPart::Expression {
                source: self.text(node),
            });
        }
    }
}

// ── Free helpers ────────────────────────────────────────────────────

/// The leading dots of a relative import (`from ..pkg import x` → `..`).
fn relative_prefix(node: Node<'_>, source: &str) -> String {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "relative_import" || child.kind() == "import_prefix" {
            return text_of(child, source);
        }
    }
    String::new()
}

/// Python's `async def` marker, whichever way the grammar attaches it.
fn node_has_async(node: Node<'_>) -> bool {
    let mut cursor = node.walk();
    node.children(&mut cursor)
        .any(|c| !c.is_named() && c.kind() == "async")
}

/// The content of a `string` node: fragments and escapes, quotes and prefix
/// excluded. Substitutions contribute nothing (see [`template_parts`]).
fn string_content(node: Node<'_>, source: &str) -> String {
    let mut out = String::new();
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if matches!(child.kind(), "string_content" | "escape_sequence") {
            out.push_str(child.utf8_text(source.as_bytes()).unwrap_or_default());
        }
    }
    out
}

/// A string node's parts. An f-string yields interleaved text and expressions;
/// a plain string yields a single text part.
///
/// Expressions are preserved as source text with their span — never evaluated.
/// `f"/orders/{order_id}"` stays `/orders/` + `order_id`, and turning that into
/// a concrete path is M05's problem, not M02's.
fn template_parts(node: Node<'_>, source: &str) -> Vec<TemplatePart> {
    let mut parts: Vec<TemplatePart> = Vec::new();
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        match child.kind() {
            "string_content" | "escape_sequence" => {
                let text = child.utf8_text(source.as_bytes()).unwrap_or_default();
                if let Some(TemplatePart::Text { value }) = parts.last_mut() {
                    value.push_str(text);
                } else {
                    parts.push(TemplatePart::Text {
                        value: text.to_owned(),
                    });
                }
            }
            "interpolation" => {
                let mut inner = child.walk();
                if let Some(expr) = child.named_children(&mut inner).next() {
                    parts.push(TemplatePart::Expression {
                        source: text_of(expr, source),
                        span: span_of(expr),
                    });
                }
            }
            _ => {}
        }
    }
    parts
}

fn is_plus_operator(node: Node<'_>, source: &str) -> bool {
    node.kind() == "binary_operator"
        && node
            .child_by_field_name("operator")
            .is_some_and(|op| op.utf8_text(source.as_bytes()) == Ok("+"))
}

/// Upgrades module-scope symbols named in `__all__` to `Exported`.
///
/// `__all__` is Python's genuine export declaration, so honouring it is
/// observation rather than invention. Symbols *not* listed keep
/// [`ExportStatus::NotApplicable`]: their absence from `__all__` affects
/// `from module import *` only, and does not make them un-importable.
fn apply_dunder_all(analysis: &mut FileAnalysis, exported: &[String]) {
    for name in exported {
        for symbol in &mut analysis.symbols {
            if symbol.enclosing.is_none() && &symbol.name == name {
                symbol.export = ExportStatus::Exported;
            }
        }
    }
}

/// Error and missing nodes become diagnostics; extraction of the rest continues.
fn collect_error_diagnostics(root: Node<'_>, analysis: &mut FileAnalysis) {
    if !root.has_error() {
        return;
    }
    let mut stack = vec![root];
    let mut truncated = false;
    while let Some(node) = stack.pop() {
        if analysis.diagnostics.len() >= DIAGNOSTIC_CAP {
            truncated = true;
            break;
        }
        if node.is_error() {
            analysis.diagnostics.push(Diagnostic {
                severity: Severity::Error,
                kind: DiagnosticKind::SyntaxError,
                message: "unparseable region".to_owned(),
                span: Some(span_of(node)),
            });
            continue;
        }
        if node.is_missing() {
            analysis.diagnostics.push(Diagnostic {
                severity: Severity::Error,
                kind: DiagnosticKind::MissingToken,
                // A grammar token name, never source text.
                message: format!("missing `{}`", node.kind()),
                span: Some(span_of(node)),
            });
            continue;
        }
        if node.has_error() {
            let mut cursor = node.walk();
            for child in node.children(&mut cursor) {
                stack.push(child);
            }
        }
    }
    if truncated {
        analysis.diagnostics.push(Diagnostic {
            severity: Severity::Warning,
            kind: DiagnosticKind::Truncated,
            message: format!("more than {DIAGNOSTIC_CAP} diagnostics; remainder dropped"),
            span: None,
        });
    }
}
