//! TypeScript / TSX fact extraction.
//!
//! The only module in the workspace that speaks tree-sitter. Nothing
//! tree-sitter-shaped crosses this boundary: the public surface of the crate
//! deals exclusively in [`crate::model`] types, so replacing the parsing
//! runtime — or adding Python at M02 — touches implementation, not consumers.
//!
//! # Recognition heuristics, documented
//!
//! - **Call sites** are recorded only when the callee is a plain identifier or
//!   an unbroken property chain (`a.b.c(…)`). Computed members, subscripts and
//!   immediately-invoked expressions are skipped: an unreliable fact is worse
//!   than an absent one.
//! - **HTTP-looking calls** are observed (never resolved) when they match:
//!   `fetch(url, …)`; `<obj>.<verb>(url, …)` where `<verb>` is an HTTP method
//!   name and either `<obj>` is one of `axios`/`client`/`http`/`api` or the
//!   URL argument is a string/template starting with `/`, `http://` or
//!   `https://`. `map.get("key")` is therefore not observed unless its
//!   argument looks like a path — and a stray observation is harmless because
//!   observations only become edges after the resolver matches them against
//!   real backend routes (M04).
//! - **Variables** are extracted at module scope only (what M05's constant
//!   propagation needs).
//! - **Strings inside concatenation chains** are represented by the chain's
//!   `Concatenation` fact rather than duplicated as individual facts.

use tree_sitter::{Node, Parser};

use crate::diagnostics::{Diagnostic, DiagnosticKind, Severity};
use crate::error::ParserError;
use crate::model::{
    CallSite, Callee, ConcatPart, ExportStatus, FileAnalysis, HttpCallObservation, HttpMethodHint,
    Import, ModuleAlias, NamedImport, ParseStatus, SourceLanguage, StringFact, Symbol, SymbolKind,
    TemplatePart, UrlObservation,
};
use crate::syntax::{has_keyword_child, span_of};

/// Per-file cap on recorded diagnostics; a `Truncated` marker notes overflow.
const DIAGNOSTIC_CAP: usize = 25;

/// Object names whose `.get/.post/…` calls are observed as HTTP regardless of
/// what the URL argument looks like.
const HTTP_CLIENT_OBJECTS: [&str; 4] = ["axios", "client", "http", "api"];

/// Which of the two grammars this extractor should use.
///
/// A dedicated enum rather than [`SourceLanguage`]: it makes handing Python to
/// the TypeScript extractor impossible to express, instead of a runtime branch
/// that has to decide what to do about it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TsFlavor {
    TypeScript,
    Tsx,
}

impl TsFlavor {
    fn language(self) -> SourceLanguage {
        match self {
            Self::TypeScript => SourceLanguage::TypeScript,
            Self::Tsx => SourceLanguage::Tsx,
        }
    }
}

pub(crate) struct TypeScriptExtractor {
    typescript: Parser,
    tsx: Parser,
}

impl TypeScriptExtractor {
    pub(crate) fn new() -> Result<Self, ParserError> {
        let mut typescript = Parser::new();
        typescript
            .set_language(&tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into())
            .map_err(|e| ParserError::Grammar {
                language: "typescript",
                detail: e.to_string(),
            })?;
        let mut tsx = Parser::new();
        tsx.set_language(&tree_sitter_typescript::LANGUAGE_TSX.into())
            .map_err(|e| ParserError::Grammar {
                language: "tsx",
                detail: e.to_string(),
            })?;
        Ok(Self { typescript, tsx })
    }

