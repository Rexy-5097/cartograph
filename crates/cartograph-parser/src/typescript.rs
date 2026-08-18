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
    Import, NamedImport, ParseStatus, SourceLanguage, Span, StringFact, Symbol, SymbolKind,
    TemplatePart, UrlObservation,
};

/// Per-file cap on recorded diagnostics; a `Truncated` marker notes overflow.
const DIAGNOSTIC_CAP: usize = 25;

/// Object names whose `.get/.post/…` calls are observed as HTTP regardless of
/// what the URL argument looks like.
const HTTP_CLIENT_OBJECTS: [&str; 4] = ["axios", "client", "http", "api"];

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

    pub(crate) fn extract(
        &mut self,
        path: &str,
        language: SourceLanguage,
        source: &str,
    ) -> FileAnalysis {
        let parser = match language {
            SourceLanguage::TypeScript => &mut self.typescript,
            SourceLanguage::Tsx => &mut self.tsx,
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
            "binary_expression" => {
                if self.concat_depth == 0 && self.extract_concatenation(node) {
                    self.concat_depth += 1;
                    self.walk_children(node);
                    self.concat_depth -= 1;
                    return;
                }
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
            return; // not reliably identifiable — skip, don't guess
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
            span: span_of(call_node),
        });
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

fn span_of(node: Node<'_>) -> Span {
    let start = node.start_position();
    let end = node.end_position();
    #[allow(clippy::cast_possible_truncation)] // >4G-line files are not a real input
    Span {
        start_line: start.row as u32 + 1,
        start_column: start.column as u32 + 1,
        end_line: end.row as u32 + 1,
        end_column: end.column as u32 + 1,
    }
}

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

/// Does the node have a bare keyword token child with this text?
fn has_keyword_child(node: Node<'_>, keyword: &str) -> bool {
    let mut cursor = node.walk();
    node.children(&mut cursor)
        .any(|c| !c.is_named() && c.kind() == keyword)
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
