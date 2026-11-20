//! Restricted abstract interpretation of string expressions.
//!
//! Takes the syntactic facts M01/M02 extracted and resolves as much of a URL
//! as the source actually determines — no more.
//!
//! ```text
//! module constants ─► Scope ─┐
//!                            ├─► evaluate ─► SymbolicValue
//! url expression ────────────┘
//! ```
//!
//! # What this is not
//!
//! Not a JavaScript engine. It does not execute analysed code, call functions,
//! index objects, read the environment, touch the network, or consult a model.
//! It recognises the expression forms listed in [`crate::symbolic`] and returns
//! [`SymbolicValue::Unsupported`] for everything else.
//!
//! That refusal is the design. An interpreter that handles "most" expressions
//! by approximating the rest produces URLs that look right and are not, and
//! every downstream confidence number inherits the approximation.
//!
//! # Recursion is bounded
//!
//! Constant references are resolved through a depth limit, so a cyclic
//! definition (`const A = B; const B = A;`) terminates as `Unknown` rather
//! than exhausting the stack.

use std::collections::{HashMap, HashSet};

use cartograph_parser::model::{
    ConcatPart, FileAnalysis, SourceLanguage, StringFact, Symbol, SymbolKind, TemplatePart,
};

use crate::symbolic::SymbolicValue;

/// How deep constant references are followed before giving up.
///
/// Real configuration chains are two or three deep; anything beyond this is
/// either generated code or a cycle, and both are better reported as unknown
/// than pursued.
const MAX_DEPTH: usize = 8;

/// Module-scope constants available when evaluating an expression.
///
/// Built from one file's own constants, optionally extended with constants
/// imported from other files (slice 3). Bindings are values, not text, so a
/// constant that is itself unresolved stays unresolved through every use.
#[derive(Debug, Default, Clone)]
pub struct Scope {
    bindings: HashMap<String, SymbolicValue>,
    /// Names the file itself binds, whether or not to a known value. An
    /// import of the same name must not supply them (fifth review, W-05,
    /// W-06): the file's own binding, or its uncertainty, wins.
    declared: HashSet<String>,
    /// Names a Python import rebinds inside a function or class body, with
    /// the body's lines: a read there sees the import, not the module
    /// constant (seventh review, Y-06).
    shadows: Vec<(String, u32, u32)>,
}

impl Scope {
    /// An empty scope: every identifier is unknown.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Binds a name to a value.
    pub fn bind(&mut self, name: impl Into<String>, value: SymbolicValue) {
        self.bindings.insert(name.into(), value);
    }

    /// The value bound to a name, if any.
    #[must_use]
    pub fn get(&self, name: &str) -> Option<&SymbolicValue> {
        self.bindings.get(name)
    }

    /// How many names are bound.
    #[must_use]
    pub fn len(&self) -> usize {
        self.bindings.len()
    }

