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

use std::collections::HashMap;

use cartograph_parser::model::{ConcatPart, StringFact, Symbol, SymbolKind, TemplatePart};

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
    /// A module-scope variable is bound when its initialiser is a string fact
    /// at the same source position. Anything else — a function result, an
    /// object, a computed value — is left unbound, so referencing it yields
    /// [`SymbolicValue::Unknown`] rather than a guess.
    ///
    /// Only module scope: a local variable's value depends on control flow this
    /// analysis does not model.
    #[must_use]
    pub fn from_file(symbols: &[Symbol], strings: &[StringFact]) -> Self {
        let mut scope = Self::new();
        for symbol in symbols {
            if symbol.kind != SymbolKind::Variable || symbol.enclosing.is_some() {
                continue;
            }
            // The initialiser is the string fact starting inside the
            // declaration's own span. Position is what ties them together;
            // M01/M02 do not record the link directly.
            let initialiser = strings.iter().find(|s| {
                let span = s.span();
                span.start_line >= symbol.span.start_line
                    && span.end_line <= symbol.span.end_line
                    && span.start_column >= symbol.span.start_column
            });
            if let Some(fact) = initialiser {
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
/// environment access, or a quoted literal. Anything with a call, an index or
/// an operator is [`SymbolicValue::Unsupported`], because interpreting those
/// correctly needs a real evaluator and interpreting them approximately is
/// worse than declining.
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

    SymbolicValue::Unsupported {
        source: text.to_owned(),
    }
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
