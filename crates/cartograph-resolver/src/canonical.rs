//! The canonical route model.
//!
//! One representation that both sides of the eventual cross-stack join can be
//! expressed in: a client call site's URL and a backend route declaration.
//!
//! # What a canonical route is not
//!
//! It is **not** a claim that two observations describe the same endpoint.
//! Canonicalisation is a per-observation transformation — it looks at one
//! observation and rewrites its path into a comparable shape. Deciding that a
//! client call reaches a particular route requires comparing two of these,
//! weighing prefixes and mounts, and attaching confidence and evidence. That
//! is M04, and nothing in this module does any of it.
//!
//! # The distinction that governs the design
//!
//! `/orders/123` and `/orders/{id}` are *not* the same thing, and M03 must
//! never conflate them:
//!
//! - `/orders/123` is a **literal** path. `123` stays [`Segment::Static`].
//!   Nothing about a numeric-looking segment says it is an identifier.
//! - `/orders/{id}` **declares** a parameter, so `id` becomes
//!   [`Segment::Parameter`] — because the *syntax* said so.
//! - `` /orders/${orderId} `` **substitutes an unknown value**, so it becomes
//!   [`Segment::Dynamic`] — a value this analysis does not know, which is a
//!   different statement from "this position accepts anything".
//!
//! Keeping `Parameter` and `Dynamic` apart matters: a server declaring a
//! parameter and a client interpolating a variable are different facts, and
//! flattening them here would destroy evidence M04 needs.

use std::fmt;

use cartograph_parser::model::{HttpMethodHint, SourceLanguage, Span};
use serde::{Deserialize, Serialize};

/// One position in a canonical path.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
#[non_exhaustive]
pub enum Segment {
    /// A literal segment, exactly as written.
    ///
    /// `123` in `/orders/123` is static. It is *not* promoted to a parameter:
    /// only route syntax declares parameters, never the shape of a value.
    Static {
        /// The segment text, percent-encoding preserved.
        value: String,
    },
    /// A parameter **declared** by route syntax: `:id`, `{id}`, `<int:id>`.
    Parameter {
        /// The declared name, when the syntax carries one.
        #[serde(skip_serializing_if = "Option::is_none")]
        name: Option<String>,
        /// A declared type constraint — Flask/Django converters such as
        /// `int`, `uuid`, `slug`. Preserved, never interpreted.
        #[serde(skip_serializing_if = "Option::is_none")]
        constraint: Option<String>,
    },
    /// A value substituted at runtime whose content is unknown here:
    /// `` ${orderId} `` in a template, or an f-string substitution.
    ///
    /// Distinct from [`Segment::Parameter`]: the client is supplying *a*
    /// value, not declaring that *any* value is accepted.
    Dynamic {
        /// The substituted expression's source text, preserved for M05.
        source: String,
    },
    /// A catch-all matching one or more remaining segments: `*`, `**`,
    /// `<path:rest>`.
    Wildcard {
        /// The declared name, when the syntax carries one.
        #[serde(skip_serializing_if = "Option::is_none")]
        name: Option<String>,
    },
}

impl Segment {
    /// Whether this position holds a fixed, known value.
    #[must_use]
    pub fn is_static(&self) -> bool {
        matches!(self, Self::Static { .. })
    }

    /// Whether this position varies — declared parameter, substituted value or
    /// catch-all.
    #[must_use]
    pub fn is_variable(&self) -> bool {
        !self.is_static()
    }
}

impl fmt::Display for Segment {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Static { value } => f.write_str(value),
            Self::Parameter { name: Some(n), .. } => write!(f, "{{{n}}}"),
            // An unnamed parameter and a substituted value both render as a
            // nameless variable position. They stay *distinct variants* — the
            // difference matters to M04 — but there is nothing to print.
            Self::Parameter { name: None, .. } | Self::Dynamic { .. } => f.write_str("{*}"),
            Self::Wildcard { name: Some(n) } => write!(f, "{{{n}...}}"),
            Self::Wildcard { name: None } => f.write_str("{**}"),
        }
    }
}

/// The HTTP methods a route observation constrains itself to.
///
/// `Unknown` is a first-class state, not an absence to be filled in. Flask's
/// `@app.route("/x")` accepts GET at runtime, but the *source* declared no
/// method; turning that into GET is framework inference and belongs to a later
/// milestone with evidence attached (RULE 009).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Methods {
    /// The source stated no method.
    Unknown,
    /// The source stated these methods, deduplicated and ordered.
    Declared(Vec<HttpMethodHint>),
}