    /// Whether the scope binds nothing.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.bindings.is_empty()
    }

    /// Collects module-scope string constants from one file's facts.
    ///
    /// A module-scope variable is bound when its value *is* a string fact
    /// (the fact's span is the value span the parser records) and nothing
    /// else in the module binds the name. Anything else is left unbound, so
    /// referencing it yields [`SymbolicValue::Unknown`] rather than a guess.
    ///
    /// Without the file's imports, a constant also bound by an import is not
    /// seen as rebound; [`Scope::from_analysis`] sees it.
    #[must_use]
    pub fn from_file(symbols: &[Symbol], strings: &[StringFact]) -> Self {
        Self::collect(symbols, strings, &[], &[], &[], false)
    }

    /// Collects module-scope string constants, as [`Scope::from_file`], also
    /// refusing a name the file's imports rebind: `try: from cfg import P`
    /// `except ImportError: P = "/api"` makes `P` whichever ran (fifth
    /// review, W-06).
    ///
    /// A Python import at module level rebinds a module constant when it
    /// comes after the assignment, or either of them is conditional, or a
    /// module-level read (another constant's value, a request) lies between
    /// them and sees the import (seventh review, Y-05). A Python star import
    /// counts the same way. An import inside a `def` or `class` body rebinds
    /// the name only within that body ([`Scope::at_line`]). A TypeScript
    /// `export * from` binds nothing locally (sixth review, X-04, X-05).
    #[must_use]
    pub fn from_analysis(file: &FileAnalysis) -> Self {
        let python = file.language == SourceLanguage::Python;
        // Python imports inside a function or class body bind that body's
        // names, not the module's. The innermost body is the one whose names
        // they are (eighth review, Z-02).
        let bodies: Vec<(u32, u32)> = file
            .symbols
            .iter()
            .filter(|s| {
                matches!(
                    s.kind,
                    SymbolKind::Function | SymbolKind::Method | SymbolKind::Class
                )
            })
            .map(|s| (s.span.start_line, s.span.end_line))
            .collect();
        // Module-level reads made while the module loads that can feed a
        // URL: a string constant's expressions and a module-level request's
        // URL. Each is kept with the source text it reads (Z-01).
        let strings_by_span: HashMap<_, _> = file.strings.iter().map(|s| (s.span(), s)).collect();
        let mut reads: Vec<(u32, String)> = file
            .symbols
            .iter()
            .filter(|s| s.enclosing.is_none() && s.kind == SymbolKind::Variable)
            .filter_map(|s| {
                let fact = strings_by_span.get(&s.value_span?)?;
                Some((s.span.start_line, fact_sources(fact)))
            })
            .collect();
        // A class body runs while the module loads too (ninth review, AA-03),
        // when the class is written at module level or in another such class
        // body. It is found by position, not by name: a method named like a
        // class is not a class body (tenth review, AB-04).
        let runs_at_load = |line: u32| -> bool {
            let mut containing: Vec<&Symbol> = file
                .symbols
                .iter()
                .filter(|s| s.span.start_line <= line && line <= s.span.end_line)
                .filter(|s| {
                    matches!(
                        s.kind,
                        SymbolKind::Function | SymbolKind::Method | SymbolKind::Class
                    )
                })
                .collect();
            containing.sort_by_key(|s| s.span.end_line - s.span.start_line);
            containing.iter().all(|s| s.kind == SymbolKind::Class)
        };
        reads.extend(
            file.http_calls
                .iter()
                .filter(|c| c.enclosing.is_none() || runs_at_load(c.span.start_line))
                .map(|c| (c.span.start_line, url_sources(&c.url))),
        );
        let mut imported = Vec::new();
        let mut star_lines = Vec::new();
        let mut shadows = Vec::new();
        for import in &file.imports {
            let line = import.span.start_line;
            // `<=`: a one-line `def f(): from cfg import P` (Y-12).
            if python {
                if let Some((start, end)) = bodies
                    .iter()
                    .filter(|(start, end)| *start <= line && line <= *end)
                    .min_by_key(|(start, end)| end - start)
                {
                    for named in &import.named {
                        let name = named.local.as_deref().unwrap_or(named.imported.as_str());
                        shadows.push((name.to_owned(), *start, *end));
                    }
                    if let Some(name) = import.namespace_name.as_deref().filter(|n| *n != "*") {
                        shadows.push((name.to_owned(), *start, *end));
                    }
                    continue;
                }
            }
            let conditional = python && import.span.start_column != 1;
            let mut bind = |name: &str| imported.push((name.to_owned(), line, conditional));
            for named in &import.named {
                bind(named.local.as_deref().unwrap_or(named.imported.as_str()));
            }
            match import.namespace_name.as_deref() {
                Some("*") => {
                    if python {
                        star_lines.push(line);
                    }
                }
                Some(name) => bind(name),
                None => {}
            }
            if let Some(name) = &import.default_name {
                bind(name);
            }
        }
        let mut scope = Self::collect(
            &file.symbols,
            &file.strings,
            &imported,
            &star_lines,
            &reads,
            python,
        );
        scope.shadows = shadows;
        scope
    }

    /// This scope as seen from `line`: a name a function-local import rebinds
    /// in the body containing `line` is unknown there.
    #[must_use]
    pub fn at_line(&self, line: u32) -> std::borrow::Cow<'_, Self> {
        let hidden: Vec<&str> = self
            .shadows
            .iter()
            .filter(|(_, start, end)| *start <= line && line <= *end)
            .map(|(name, _, _)| name.as_str())
            .collect();
        if hidden.is_empty() {
            return std::borrow::Cow::Borrowed(self);
        }
        let mut scope = self.clone();
        for name in hidden {
            scope.bindings.remove(name);
        }
        std::borrow::Cow::Owned(scope)
    }

    fn collect(
        symbols: &[Symbol],
        strings: &[StringFact],
        imported: &[(String, u32, bool)],
        star_lines: &[u32],
        reads: &[(u32, String)],
        python: bool,
    ) -> Self {
        let mut scope = Self::new();
        // Every module-level binding of a name counts: a variable, a `def`,
        // a class, and a `const` inside a block or callback, which the lookup
        // cannot tell from the module's own (R-05). Two exceptions bind no
        // value: a TypeScript `interface` or `type`, which lives in the type
        // namespace, and a Python bare annotation `P: str` (fifth review,
        // W-07). Every name the file binds blocks an import of that name.
        let mut bindings: HashMap<&str, usize> = HashMap::new();
        // An object-literal method has no enclosing symbol but is a property,
        // not a binding (sixth review, X-06).
        for symbol in symbols
            .iter()
            .filter(|s| s.enclosing.is_none() && s.kind != SymbolKind::Method)
        {
            scope.declared.insert(symbol.name.clone());
            let binds_no_value =
                matches!(symbol.kind, SymbolKind::Interface | SymbolKind::TypeAlias)
                    || (python
                        && symbol.kind == SymbolKind::Variable
                        && symbol.value_span.is_none());
            if !binds_no_value {
                *bindings.entry(symbol.name.as_str()).or_default() += 1;
            }
        }
        let strings_by_span: HashMap<_, _> = strings.iter().map(|s| (s.span(), s)).collect();
        for symbol in symbols
            .iter()
            .filter(|s| s.kind == SymbolKind::Variable && s.enclosing.is_none())
        {
            // The value span is the value's own span, with parentheses and
            // type assertions looked through (T-010).
            let Some(value) = symbol.value_span else {
                continue;
            };
            // Bound twice at module level (`BASE = BASE.replace(…)`, a `def`
            // of the same name), by an import, or possibly by a later star
            // import: which value a use sees is not decided here (V-02, V-03,
            // W-06).
            let line = symbol.span.start_line;
            // A Python statement not in column 1 is inside a compound
            // statement (`try`, `if`) and may not run.
            let conditional = python && symbol.span.start_column != 1;
            // A module-level read between an earlier import and this
            // assignment sees the import's value (Y-05, Y-09).
            // Only a read that names the constant matters, and one on the
            // import's own line (after a `;`) counts too (Z-01, Z-04).
            let read_between = |at: u32| {
                reads
                    .iter()
                    .any(|(r, text)| at <= *r && *r < line && mentions(text, &symbol.name))
            };
            let rebound_by_import = imported.iter().any(|(name, at, import_conditional)| {
                name == &symbol.name
                    && (!python
                        || *at > line
                        || *import_conditional
                        || conditional
                        || read_between(*at))
            });
            if bindings.get(symbol.name.as_str()).copied().unwrap_or(0) > 1
                || rebound_by_import
                // A conditional assignment after a star import may not run
                // (ninth review, AA-04).
                || star_lines
                    .iter()
                    .any(|at| *at > line || conditional || read_between(*at))
            {
                continue;
            }
            if let Some(fact) = strings_by_span.get(&value) {
                scope.bind(&symbol.name, from_string_fact(fact));
            }
        }
        scope
    }

    /// Extends this scope with constants a file imports from another.
    ///
    /// `imported` maps a local binding name to the value it refers to in the
    /// exporting module. Only names the importing file actually mentions are
    /// added; nothing is pulled in speculatively.
    pub fn extend_with_imports(
        &mut self,
        imported: impl IntoIterator<Item = (String, SymbolicValue)>,
    ) {
        for (name, value) in imported {
            if self.declared.contains(&name) {
                continue;
            }
            self.bindings.entry(name).or_insert(value);
        }
    }
}

