//! The extracted-fact model: what an analysed file *says*, language-neutrally.
//!
//! Everything here is an **observed syntactic fact**, not a verified semantic
//! relationship. An [`HttpCallObservation`] is not an `HttpCall` graph edge; a
//! [`CallSite`] is not a resolved call; an [`Import`] specifier is a string the
//! file contains, not a resolved dependency. The resolver (M03–M06) consumes
//! these facts and produces evidenced edges; keeping the two vocabularies
//! separate is central to Cartograph (RULES 007–010).
//!
//! No type in this module names a tree-sitter construct. The model is shaped
//! by what later resolver stages need, so a Python extractor (M02) can produce
//! the same fact types from a different grammar.

use serde::{Deserialize, Serialize};

use crate::diagnostics::Diagnostic;

/// A source language Cartograph can extract facts from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
#[non_exhaustive]
pub enum SourceLanguage {
    /// TypeScript (`.ts`, `.mts`, `.cts`, including `.d.ts`).
    ///
    /// Renamed explicitly: kebab-case would split this into `type-script`,
    /// which is not the word, and Display already says `typescript`.
    #[serde(rename = "typescript")]
    TypeScript,
    /// TypeScript with JSX (`.tsx`).
    Tsx,
}

impl SourceLanguage {
    /// Infers the language from a file path's extension, if supported.
    /// Extension matching is case-insensitive.
    #[must_use]
    pub fn from_path(path: &str) -> Option<Self> {
        let extension = std::path::Path::new(path).extension()?.to_str()?;
        if extension.eq_ignore_ascii_case("tsx") {
            Some(Self::Tsx)
        } else if ["ts", "mts", "cts"]
            .iter()
            .any(|e| extension.eq_ignore_ascii_case(e))
        {
            Some(Self::TypeScript)
        } else {
            None
        }
    }
}

impl std::fmt::Display for SourceLanguage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::TypeScript => f.write_str("typescript"),
            Self::Tsx => f.write_str("tsx"),
        }
    }
}

/// A region of source text.
///
/// Lines and columns are 1-based, matching editors, language servers and the
/// `file.tsx:34` rendering the product promises. `end_column` is exclusive:
/// a span covering the single character at line 1 column 1 has
/// `start_column = 1, end_column = 2`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Span {
    /// 1-based first line.
    pub start_line: u32,
    /// 1-based first column.
    pub start_column: u32,
    /// 1-based last line.
    pub end_line: u32,
    /// 1-based exclusive end column.
    pub end_column: u32,
}

impl std::fmt::Display for Span {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}:{}", self.start_line, self.start_column)
    }
}

/// How far parsing got.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ParseStatus {
    /// The file parsed without syntax errors.
    Complete,
    /// The file parsed, but contains syntax errors. Facts were still extracted
    /// from the well-formed parts — one broken region must not destroy
    /// analysis of the rest.
    CompleteWithErrors,
    /// The file could not be analysed at all (unreadable, or not UTF-8).
    /// `diagnostics` says why; every fact list is empty.
    Failed,
}

/// Everything extracted from one file.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FileAnalysis {
    /// Repository-relative path, forward slashes, validated against absolute
    /// paths and `..` traversal (same rules as the M00 domain model).
    pub path: String,
    /// The language the file was parsed as.
    pub language: SourceLanguage,
    /// How far parsing got.
    pub status: ParseStatus,
    /// Import statements, in source order.
    pub imports: Vec<Import>,
    /// Declared symbols, in source order.
    pub symbols: Vec<Symbol>,
    /// Call sites, in source order.
    pub calls: Vec<CallSite>,
    /// String and template facts, in source order.
    pub strings: Vec<StringFact>,
    /// Syntactically HTTP-looking call sites. Observations only — see
    /// [`HttpCallObservation`].
    pub http_calls: Vec<HttpCallObservation>,
    /// Parse problems, capped per file.
    pub diagnostics: Vec<Diagnostic>,
}

