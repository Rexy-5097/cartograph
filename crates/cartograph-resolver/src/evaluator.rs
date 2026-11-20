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
        Self::collect(symbols, strings, &HashSet::new(), &[], false)
    }

    /// Collects module-scope string constants, as [`Scope::from_file`], also
    /// refusing a name the file's imports bind: `try: from cfg import P`
    /// `except ImportError: P = "/api"` makes `P` whichever ran (fifth
    /// review, W-06). A star import after the assignment may rebind it.
    #[must_use]
    pub fn from_analysis(file: &FileAnalysis) -> Self {
        let mut imported = HashSet::new();
        let mut star_lines = Vec::new();
        for import in &file.imports {
            for named in &import.named {
                imported.insert(
                    named
                        .local
                        .clone()
                        .unwrap_or_else(|| named.imported.clone()),
                );
            }
            match import.namespace_name.as_deref() {
                Some("*") => star_lines.push(import.span.start_line),
                Some(name) => {
                    imported.insert(name.to_owned());
                }
                None => {}
            }
            if let Some(name) = &import.default_name {
                imported.insert(name.clone());
            }
        }
        let python = file.language == SourceLanguage::Python;
        Self::collect(&file.symbols, &file.strings, &imported, &star_lines, python)
    }

    fn collect(
        symbols: &[Symbol],
        strings: &[StringFact],
        imported: &HashSet<String>,
        star_lines: &[u32],
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
        for symbol in symbols.iter().filter(|s| s.enclosing.is_none()) {
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
            if bindings.get(symbol.name.as_str()).copied().unwrap_or(0) > 1
                || imported.contains(&symbol.name)
                || star_lines.iter().any(|line| *line > symbol.span.start_line)
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