/// The expression source texts in a string fact, joined.
fn fact_sources(fact: &StringFact) -> String {
    match fact {
        StringFact::Literal { .. } => String::new(),
        StringFact::Template { parts, .. } => parts
            .iter()
            .filter_map(|p| match p {
                TemplatePart::Expression { source, .. } => Some(source.as_str()),
                TemplatePart::Text { .. } => None,
            })
            .collect::<Vec<_>>()
            .join(" "),
        StringFact::Concatenation { parts, .. } => parts
            .iter()
            .filter_map(|p| match p {
                ConcatPart::Expression { source } => Some(source.as_str()),
                ConcatPart::Literal { .. } => None,
            })
            .collect::<Vec<_>>()
            .join(" "),
    }
}

/// The expression source texts in a request's URL.
fn url_sources(url: &cartograph_parser::model::UrlObservation) -> String {
    use cartograph_parser::model::UrlObservation;
    match url {
        UrlObservation::Literal { .. } => String::new(),
        UrlObservation::Template { parts } => parts
            .iter()
            .filter_map(|p| match p {
                TemplatePart::Expression { source, .. } => Some(source.as_str()),
                TemplatePart::Text { .. } => None,
            })
            .collect::<Vec<_>>()
            .join(" "),
        UrlObservation::Dynamic { source } => source.clone(),
    }
}