/// An `import` statement.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Import {
    /// The module specifier as written: `"./api"`, `"react"`.
    ///
    /// A string the file contains — not a resolved module. Resolution to a
    /// file or package is later work.
    pub specifier: String,
    /// `import X from "m"` — the default binding, if present.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_name: Option<String>,
    /// `import * as ns from "m"` — the namespace binding, if present.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub namespace_name: Option<String>,
    /// `import { a, b as c } from "m"` — named bindings.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub named: Vec<NamedImport>,
    /// `import type …` — a type-only import.
    pub type_only: bool,
    /// Where the statement is.
    pub span: Span,
}

/// One named binding inside an import.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NamedImport {
    /// The exported name being imported.
    pub imported: String,
    /// The local alias, when `as` renamed it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub local: Option<String>,
}

/// What kind of declaration a symbol is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
#[non_exhaustive]
pub enum SymbolKind {
    /// A function declaration (including generators).
    Function,
    /// A class declaration.
    Class,
    /// A method inside a class.
    Method,
    /// A module-scope `const`/`let`/`var` binding.
    ///
    /// Only module-scope variables are recorded: they are what M05's constant
    /// propagation needs (`const BASE = "/api/v1"`), and recording every local
    /// would drown the useful facts.
    Variable,
    /// An interface declaration.
    Interface,
    /// A type alias.
    TypeAlias,
    /// An enum declaration.
    Enum,
}

/// Whether and how a symbol is exported.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ExportStatus {
    /// Not exported.
    NotExported,
    /// `export …` or listed in an `export { … }` clause.
    Exported,
    /// `export default …`.
    DefaultExported,
}

/// A declared symbol.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Symbol {
    /// What kind of declaration this is.
    pub kind: SymbolKind,
    /// The declared name. An anonymous `export default` declaration is
    /// recorded as `default`.
    pub name: String,
    /// `async` functions and methods.
    pub is_async: bool,
    /// Name of the nearest enclosing named symbol (class for methods, outer
    /// function for nested functions). `None` at module scope.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enclosing: Option<String>,
    /// Whether and how the symbol is exported.
    pub export: ExportStatus,
    /// Where the declaration is.
    pub span: Span,
}

/// The callee shape of a call site, as written.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Callee {
    /// `foo(…)`.
    Identifier {
        /// The identifier.
        name: String,
    },
    /// `a.b.c(…)` — a plain property chain.
    Member {
        /// The object chain, e.g. `["a", "b"]` for `a.b.c(…)`. `this` appears
        /// as the literal text `this`.
        object: Vec<String>,
        /// The final property, e.g. `c`.
        property: String,
    },
}

impl std::fmt::Display for Callee {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Identifier { name } => f.write_str(name),
            Self::Member { object, property } => {
                write!(f, "{}.{property}", object.join("."))
            }
        }
    }
}

/// A syntactic call site.
///
/// **Not** a resolved call. Which declaration `foo` refers to needs semantic
/// resolution (LSP, M04); this records only what the file says. Calls whose
/// callee is not a plain identifier or property chain (computed members,
/// immediately-invoked expressions) are not recorded — an unreliable fact is
/// worse than an absent one (RULE 009).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CallSite {
    /// What is being called, as written.
    pub callee: Callee,
    /// `new Foo(…)` rather than `foo(…)`.
    pub is_new: bool,
    /// How many arguments appear at the call site.
    pub argument_count: u32,
    /// Where the call is.
    pub span: Span,
}

/// One piece of a template literal, in source order.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum TemplatePart {
    /// Literal text between substitutions.
    Text {
        /// The raw text, escapes preserved as written.
        value: String,
    },
    /// A `${…}` substitution. Preserved as source text for M05's restricted
    /// evaluator — **not** evaluated, not guessed at.
    Expression {
        /// The substitution's source text, e.g. `BASE` or `orderId`.
        source: String,
        /// Where the substitution is.
        span: Span,
    },
}