impl Methods {
    /// Builds from observed hints, deduplicating and ordering deterministically.
    ///
    /// An empty input is [`Methods::Unknown`] — never an empty `Declared`,
    /// which would read as "declared nothing" rather than "declared nothing
    /// known".
    #[must_use]
    pub fn from_hints(hints: &[HttpMethodHint]) -> Self {
        if hints.is_empty() {
            return Self::Unknown;
        }
        let mut sorted: Vec<HttpMethodHint> = Vec::new();
        for hint in hints {
            if !sorted.contains(hint) {
                sorted.push(*hint);
            }
        }
        sorted.sort_by_key(|m| format!("{m:?}"));
        Self::Declared(sorted)
    }

    /// Whether the source stated any method at all.
    #[must_use]
    pub fn is_known(&self) -> bool {
        matches!(self, Self::Declared(_))
    }

    /// Whether two method sets could describe the same traffic.
    ///
    /// Purely structural: `Unknown` is compatible with anything, because an
    /// undeclared method excludes nothing. **This is not a match.** It answers
    /// "is there any reason these could not overlap?", which M04 combines with
    /// path comparison, mount prefixes and evidence before claiming anything.
    #[must_use]
    pub fn compatible_with(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Unknown, _) | (_, Self::Unknown) => true,
            (Self::Declared(a), Self::Declared(b)) => a.iter().any(|m| b.contains(m)),
        }
    }
}

impl fmt::Display for Methods {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unknown => f.write_str("UNKNOWN"),
            Self::Declared(methods) => {
                let names: Vec<String> = methods
                    .iter()
                    .map(|m| format!("{m:?}").to_uppercase())
                    .collect();
                f.write_str(&names.join("|"))
            }
        }
    }
}

/// How completely an observation could be canonicalised.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum NormalizationStatus {
    /// Every segment was determined from the source.
    Exact,
    /// The shape is known but at least one position holds an unknown value —
    /// a template substitution, for instance. Usable, with the gaps explicit.
    Partial,
    /// The observation could not be canonicalised safely. The raw form is
    /// retained; no path structure is asserted.
    ///
    /// Preferring this over a plausible guess is the point: "cannot safely
    /// normalise" is a usable fact, "probably this route" is not.
    Unsupported,
}

/// Something worth recording about a normalisation.
///
/// Notes describe **the transformation**, not how much to trust a
/// cross-stack relationship. There is no confidence here; confidence attaches
/// to edges, and M03 produces none.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
#[non_exhaustive]
pub enum NormalizationNote {
    /// A query string was separated from the path and kept as metadata.
    QuerySeparated,
    /// A fragment was separated from the path.
    FragmentSeparated,
    /// An absolute URL's scheme and authority were separated from the path.
    AuthoritySeparated,
    /// Consecutive slashes produced empty segments, which were dropped.
    EmptySegmentsDropped,
    /// A template substitution became a [`Segment::Dynamic`] rather than being
    /// evaluated. Evaluation is M05.
    TemplateSubstitutionUnresolved,
    /// A template's leading substitution means the path's own prefix is
    /// unknown, so the result is a suffix rather than a complete path.
    TemplatePrefixUnknown,
    /// A regular-expression route was not interpreted.
    RegexNotInterpreted,
    /// The observation's path was not a literal or template at all.
    DynamicPathNotInterpreted,
}

/// Where a canonical route came from.
///
/// Retained so M04 can explain a match rather than assert one, and so a user
/// clicking an eventual edge sees the original source position.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RouteProvenance {
    /// Whether the observation was a client call or a route declaration.
    pub kind: ObservationKind,
    /// The language the observation was extracted from.
    pub language: SourceLanguage,
    /// Repository-relative file path.
    pub file: String,
    /// Where in the file.
    pub span: Span,
    /// The path exactly as it appeared in source, before normalisation.
    pub raw_path: String,
    /// The handler symbol, for route declarations that name one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub handler: Option<String>,
    /// For a client call, the function the call site sits inside.
    ///
    /// Keeps a generated client's wrapper function on the path: the graph can
    /// then say `postPool` issues the request, rather than attributing it to
    /// the whole file.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enclosing: Option<String>,
}

/// Which side of the eventual join an observation came from.
///
/// Language-neutral by design: a TypeScript `fetch` and a Python `requests`
/// call are both [`ObservationKind::ClientCall`], and M04 must not need
/// per-language branching.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
#[non_exhaustive]
pub enum ObservationKind {
    /// A call site that issues an HTTP request.
    ClientCall,
    /// A declaration that registers a route.
    RouteDeclaration,
}