/// Whether `text` names the identifier `name`.
fn mentions(text: &str, name: &str) -> bool {
    text.split(|c: char| !(c.is_alphanumeric() || c == '_'))
        .any(|token| token == name)
}

/// Evaluates a string fact against a scope.
///
/// Slices 2 and 4: template literals and `+` concatenation chains resolve
/// part-by-part, with each substitution evaluated as an expression.
#[must_use]
pub fn evaluate_string_fact(fact: &StringFact, scope: &Scope) -> SymbolicValue {
    from_string_fact_with(fact, scope, 0).normalized()
}

/// Evaluates one expression's source text against a scope.
///
/// The expression grammar is deliberately tiny — an identifier, a dotted
/// environment access, a quoted literal, or a `+` chain of those (T-003).
/// Anything with a call, an index or any other operator is
/// [`SymbolicValue::Unsupported`], because interpreting those correctly needs
/// a real evaluator and interpreting them approximately is worse than
/// declining.
#[must_use]
pub fn evaluate_expression(source: &str, scope: &Scope) -> SymbolicValue {
    evaluate_expression_at(source, scope, 0)
}

fn evaluate_expression_at(source: &str, scope: &Scope, depth: usize) -> SymbolicValue {
    let text = source.trim();

    if depth >= MAX_DEPTH {
        // A cycle, or a chain too deep to be worth following.
        return SymbolicValue::Unknown {
            name: text.to_owned(),
        };
    }
    if text.is_empty() {
        return SymbolicValue::Unsupported {
            source: String::new(),
        };
    }

    // A quoted literal written inside a substitution.
    if let Some(value) = quoted_literal(text) {
        return SymbolicValue::Literal { value };
    }

    // `process.env.NAME`, `import.meta.env.NAME`, `os.environ["NAME"]`.
    if let Some(name) = environment_variable(text) {
        return SymbolicValue::EnvVar { name };
    }

    // A bound module constant. Evaluated through the binding so a constant
    // defined in terms of another resolves transitively.
    if is_plain_identifier(text) {
        return match scope.get(text) {
            Some(value) => resolve_bound(value, scope, depth + 1),
            None => SymbolicValue::Unknown {
                name: text.to_owned(),
            },
        };
    }

    // A member access that is not an environment read: `config.baseUrl`.
    // Resolving it needs object tracking this analysis does not do.
    if text
        .chars()
        .all(|c| c.is_alphanumeric() || "._$".contains(c))
    {
        return SymbolicValue::Unknown {
            name: text.to_owned(),
        };
    }

    // `BASE + "/orders"`: a `+` chain written at the use site (T-003). The
    // parser hands a template literal over in parts, but a concatenation used
    // directly as an argument arrives as source text, and it means the same
    // thing as `${BASE}/orders`. Each operand is evaluated by the rules above;
    // one that is none of them makes the whole value unsupported.
    if let Some(operands) = concatenation_operands(text) {
        return SymbolicValue::Concat {
            parts: operands
                .into_iter()
                .map(|operand| evaluate_expression_at(operand, scope, depth + 1))
                .collect(),
        }
        .normalized();
    }

    SymbolicValue::Unsupported {
        source: text.to_owned(),
    }
}