    pub(crate) fn extract(&mut self, path: &str, flavor: TsFlavor, source: &str) -> FileAnalysis {
        let language = flavor.language();
        let parser = match flavor {
            TsFlavor::TypeScript => &mut self.typescript,
            TsFlavor::Tsx => &mut self.tsx,
        };

        let mut analysis = FileAnalysis {
            path: path.to_owned(),
            language,
            status: ParseStatus::Complete,
            imports: Vec::new(),
            symbols: Vec::new(),
            calls: Vec::new(),
            strings: Vec::new(),
            http_calls: Vec::new(),
            routes: Vec::new(),
            routers: Vec::new(),
            router_inclusions: Vec::new(),
            module_aliases: Vec::new(), // TypeScript route declarations: not M02's scope
            diagnostics: Vec::new(),
        };

        // tree-sitter is error-tolerant: it returns a tree for any input.
        // `parse` yields None only for cancellation/timeouts, which M01 does
        // not configure.
        let Some(tree) = parser.parse(source, None) else {
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
            export: ExportContext::None,
            deferred_exports: Vec::new(),
        };
        walker.walk(root);
        let deferred = std::mem::take(&mut walker.deferred_exports);
        apply_deferred_exports(&mut analysis, &deferred);
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

/// Export context inherited from a wrapping `export` statement.
#[derive(Clone, Copy, PartialEq)]
enum ExportContext {
    None,
    Exported,
    Default,
}

impl ExportContext {
    fn status(self) -> ExportStatus {
        match self {
            Self::None => ExportStatus::NotExported,
            Self::Exported => ExportStatus::Exported,
            Self::Default => ExportStatus::DefaultExported,
        }
    }
}

struct Walker<'a> {
    source: &'a str,
    analysis: &'a mut FileAnalysis,
    /// Names of enclosing named symbols (class, function, …).
    scope: Vec<String>,
    /// > 0 while inside a `+` chain already represented by a Concatenation.
    concat_depth: u32,
    /// Export context for the immediate declaration being visited.
    export: ExportContext,
    /// `export { a, b as c }` / `export default ident` clauses, applied to
    /// module-scope symbols after the walk.
    deferred_exports: Vec<(String, ExportStatus)>,
}

/// Is this argument a handler rather than a request option?
///
/// A handler is a function written in place, a reference to one, or a call
/// that returns one. A request's trailing arguments are options — an object,
/// a string, a number — never any of those.
///
/// # Why a call counts
///
/// The M07 acceptance audit found `PostHog`'s Node service registering
/// `app.get('/_health', buildGetHealth(services))`, which was matched to a
/// Python health endpoint in a different service. A factory that returns a
/// handler is still passing a handler; how it was produced is not the
/// question. This is the third shape of the same family, after the inline
/// function and the named reference.
///
/// This predicate never decides anything on its own. It is consulted only
/// where the extractor has already established that the callee is a verb-named
/// member of a receiver that is **not** a known HTTP client, which is what
/// keeps `http.get(url, makeCallback())` a request.
fn is_handler_argument(node: &Node<'_>) -> bool {
    matches!(
        node.kind(),
        "arrow_function"
            | "function_expression"
            | "function"
            | "generator_function"
            | "identifier"
            | "member_expression"
            | "call_expression"
    )
}

impl Walker<'_> {
    fn walk(&mut self, node: Node<'_>) {
        let export = std::mem::replace(&mut self.export, ExportContext::None);

        match node.kind() {
            "import_statement" => {
                self.extract_import(node);
                // Do not descend: the specifier string is recorded on the
                // Import, not duplicated as a string fact.
                return;
            }
            "export_statement" => {
                self.extract_export(node);
                return;
            }
            "pair" => self.extract_module_aliases(node),
            "function_declaration" | "generator_function_declaration" => {
                self.extract_function(node, SymbolKind::Function, export);
                return;
            }
            "class_declaration" | "abstract_class_declaration" => {
                self.extract_class(node, export);
                return;
            }
            "method_definition" => {
                self.extract_function(node, SymbolKind::Method, ExportContext::None);
                return;
            }
            "interface_declaration" => {
                self.extract_named_only(node, SymbolKind::Interface, export);
            }
            "type_alias_declaration" => {
                self.extract_named_only(node, SymbolKind::TypeAlias, export);
            }
            "enum_declaration" => {
                self.extract_named_only(node, SymbolKind::Enum, export);
            }
            "lexical_declaration" | "variable_declaration" => {
                self.extract_variables(node, export);
                // Still descend: initialisers contain calls and strings.
            }
            "call_expression" => self.extract_call(node, false),
            "new_expression" => self.extract_call(node, true),
            "string" => {
                if self.concat_depth == 0 {
                    let fact = StringFact::Literal {
                        value: string_content(node, self.source),
                        span: span_of(node),
                    };
                    self.analysis.strings.push(fact);
                }
            }
            "template_string" => {
                if self.concat_depth == 0 {
                    let fact = StringFact::Template {
                        parts: template_parts(node, self.source),
                        span: span_of(node),
                    };
                    self.analysis.strings.push(fact);
                }
                // Descend anyway: substitutions may contain calls.
            }
            // The guard has a side effect (recording the fact) by design:
            // the arm only fires when a concatenation was actually recorded.
            "binary_expression" if self.concat_depth == 0 && self.extract_concatenation(node) => {
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
        node.utf8_text(self.source.as_bytes())
            .unwrap_or_default()
            .to_owned()
    }

    // ── Imports ──────────────────────────────────────────────────────

    fn extract_import(&mut self, node: Node<'_>) {
        let Some(source_node) = node.child_by_field_name("source") else {
            return; // side-effect-free `import "x"` still has a source; bail on anything else
        };
        let mut import = Import {
            specifier: string_content(source_node, self.source),
            default_name: None,
            namespace_name: None,
            named: Vec::new(),
            type_only: has_keyword_child(node, "type"),
            span: span_of(node),
        };

        let mut cursor = node.walk();
        for child in node.named_children(&mut cursor) {
            if child.kind() != "import_clause" {
                continue;
            }
            let mut clause_cursor = child.walk();
            for part in child.named_children(&mut clause_cursor) {
                match part.kind() {
                    "identifier" => import.default_name = Some(self.text(part)),
                    "namespace_import" => {
                        let mut ns_cursor = part.walk();
                        if let Some(name) = part
                            .named_children(&mut ns_cursor)
                            .find(|n| n.kind() == "identifier")
                        {
                            import.namespace_name = Some(self.text(name));
                        }
                    }
                    "named_imports" => {
                        let mut named_cursor = part.walk();
                        for spec in part.named_children(&mut named_cursor) {
                            if spec.kind() != "import_specifier" {
                                continue;
                            }
                            let imported = spec
                                .child_by_field_name("name")
                                .map(|n| self.text(n))
                                .unwrap_or_default();
                            let local = spec.child_by_field_name("alias").map(|n| self.text(n));
                            if !imported.is_empty() {
                                import.named.push(NamedImport { imported, local });
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
        self.analysis.imports.push(import);
    }

    // ── Exports ──────────────────────────────────────────────────────

    fn extract_export(&mut self, node: Node<'_>) {
        let is_default = has_keyword_child(node, "default");

        // `export * from "./queries"` and `export { X } from "./m"` — a barrel
        // file. It declares nothing itself and forwards everything, so without
        // recording it an importer of the barrel reaches a dead end. Airflow's
        // generated query layer is exactly this: `openapi-gen/queries/index.ts`
        // is two wildcard re-exports and nothing else.
        //
        // Recorded as an import, because that is what it is from the
        // resolver's point of view: this module binds those names from
        // another. A wildcard uses the namespace binding `*`, the same
        // representation the Python extractor gives `from x import *`.
        if let Some(source) = node.child_by_field_name("source") {
            let specifier = string_content(source, self.source);
            let mut named = Vec::new();
            let mut wildcard = false;
            let mut cursor = node.walk();
            for child in node.named_children(&mut cursor) {
                match child.kind() {
                    "export_clause" => {
                        let mut inner = child.walk();
                        for spec in child.named_children(&mut inner) {
                            if spec.kind() != "export_specifier" {
                                continue;
                            }
                            let Some(name) = spec.child_by_field_name("name") else {
                                continue;
                            };
                            named.push(NamedImport {
                                imported: self.text(name),
                                local: spec.child_by_field_name("alias").map(|a| self.text(a)),
                            });
                        }
                    }
                    "*" => wildcard = true,
                    _ => {}
                }
            }
            if named.is_empty() && !wildcard {
                // `export * as ns from "./m"` and anything else unrecognised.
                wildcard = true;
            }
            self.analysis.imports.push(Import {
                specifier,
                default_name: None,
                namespace_name: wildcard.then(|| "*".to_owned()),
                named,
                type_only: has_keyword_child(node, "type"),
                span: span_of(node),
            });
            return;
        }

        if let Some(declaration) = node.child_by_field_name("declaration") {
            self.export = if is_default {
                ExportContext::Default
            } else {
                ExportContext::Exported
            };
            self.walk(declaration);
            return;
        }

        // `export default <expression>;`
        if is_default {
            if let Some(value) = node.child_by_field_name("value") {
                match value.kind() {
                    // `export default someName;` marks the earlier declaration.
                    "identifier" => {
                        self.deferred_exports
                            .push((self.text(value), ExportStatus::DefaultExported));
                    }
                    // Anonymous defaults parse as *expressions*, not
                    // declarations; they still deserve a symbol, named
                    // `default` because that is the only name they have.
                    "function_expression" | "generator_function" | "arrow_function" => {
                        self.push_symbol(
                            SymbolKind::Function,
                            "default",
                            has_keyword_child(value, "async"),
                            ExportContext::Default,
                            value,
                        );
                    }
                    "class" => {
                        self.push_symbol(
                            SymbolKind::Class,
                            "default",
                            false,
                            ExportContext::Default,
                            value,
                        );
                    }
                    _ => {}
                }
                self.walk(value);
            }
            return;
        }

        // `export { a, b as c }` — marks existing module-scope symbols.
        let mut cursor = node.walk();
        for child in node.named_children(&mut cursor) {
            if child.kind() != "export_clause" {
                continue;
            }
            let mut clause_cursor = child.walk();
            for spec in child.named_children(&mut clause_cursor) {
                if spec.kind() != "export_specifier" {
                    continue;
                }
                if let Some(name) = spec.child_by_field_name("name") {
                    self.deferred_exports
                        .push((self.text(name), ExportStatus::Exported));
                }
            }
        }
    }

    // ── Symbols ──────────────────────────────────────────────────────

    fn extract_function(&mut self, node: Node<'_>, kind: SymbolKind, export: ExportContext) {
        let name = node
            .child_by_field_name("name")
            .map(|n| self.text(n))
            .filter(|n| !n.is_empty())
            .unwrap_or_else(|| {
                if export == ExportContext::Default {
                    "default".to_owned()
                } else {
                    String::new()
                }
            });
        if name.is_empty() {
            self.walk_children(node);
            return;
        }

        self.push_symbol(kind, &name, has_keyword_child(node, "async"), export, node);
        self.scope.push(name);
        if let Some(body) = node.child_by_field_name("body") {
            self.walk(body);
        }
        self.scope.pop();
    }

    fn extract_class(&mut self, node: Node<'_>, export: ExportContext) {
        let name = node
            .child_by_field_name("name")
            .map(|n| self.text(n))
            .filter(|n| !n.is_empty())
            .unwrap_or_else(|| {
                if export == ExportContext::Default {
                    "default".to_owned()
                } else {
                    String::new()
                }
            });
        if name.is_empty() {
            self.walk_children(node);
            return;
        }

        self.push_symbol(SymbolKind::Class, &name, false, export, node);
        self.scope.push(name);
        if let Some(body) = node.child_by_field_name("body") {
            self.walk_children(body);
        }
        self.scope.pop();
    }

    fn extract_named_only(&mut self, node: Node<'_>, kind: SymbolKind, export: ExportContext) {
        if let Some(name) = node.child_by_field_name("name") {
            let name = self.text(name);
            self.push_symbol(kind, &name, false, export, node);
        }
    }

    fn extract_variables(&mut self, node: Node<'_>, export: ExportContext) {
        // Module scope only: locals are noise for every downstream consumer.
        if !self.scope.is_empty() {
            return;
        }
        let mut cursor = node.walk();
        for declarator in node.named_children(&mut cursor) {
            if declarator.kind() != "variable_declarator" {
                continue;
            }
            if let Some(name) = declarator.child_by_field_name("name") {
                if name.kind() == "identifier" {
                    let name = self.text(name);
                    self.push_symbol(SymbolKind::Variable, &name, false, export, declarator);
                }
                // Destructuring patterns are skipped in M01 (documented).
            }
        }
    }

    fn push_symbol(
        &mut self,
        kind: SymbolKind,
        name: &str,
        is_async: bool,
        export: ExportContext,
        node: Node<'_>,
    ) {
        self.analysis.symbols.push(Symbol {
            kind,
            name: name.to_owned(),
            is_async,
            enclosing: self.scope.last().cloned(),
            export: export.status(),
            // TypeScript decorator extraction is not part of M02; the field is
            // shared, and TS fills it when a milestone needs it.
            decorators: Vec::new(),
            // TypeScript class heritage is not extracted yet; the field is
            // shared and TS fills it when a milestone needs it.
            bases: Vec::new(),
            span: span_of(node),
        });
    }

    // ── Calls ────────────────────────────────────────────────────────

    fn extract_call(&mut self, node: Node<'_>, is_new: bool) {
        let callee_field = if is_new { "constructor" } else { "function" };
        let Some(callee_node) = node.child_by_field_name(callee_field) else {
            return;
        };
        let Some(callee) = self.callee_shape(callee_node) else {
            // The callee is not a plain identifier or an unbroken property
            // chain — a parenthesised expression, a computed member, a call in
            // the chain. No CallSite is recorded, because naming that callee
            // would be a guess.
            //
            // A request configuration object is different: its URL is stated
            // literally and does not depend on identifying the callee at all.
            // full-stack-fastapi's generated SDK writes every request as
            // `(options?.client ?? client).get({ url: '/api/v1/items/' })`,
            // and refusing the whole call for the sake of the callee's shape
            // discards a fact the source states plainly.
            if !is_new {
                self.observe_http_from_config(callee_node, node);
            }
            return;
        };

        let arguments = node.child_by_field_name("arguments");
        let argument_count = arguments.map_or(0, |a| {
            let mut cursor = a.walk();
            u32::try_from(a.named_children(&mut cursor).count()).unwrap_or(u32::MAX)
        });

        let call = CallSite {
            callee: callee.clone(),
            is_new,
            argument_count,
            span: span_of(node),
        };
        self.analysis.calls.push(call);

        if !is_new {
            self.observe_http(&callee, arguments, node);
        }
    }

    /// A plain identifier or unbroken property chain, or `None`.
    fn callee_shape(&self, node: Node<'_>) -> Option<Callee> {
        match node.kind() {
            "identifier" => Some(Callee::Identifier {
                name: self.text(node),
            }),
            "member_expression" => {
                let property = self.text(node.child_by_field_name("property")?);
                let mut object = Vec::new();
                let mut current = node.child_by_field_name("object")?;
                loop {
                    match current.kind() {
                        "identifier" | "this" => {
                            object.push(self.text(current));
                            break;
                        }
                        "member_expression" => {
                            let prop = current.child_by_field_name("property")?;
                            object.push(self.text(prop));
                            current = current.child_by_field_name("object")?;
                        }
                        _ => return None, // computed / call in the chain
                    }
                }
                object.reverse();
                Some(Callee::Member { object, property })
            }
            _ => None,
        }
    }

    // ── HTTP observations ────────────────────────────────────────────

    fn observe_http(&mut self, callee: &Callee, arguments: Option<Node<'_>>, call_node: Node<'_>) {
        let Some(args) = arguments else { return };
        let mut cursor = args.walk();
        let arg_nodes: Vec<_> = args.named_children(&mut cursor).collect();
        let Some(first) = arg_nodes.first().copied() else {
            return;
        };

        // A request described by a configuration object rather than by
        // positional arguments. Generated clients overwhelmingly write this
        // shape, and M07 measured the cost of not reading it: of 1,458
        // HttpCall edges across seven repositories only twelve had a
        // TypeScript client, because every generated SDK in the corpus hides
        // its URL in an options object.
        //
        //     __request(OpenAPI, { method: 'POST', url: '/api/v2/pools' })
        //     (options.client ?? client).post({ url: '/api/v1/items/' })
        //     SupersetClient.get({ endpoint: '/api/v1/chart/' })
        //
        // The evidence is the literal URL itself, so this does not depend on
        // recognising a generator, a client library or a callee name.
        if let Some((url, method)) = self.request_config(&arg_nodes) {
            // A literal `method` property wins. Failing that, a callee named
            // for a verb states the method just as plainly: `client.get({url})`
            // is a GET. Neither is an inference — both are written down.
            let method = method.or_else(|| match callee {
                Callee::Member { property, .. } => HttpMethodHint::from_name(property),
                Callee::Identifier { .. } => None,
            });
            self.analysis.http_calls.push(HttpCallObservation {
                callee: callee.to_string(),
                method_hint: method,
                url,
                enclosing: self.scope.last().cloned(),
                span: span_of(call_node),
            });
            return;
        }

        let (is_http, method_hint) = match callee {
            Callee::Identifier { name } if name == "fetch" => (
                true,
                arg_nodes
                    .get(1)
                    .and_then(|n| self.literal_method_property(*n)),
            ),
            Callee::Member { object, property } => {
                let Some(verb) = HttpMethodHint::from_name(property) else {
                    return;
                };
                let known_client =
                    object.len() == 1 && HTTP_CLIENT_OBJECTS.contains(&object[0].as_str());
                // Express and Hono register a route by passing handlers,
                // and the syntax is identical to a client call:
                //
                //     app.get('/_health', (c) => c.json({ok: true}))
                //     app.get('/_metrics', getMetrics)
                //     app.get('/_health', buildGetHealth(services))
                //     app.get('/x', requireAuth, handleX)
                //
                // Every argument after the path is a handler; none is a
                // request option. A request passes options — an object, a
                // string, a number — or nothing at all.
                //
                // The rule applies only to a receiver that is *not* a known
                // HTTP client, because Node's own `http.get(url, callback)`
                // passes a function and is a real request. That receiver
                // check, not the argument shape alone, is what keeps this
                // contextual.
                let all_handlers =
                    arg_nodes.len() > 1 && arg_nodes.iter().skip(1).all(is_handler_argument);
                if !known_client && all_handlers {
                    return;
                }
                if known_client || self.looks_like_path(first) {
                    (true, Some(verb))
                } else {
                    return;
                }
            }
            Callee::Identifier { .. } => return,
        };
        if !is_http {
            return;
        }

        let url = match first.kind() {
            "string" => UrlObservation::Literal {
                value: string_content(first, self.source),
            },
            "template_string" => UrlObservation::Template {
                parts: template_parts(first, self.source),
            },
            _ => UrlObservation::Dynamic {
                source: self.text(first),
            },
        };

        self.analysis.http_calls.push(HttpCallObservation {
            callee: callee.to_string(),
            method_hint,
            url,
            enclosing: self.scope.last().cloned(),
            span: span_of(call_node),
        });
    }

    /// `resolve: { alias: { openapi: "/openapi-gen" } }` in a build config.
    ///
    /// Read only under a `resolve` property, so an unrelated object with a key
    /// called `alias` — a database column mapping, a CLI flag table — cannot
    /// be mistaken for module resolution. Only string values are taken;
    /// anything computed is not a fact this can read.
    fn extract_module_aliases(&mut self, pair: Node<'_>) {
        let Some(key) = pair.child_by_field_name("key") else {
            return;
        };
        let key_name = match key.kind() {
            "property_identifier" => self.text(key),
            "string" => string_content(key, self.source),
            _ => return,
        };
        if key_name != "resolve" {
            return;
        }
        let Some(resolve_object) = pair.child_by_field_name("value") else {
            return;
        };
        if resolve_object.kind() != "object" {
            return;
        }
        let Some(alias_object) = self.literal_property(resolve_object, &["alias"]) else {
            return;
        };
        if alias_object.kind() != "object" {
            return;
        }

        let mut cursor = alias_object.walk();
        for entry in alias_object.named_children(&mut cursor) {
            if entry.kind() != "pair" {
                continue;
            }
            let (Some(name), Some(value)) = (
                entry.child_by_field_name("key"),
                entry.child_by_field_name("value"),
            ) else {
                continue;
            };
            let alias = match name.kind() {
                "property_identifier" => self.text(name),
                "string" => string_content(name, self.source),
                _ => continue,
            };
            if value.kind() != "string" {
                continue;
            }
            self.analysis.module_aliases.push(ModuleAlias {
                alias,
                target: string_content(value, self.source),
                span: span_of(entry),
            });
        }
    }

    /// Records a configuration-object request whose callee could not be named.
    ///
    /// The callee is recorded as the invoked property when there is one, so
    /// the observation still says what was called, and never as raw source.
    fn observe_http_from_config(&mut self, callee_node: Node<'_>, call_node: Node<'_>) {
        let Some(arguments) = call_node.child_by_field_name("arguments") else {
            return;
        };
        let mut cursor = arguments.walk();
        let arg_nodes: Vec<_> = arguments.named_children(&mut cursor).collect();
        let Some((url, method)) = self.request_config(&arg_nodes) else {
            return;
        };
        let callee = callee_node
            .child_by_field_name("property")
            .map_or_else(|| "(unnamed)".to_owned(), |p| self.text(p));
        let method = method.or_else(|| HttpMethodHint::from_name(&callee));
        self.analysis.http_calls.push(HttpCallObservation {
            callee,
            method_hint: method,
            url,
            enclosing: self.scope.last().cloned(),
            span: span_of(call_node),
        });
    }

    /// A request configuration object among a call's arguments.
    ///
    /// Returns the URL and, when the object states one literally, the method.
    ///
    /// # What this deliberately will not read
    ///
    /// Only `url` and `endpoint` are accepted as the URL key. `path` is not:
    /// React Router, Vue Router and every routing table in the corpus write
    /// `{ path: "/orders", element: … }`, and reading those would turn a
    /// frontend's own route table into outbound HTTP calls — a false positive
    /// class larger than the true positives this exists to find.
    ///
    /// The value must be a literal or template whose text begins a URL path,
    /// which is the same discriminating evidence the positional forms use.
    fn request_config(
        &self,
        arguments: &[Node<'_>],
    ) -> Option<(UrlObservation, Option<HttpMethodHint>)> {
        for argument in arguments {
            if argument.kind() != "object" {
                continue;
            }
            let url_node = self.literal_property(*argument, &["url", "endpoint"])?;
            if !self.looks_like_path(url_node) {
                continue;
            }
            let url = match url_node.kind() {
                "string" => UrlObservation::Literal {
                    value: string_content(url_node, self.source),
                },
                _ => UrlObservation::Template {
                    parts: template_parts(url_node, self.source),
                },
            };
            let method = self
                .literal_property(*argument, &["method"])
                .filter(|n| n.kind() == "string")
                .and_then(|n| HttpMethodHint::from_name(&string_content(n, self.source)));
            return Some((url, method));
        }
        None
    }

    /// The value node of the first of `keys` this object states.
    fn literal_property<'t>(&self, object: Node<'t>, keys: &[&str]) -> Option<Node<'t>> {
        let mut cursor = object.walk();
        for pair in object.named_children(&mut cursor) {
            if pair.kind() != "pair" {
                continue;
            }
            let Some(key) = pair.child_by_field_name("key") else {
                continue;
            };
            let name = match key.kind() {
                "property_identifier" => self.text(key),
                "string" => string_content(key, self.source),
                _ => continue,
            };
            if keys.contains(&name.as_str()) {
                return pair.child_by_field_name("value");
            }
        }
        None
    }

    /// Does this argument syntactically look like a URL path?
    fn looks_like_path(&self, node: Node<'_>) -> bool {
        let head = match node.kind() {
            "string" => string_content(node, self.source),
            "template_string" => match template_parts(node, self.source).first() {
                Some(TemplatePart::Text { value }) => value.clone(),
                _ => return false,
            },
            _ => return false,
        };
        head.starts_with('/') || head.starts_with("http://") || head.starts_with("https://")
    }

    /// `{ method: "POST", … }` → the hinted method, if literally present.
    fn literal_method_property(&self, node: Node<'_>) -> Option<HttpMethodHint> {
        if node.kind() != "object" {
            return None;
        }
        let mut cursor = node.walk();
        for pair in node.named_children(&mut cursor) {
            if pair.kind() != "pair" {
                continue;
            }
            let key = pair.child_by_field_name("key")?;
            let key_text = match key.kind() {
                "property_identifier" => self.text(key),
                "string" => string_content(key, self.source),
                _ => continue,
            };
            if key_text != "method" {
                continue;
            }
            let value = pair.child_by_field_name("value")?;
            if value.kind() == "string" {
                return HttpMethodHint::from_name(&string_content(value, self.source));
            }
            return None; // method present but not a literal: unknown stays unknown
        }
        None
    }

    // ── Concatenation ────────────────────────────────────────────────

    /// Flattens a `+` chain; records it if any operand is a string literal.
    /// Returns whether a fact was recorded.
    fn extract_concatenation(&mut self, node: Node<'_>) -> bool {
        if !is_plus_expression(node, self.source) {
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
        if is_plus_expression(node, self.source) {
            if let (Some(left), Some(right)) = (
                node.child_by_field_name("left"),
                node.child_by_field_name("right"),
            ) {
                self.flatten_concat(left, parts, has_string);
                self.flatten_concat(right, parts, has_string);
                return;
            }
        }
        match node.kind() {
            "string" => {
                *has_string = true;
                parts.push(ConcatPart::Literal {
                    value: string_content(node, self.source),
                });
            }
            _ => parts.push(ConcatPart::Expression {
                source: self.text(node),
            }),
        }
    }
}

// ── Free helpers (no walker state) ──────────────────────────────────

/// The content of a `string` node (fragments and escapes, quotes excluded).
fn string_content(node: Node<'_>, source: &str) -> String {
    let mut out = String::new();
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if matches!(child.kind(), "string_fragment" | "escape_sequence") {
            out.push_str(child.utf8_text(source.as_bytes()).unwrap_or_default());
        }
    }
    out
}

/// The parts of a `template_string` node, in source order.
fn template_parts(node: Node<'_>, source: &str) -> Vec<TemplatePart> {
    let mut parts = Vec::new();
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        match child.kind() {
            "string_fragment" | "escape_sequence" => {
                let text = child.utf8_text(source.as_bytes()).unwrap_or_default();
                // Merge adjacent text (fragment + escape + fragment).
                if let Some(TemplatePart::Text { value }) = parts.last_mut() {
                    value.push_str(text);
                } else {
                    parts.push(TemplatePart::Text {
                        value: text.to_owned(),
                    });
                }
            }
            "template_substitution" => {
                let mut sub_cursor = child.walk();
                if let Some(expr) = child.named_children(&mut sub_cursor).next() {
                    parts.push(TemplatePart::Expression {
                        source: expr
                            .utf8_text(source.as_bytes())
                            .unwrap_or_default()
                            .to_owned(),
                        span: span_of(expr),
                    });
                }
            }
            _ => {}
        }
    }
    parts
}

fn is_plus_expression(node: Node<'_>, source: &str) -> bool {
    node.kind() == "binary_expression"
        && node
            .child_by_field_name("operator")
            .is_some_and(|op| op.utf8_text(source.as_bytes()) == Ok("+"))
}

/// `export { a }` / `export default ident` applied after the walk.
fn apply_deferred_exports(analysis: &mut FileAnalysis, deferred: &[(String, ExportStatus)]) {
    for (name, status) in deferred {
        for symbol in &mut analysis.symbols {
            if symbol.enclosing.is_none()
                && &symbol.name == name
                && symbol.export == ExportStatus::NotExported
            {
                symbol.export = *status;
            }
        }
    }
}

/// Error/missing nodes become diagnostics; extraction of the rest continues.
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
            // Do not descend into an error region's internals.
            continue;
        }
        if node.is_missing() {
            analysis.diagnostics.push(Diagnostic {
                severity: Severity::Error,
                kind: DiagnosticKind::MissingToken,
                // node.kind() is a grammar token name, never source text.
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
