//! The symbolic string value model.
//!
//! A restricted abstract interpreter over string expressions — **not** a
//! JavaScript runtime. Nothing here executes analysed code, reads the
//! environment, touches the network, or consults a model. Given the same
//! source it always produces the same value.
//!
//! # The invariant this module exists to enforce
//!
//! **An unresolved expression must never become a plausible-looking string.**
//!
//! The failure mode is not a crash; it is `"undefined"`, `"null"`,
//! `"[object Object]"` or an empty string flowing into a URL and matching a
//! real route. Every one of those looks like data. So unresolved values are
//! represented *structurally* — [`SymbolicValue::Unknown`],
//! [`SymbolicValue::EnvVar`], [`SymbolicValue::Unsupported`] — and there is
//! deliberately no way to flatten one into text. [`SymbolicValue::as_literal`]
//! returns `Option`, so a caller wanting a concrete string has to confront the
//! case where there isn't one.
//!
//! # Supported expression subset
//!
//! | Form | Result |
//! |---|---|
//! | `"literal"` | [`SymbolicValue::Literal`] |
//! | `` `text ${expr} text` `` | [`SymbolicValue::Concat`] of parts |
//! | `a + b` | [`SymbolicValue::Concat`] of both sides |
//! | a bound module constant | that constant's value |
//! | `process.env.NAME` / `import.meta.env.NAME` | [`SymbolicValue::EnvVar`] |
//! | an unbound identifier | [`SymbolicValue::Unknown`] |
//! | anything else | [`SymbolicValue::Unsupported`] |
//!
//! Anything outside this table is `Unsupported` — never approximated.

use std::fmt;

use serde::{Deserialize, Serialize};

/// A string value known only as far as static analysis can determine it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
#[non_exhaustive]
pub enum SymbolicValue {
    /// A fully determined string.
    Literal {
        /// The value.
        value: String,
    },
    /// A named value that could not be resolved — an unbound identifier, a
    /// function parameter, a call result.
    ///
    /// The name is kept for evidence, never as a value: `id` is what the
    /// source called it, not what it holds.
    Unknown {
        /// The identifier or expression text, for the evidence record.
        name: String,
    },
    /// An environment variable reference.
    ///
    /// **The variable is never read.** Substituting the analysing machine's
    /// environment would make results unreproducible, leak configuration into
    /// the graph, and assert a deployment fact the source does not contain.
    /// Only the *name* is recorded (RULE 015).
    EnvVar {
        /// The variable's name, e.g. `API_URL`.
        name: String,
    },
    /// An expression outside the supported subset.
    ///
    /// Distinct from [`SymbolicValue::Unknown`]: unknown means "a value this
    /// analysis cannot determine", unsupported means "a form this analysis
    /// does not interpret". The difference matters when deciding whether a
    /// gap is worth closing.
    Unsupported {
        /// The expression's source text, for the evidence record.
        source: String,
    },
    /// An ordered sequence of values, from a template literal or a `+` chain.
    Concat {
        /// The parts, in source order.
        parts: Vec<SymbolicValue>,
    },
}

impl SymbolicValue {
    /// A fully determined string, or `None` if any part is undetermined.
    ///
    /// Returning `Option` is the point: there is no way to obtain a string
    /// from an unresolved value, so a caller cannot accidentally treat
    /// `Unknown` as text.
    #[must_use]
    pub fn as_literal(&self) -> Option<String> {
        match self {
            Self::Literal { value } => Some(value.clone()),
            Self::Concat { parts } => {
                let mut out = String::new();
                for part in parts {
                    out.push_str(&part.as_literal()?);
                }
                Some(out)
            }
            Self::Unknown { .. } | Self::EnvVar { .. } | Self::Unsupported { .. } => None,
        }
    }

    /// Whether every part of this value is determined.
    #[must_use]
    pub fn is_fully_known(&self) -> bool {
        self.as_literal().is_some()
    }

    /// Flattens nested concatenations and merges adjacent literals.
    ///
    /// `Concat[Concat[Literal("/api"), Literal("/v1")], Unknown]` becomes
    /// `Concat[Literal("/api/v1"), Unknown]`, so equivalent expressions written
    /// differently produce equal values and the reconstruction step sees a flat
    /// sequence.
    #[must_use]
    pub fn normalized(self) -> Self {
        let Self::Concat { parts } = self else {
            return self;
        };

        let mut flat: Vec<Self> = Vec::new();
        for part in parts {
            match part.normalized() {
                Self::Concat { parts: inner } => flat.extend(inner),
                other => flat.push(other),
            }
        }

        let mut merged: Vec<Self> = Vec::new();
        for part in flat {
            match (merged.last_mut(), &part) {
                (Some(Self::Literal { value: prev }), Self::Literal { value: next }) => {
                    prev.push_str(next);
                }
                _ => merged.push(part),
            }
        }
        // An empty literal contributes nothing; dropping it keeps values
        // comparable without changing meaning.
        merged.retain(|p| !matches!(p, Self::Literal { value } if value.is_empty()));

        match merged.len() {
            0 => Self::Literal {
                value: String::new(),
            },
            1 => merged.pop().unwrap_or(Self::Literal {
                value: String::new(),
            }),
            _ => Self::Concat { parts: merged },
        }
    }

    /// The parts of this value as a slice-like sequence, treating a scalar as
    /// a single part.
    #[must_use]
    pub fn parts(&self) -> Vec<&Self> {
        match self {
            Self::Concat { parts } => parts.iter().collect(),
            other => vec![other],
        }
    }

    /// A short description of why this value is not determined, for evidence.
    ///
    /// `None` for values that are determined.
    #[must_use]
    pub fn unresolved_reason(&self) -> Option<String> {
        match self {
            Self::Literal { .. } => None,
            Self::Unknown { name } => Some(format!("`{name}` is not statically known")),
            Self::EnvVar { name } => Some(format!(
                "`{name}` is an environment variable, read at runtime"
            )),
            Self::Unsupported { source } => Some(format!(
                "`{source}` is outside the supported expression subset"
            )),
            Self::Concat { parts } => {
                let reasons: Vec<String> =
                    parts.iter().filter_map(Self::unresolved_reason).collect();
                if reasons.is_empty() {
                    None
                } else {
                    Some(reasons.join("; "))
                }
            }
        }
    }
}

/// Renders a value with unresolved parts kept visibly unresolved.
///
/// `{unknown}`, `{env:NAME}` and `{unsupported}` are markers, not values, and
/// they are chosen to be impossible to mistake for a path segment.
impl fmt::Display for SymbolicValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Literal { value } => f.write_str(value),
            Self::Unknown { .. } => f.write_str("{unknown}"),
            Self::EnvVar { name } => write!(f, "{{env:{name}}}"),
            Self::Unsupported { .. } => f.write_str("{unsupported}"),
            Self::Concat { parts } => {
                for part in parts {
                    write!(f, "{part}")?;
                }
                Ok(())
            }
        }
    }
}