/// The operands of a top-level `+` chain, if the text is nothing else.
///
/// Deliberately strict, because this reads source text rather than a syntax
/// tree. Outside a quoted literal only identifier characters, `.`, whitespace
/// and `+` are accepted, so a call, an index, a parenthesis, a template
/// literal or any other operator makes the text *not* a chain, and the caller
/// declines it. Inside a literal a backslash does the same: an escape is not
/// interpreted here. An empty operand (`a + + b`, `+a`, `a++`) is refused too.
fn concatenation_operands(text: &str) -> Option<Vec<&str>> {
    let mut operands = Vec::new();
    let mut start = 0;
    let mut quote: Option<char> = None;
    for (index, c) in text.char_indices() {
        match quote {
            Some(open) => {
                if c == '\\' {
                    return None;
                }
                if c == open {
                    quote = None;
                }
            }
            None => match c {
                '"' | '\'' => quote = Some(c),
                '+' => {
                    operands.push(text[start..index].trim());
                    start = index + 1;
                }
                c if c.is_ascii_alphanumeric() || "._$".contains(c) || c.is_whitespace() => {}
                _ => return None,
            },
        }
    }
    if quote.is_some() {
        return None;
    }
    operands.push(text[start..].trim());
    // A single operand is not a chain, and an operand must be one token: a
    // literal, or text with no whitespace inside it.
    let well_formed = |operand: &&str| {
        !operand.is_empty()
            && (quoted_literal(operand).is_some() || !operand.contains(char::is_whitespace))
    };
    (operands.len() >= 2 && operands.iter().all(well_formed)).then_some(operands)
}

/// Re-evaluates a bound value so nested identifiers resolve too.
fn resolve_bound(value: &SymbolicValue, scope: &Scope, depth: usize) -> SymbolicValue {
    if depth >= MAX_DEPTH {
        return SymbolicValue::Unknown {
            name: value.to_string(),
        };
    }
    match value {
        SymbolicValue::Unknown { name } if scope.get(name).is_some() => {
            evaluate_expression_at(name, scope, depth)
        }
        SymbolicValue::Concat { parts } => SymbolicValue::Concat {
            parts: parts
                .iter()
                .map(|p| resolve_bound(p, scope, depth + 1))
                .collect(),
        }
        .normalized(),
        other => other.clone(),
    }
}

fn from_string_fact(fact: &StringFact) -> SymbolicValue {
    from_string_fact_with(fact, &Scope::new(), 0).normalized()
}

fn from_string_fact_with(fact: &StringFact, scope: &Scope, depth: usize) -> SymbolicValue {
    match fact {
        StringFact::Literal { value, .. } => SymbolicValue::Literal {
            value: value.clone(),
        },
        StringFact::Template { parts, .. } => SymbolicValue::Concat {
            parts: parts
                .iter()
                .map(|part| match part {
                    TemplatePart::Text { value } => SymbolicValue::Literal {
                        value: value.clone(),
                    },
                    TemplatePart::Expression { source, .. } => {
                        evaluate_expression_at(source, scope, depth)
                    }
                })
                .collect(),
        },
        StringFact::Concatenation { parts, .. } => SymbolicValue::Concat {
            parts: parts
                .iter()
                .map(|part| match part {
                    ConcatPart::Literal { value } => SymbolicValue::Literal {
                        value: value.clone(),
                    },
                    ConcatPart::Expression { source } => {
                        evaluate_expression_at(source, scope, depth)
                    }
                })
                .collect(),
        },
    }
}

/// The content of a quoted literal, if the text is one.
fn quoted_literal(text: &str) -> Option<String> {
    for quote in ['"', '\'', '`'] {
        if text.len() >= 2 && text.starts_with(quote) && text.ends_with(quote) {
            let inner = &text[1..text.len() - 1];
            // A nested quote or substitution means this is not a plain literal.
            if !inner.contains(quote) && !inner.contains("${") {
                return Some(inner.to_owned());
            }
        }
    }
    None
}

/// The variable name of an environment access, if the text is one.
///
/// The variable is **never read** — only its name is recorded (RULE 015).
fn environment_variable(text: &str) -> Option<String> {
    for prefix in ["process.env.", "import.meta.env."] {
        if let Some(name) = text.strip_prefix(prefix) {
            if is_plain_identifier(name) {
                return Some(name.to_owned());
            }
        }
    }
    // Python: `os.environ["NAME"]` / `os.environ.get("NAME")`.
    for prefix in ["os.environ[", "os.environ.get(", "environ["] {
        if let Some(rest) = text.strip_prefix(prefix) {
            let inner = rest.trim_end_matches([']', ')']).trim();
            if let Some(name) = quoted_literal(inner) {
                if is_plain_identifier(&name) {
                    return Some(name);
                }
            }
        }
    }
    None
}

fn is_plain_identifier(text: &str) -> bool {
    !text.is_empty()
        && text
            .chars()
            .next()
            .is_some_and(|c| c.is_ascii_alphabetic() || c == '_' || c == '$')
        && text
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '$')
}