/// One operand of a string concatenation chain.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ConcatPart {
    /// A string literal operand.
    Literal {
        /// The literal's content.
        value: String,
    },
    /// Any other operand, preserved as source text.
    Expression {
        /// The operand's source text.
        source: String,
    },
}

/// A string-shaped fact.
///
/// Import specifiers are recorded on their [`Import`] and not duplicated here.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum StringFact {
    /// A plain string literal.
    Literal {
        /// The content between the quotes, escapes preserved as written.
        value: String,
        /// Where the literal is.
        span: Span,
    },
    /// A template literal, structure preserved for M05. `` `${BASE}/orders` ``
    /// is recorded as its parts — it is **not** resolved to a concrete URL
    /// here, and nothing in M01 pretends otherwise.
    Template {
        /// The parts, in source order.
        parts: Vec<TemplatePart>,
        /// Where the template is.
        span: Span,
    },
    /// A `+` chain in which at least one operand is a string literal.
    Concatenation {
        /// The flattened operands, in source order.
        parts: Vec<ConcatPart>,
        /// Where the whole chain is.
        span: Span,
    },
}

impl StringFact {
    /// Where this fact is.
    #[must_use]
    pub fn span(&self) -> Span {
        match self {
            Self::Literal { span, .. }
            | Self::Template { span, .. }
            | Self::Concatenation { span, .. } => *span,
        }
    }
}

/// An HTTP method suggested by the callee's own name.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum HttpMethodHint {
    /// `.get(…)` or `method: "GET"`.
    Get,
    /// `.post(…)` or `method: "POST"`.
    Post,
    /// `.put(…)` or `method: "PUT"`.
    Put,
    /// `.delete(…)` or `method: "DELETE"`.
    Delete,
    /// `.patch(…)` or `method: "PATCH"`.
    Patch,
    /// `.head(…)` or `method: "HEAD"`.
    Head,
    /// `.options(…)` or `method: "OPTIONS"`.
    Options,
}

impl HttpMethodHint {
    pub(crate) fn from_name(name: &str) -> Option<Self> {
        match name.to_ascii_lowercase().as_str() {
            "get" => Some(Self::Get),
            "post" => Some(Self::Post),
            "put" => Some(Self::Put),
            "delete" => Some(Self::Delete),
            "patch" => Some(Self::Patch),
            "head" => Some(Self::Head),
            "options" => Some(Self::Options),
            _ => None,
        }
    }
}

/// The URL argument of an HTTP-looking call, as observed.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum UrlObservation {
    /// A string literal URL: `fetch("/orders")`.
    Literal {
        /// The literal's content.
        value: String,
    },
    /// A template literal URL, structure preserved for M05.
    Template {
        /// The template's parts, in source order.
        parts: Vec<TemplatePart>,
    },
    /// Anything else — an identifier, a call, a concatenation. Preserved as
    /// source text; explicitly **not** resolved.
    Dynamic {
        /// The argument's source text.
        source: String,
    },
}

/// A syntactically HTTP-looking call site.
///
/// **This is an observation, not a claim.** It asserts only that the file
/// contains a call shaped like an HTTP request. It does not assert that a
/// backend route exists, and it must never become an `HttpCall` graph edge on
/// its own — the resolver (M04) matches observations against extracted backend
/// routes and only then produces an evidenced edge.
///
/// Recognition is a documented syntactic heuristic (see the extractor); the
/// downstream cost of a false observation is bounded because matching happens
/// later against real routes.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HttpCallObservation {
    /// The callee as written, e.g. `fetch` or `axios.get`.
    pub callee: String,
    /// Method suggested by the callee name or a literal `method:` property.
    /// `None` means unknown — it is not defaulted to GET (RULE 009).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub method_hint: Option<HttpMethodHint>,
    /// The first argument, as observed.
    pub url: UrlObservation,
    /// Where the call is.
    pub span: Span,
}