/// A path in canonical form, with everything that was separated from it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CanonicalPath {
    /// The path's segments, in order. Empty means the root path.
    pub segments: Vec<Segment>,
    /// Whether the source path ended in `/`.
    ///
    /// Preserved rather than collapsed: `/orders` and `/orders/` are distinct
    /// observations, and frameworks disagree about whether they are the same
    /// route. M04 decides; M03 must not destroy the evidence.
    pub trailing_slash: bool,
    /// The scheme of an absolute URL, if one was present.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scheme: Option<String>,
    /// The authority (host and optional port) of an absolute URL.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub authority: Option<String>,
    /// The query string, without the `?`.
    ///
    /// Kept as metadata and never turned into segments. Query parameter names
    /// are not path parameters, and using them as such would invent structure.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub query: Option<String>,
    /// The fragment, without the `#`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fragment: Option<String>,
}

impl CanonicalPath {
    /// How many positions the path has.
    #[must_use]
    pub fn arity(&self) -> usize {
        self.segments.len()
    }

    /// The path with variable positions erased to `{*}`.
    ///
    /// This is the form the specification writes as `/orders/{*}`. Two
    /// observations sharing a shape are *structurally comparable* — they are
    /// not thereby the same endpoint.
    #[must_use]
    pub fn shape(&self) -> String {
        let mut out = String::new();
        for segment in &self.segments {
            out.push('/');
            match segment {
                Segment::Static { value } => out.push_str(value),
                Segment::Wildcard { .. } => out.push_str("{**}"),
                Segment::Parameter { .. } | Segment::Dynamic { .. } => out.push_str("{*}"),
            }
        }
        // The root path renders as "/", and a trailing slash is preserved;
        // both cases append the same character.
        if out.is_empty() || self.trailing_slash {
            out.push('/');
        }
        out
    }
}

impl fmt::Display for CanonicalPath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let (Some(scheme), Some(authority)) = (&self.scheme, &self.authority) {
            write!(f, "{scheme}://{authority}")?;
        }
        if self.segments.is_empty() {
            f.write_str("/")?;
        } else {
            for segment in &self.segments {
                write!(f, "/{segment}")?;
            }
            if self.trailing_slash {
                f.write_str("/")?;
            }
        }
        Ok(())
    }
}

/// One observation, canonicalised.
///
/// # Boundary
///
/// A `CanonicalRoute` describes **one** observation. It never references
/// another observation, carries no confidence, and asserts no relationship.
/// The comparison helpers it offers are structural predicates that answer
/// "could these be comparable?" — never "do these match?".
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CanonicalRoute {
    /// The canonical path.
    pub path: CanonicalPath,
    /// The declared methods, or `Unknown`.
    pub methods: Methods,
    /// How completely the observation could be canonicalised.
    pub status: NormalizationStatus,
    /// What happened during normalisation.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub notes: Vec<NormalizationNote>,
    /// Where this came from, and what it looked like before.
    pub provenance: RouteProvenance,
}

impl CanonicalRoute {
    /// Whether two canonical paths have the same structure: the same number of
    /// positions, with static positions holding equal text and variable
    /// positions aligned.
    ///
    /// # This is not a match
    ///
    /// It ignores methods, mount prefixes, base URLs, trailing slashes and
    /// every other consideration that decides whether a client call actually
    /// reaches a route. Two routes on unrelated services can share a shape.
    /// M04 combines this with method compatibility, prefix analysis and
    /// evidence before making any claim — and only M04 may make one.
    ///
    /// Returns `false` for anything that could not be canonicalised
    /// ([`NormalizationStatus::Unsupported`]), because there is no structure
    /// to compare.
    #[must_use]
    pub fn same_shape(&self, other: &Self) -> bool {
        if self.status == NormalizationStatus::Unsupported
            || other.status == NormalizationStatus::Unsupported
        {
            return false;
        }
        if self.path.arity() != other.path.arity() {
            return false;
        }
        self.path
            .segments
            .iter()
            .zip(&other.path.segments)
            .all(|(a, b)| match (a, b) {
                (Segment::Static { value: x }, Segment::Static { value: y }) => x == y,
                (Segment::Static { .. }, _) | (_, Segment::Static { .. }) => false,
                // Parameter, Dynamic and Wildcard all occupy a variable
                // position. Whether a client's substituted value may fill a
                // server's declared parameter is a matching judgement (M04).
                _ => true,
            })
    }

    /// Whether the two observations' methods could overlap. See
    /// [`Methods::compatible_with`] — structural only, never a match.
    #[must_use]
    pub fn compatible_method(&self, other: &Self) -> bool {
        self.methods.compatible_with(&other.methods)
    }
}

impl fmt::Display for CanonicalRoute {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} {}", self.methods, self.path)
    }
}
