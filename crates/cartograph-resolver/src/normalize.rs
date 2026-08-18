//! The normalisation pipeline.
//!
//! ```text
//! observation → split off scheme/authority/query/fragment
//!             → tokenise path
//!             → classify each segment
//!             → canonical route (+ notes)
//! ```
//!
//! Deterministic and total: every observation produces a result, and one that
//! cannot be canonicalised safely says so via
//! [`NormalizationStatus::Unsupported`] rather than guessing.
//!
//! # The rule that shapes every decision here
//!
//! **Only syntax declares a parameter.** `/orders/123` keeps `123` as a static
//! segment; `/users/alice` keeps `alice`. A segment becomes a parameter only
//! when the source wrote `:id`, `{id}`, `<id>` or `<int:id>` — never because
//! its value looks like an identifier. Inferring otherwise would fabricate
//! structure, and every later stage would inherit the fabrication.

use cartograph_parser::model::{
    HttpCallObservation, HttpMethodHint, RouteDeclarationStyle, RouteObservation, SourceLanguage,
    Span, TemplatePart, UrlObservation,
};

use crate::canonical::{
    CanonicalPath, CanonicalRoute, Methods, NormalizationNote, NormalizationStatus,
    ObservationKind, RouteProvenance, Segment,
};

/// Canonicalises a backend route declaration.
///
/// Produces a canonical form for **this declaration alone**. It is not
/// compared with anything.
#[must_use]
pub fn normalize_route_declaration(
    observation: &RouteObservation,
    file: &str,
    language: SourceLanguage,
) -> CanonicalRoute {
    // A regex URL conf is never interpreted as structured path syntax.
    let regex_route = observation.style == RouteDeclarationStyle::UrlConfEntry
        && looks_like_regex(&observation.path);

    let provenance = RouteProvenance {
        kind: ObservationKind::RouteDeclaration,
        language,
        file: file.to_owned(),
        span: observation.span,
        raw_path: raw_text(&observation.path),
        handler: observation.handler.clone(),
    };
    let methods = Methods::from_hints(&observation.methods);

    if regex_route {
        return unsupported(
            provenance,
            methods,
            NormalizationNote::RegexNotInterpreted,
            observation.span,
        );
    }
    canonicalize(
        &observation.path,
        methods,
        provenance,
        ParameterSyntax::Declared,
    )
}

/// Canonicalises a client call site's URL.
///
/// Client URLs carry *values*, not declarations, so route-parameter syntax is
/// not recognised in them: a literal `/orders/:id` fetched by a client is a
/// literal path containing a colon, not a declared parameter. Template
/// substitutions become [`Segment::Dynamic`].
#[must_use]
pub fn normalize_client_call(
    observation: &HttpCallObservation,
    file: &str,
    language: SourceLanguage,
) -> CanonicalRoute {
    let provenance = RouteProvenance {
        kind: ObservationKind::ClientCall,
        language,
        file: file.to_owned(),
        span: observation.span,
        raw_path: raw_text(&observation.url),
        handler: None,
    };
    let methods = observation
        .method_hint
        .map_or(Methods::Unknown, |m| Methods::from_hints(&[m]));

    canonicalize(
        &observation.url,
        methods,
        provenance,
        ParameterSyntax::Literal,
    )
}

/// Whether route-parameter syntax should be recognised in a path.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ParameterSyntax {
    /// A route declaration: `:id`, `{id}`, `<int:id>` declare parameters.
    Declared,
    /// A client URL: those forms are literal text, because a client supplies
    /// values rather than declaring what it accepts.
    Literal,
}

fn canonicalize(
    url: &UrlObservation,
    methods: Methods,
    provenance: RouteProvenance,
    syntax: ParameterSyntax,
) -> CanonicalRoute {
    let span = provenance.span;
    match url {
        UrlObservation::Literal { value } => {
            let (path, mut notes) = parse_path(value, syntax);
            let status = if path.segments.iter().any(Segment::is_variable)
                && syntax == ParameterSyntax::Literal
            {
                NormalizationStatus::Partial
            } else {
                NormalizationStatus::Exact
            };
            notes.dedup();
            CanonicalRoute {
                path,
                methods,
                status,
                notes,
                provenance,
            }
        }
        UrlObservation::Template { parts } => {
            canonicalize_template(parts, methods, provenance, syntax)
        }
        UrlObservation::Dynamic { .. } => unsupported(
            provenance,
            methods,
            NormalizationNote::DynamicPathNotInterpreted,
            span,
        ),
    }
}

/// Canonicalises a template by walking its parts, keeping substitutions
/// unevaluated.
///
/// The text between substitutions is tokenised normally; each substitution
/// becomes exactly one [`Segment::Dynamic`] when it occupies a whole segment.
/// A substitution glued to adjacent text (`` /v{ver}/orders ``) makes that
/// segment's value unknown, so the whole segment becomes `Dynamic` — a partial
/// result, never an invented value.
/// Stands in for a substitution while the assembled path is tokenised, so
/// slash handling lives in exactly one place. A NUL cannot occur in real
/// source text, so it can never collide with a genuine segment.
const PLACEHOLDER: char = '\u{0}';

fn canonicalize_template(
    parts: &[TemplatePart],
    methods: Methods,
    provenance: RouteProvenance,
    syntax: ParameterSyntax,
) -> CanonicalRoute {
    let mut notes = vec![NormalizationNote::TemplateSubstitutionUnresolved];

    // A leading substitution means the path's own prefix is unknown — the
    // result describes a suffix. `${BASE}/orders` is not `/orders`.
    if matches!(parts.first(), Some(TemplatePart::Expression { .. })) {
        notes.push(NormalizationNote::TemplatePrefixUnknown);
    }

    // Build a placeholder-bearing string, then tokenise once.
    let mut assembled = String::new();
    let mut expressions: Vec<String> = Vec::new();
    for part in parts {
        match part {
            TemplatePart::Text { value } => assembled.push_str(value),
            TemplatePart::Expression { source, .. } => {
                assembled.push(PLACEHOLDER);
                expressions.push(source.clone());
            }
        }
    }

    let (mut path, parse_notes) = parse_path(&assembled, syntax);
    notes.extend(parse_notes);

    // Any segment containing a placeholder has an unknown value.
    let mut next = expressions.into_iter();
    for segment in &mut path.segments {
        if let Segment::Static { value } = segment {
            let count = value.matches(PLACEHOLDER).count();
            if count > 0 {
                let sources: Vec<String> = (0..count).filter_map(|_| next.next()).collect();
                *segment = Segment::Dynamic {
                    source: sources.join(", "),
                };
            }
        }
    }

    notes.dedup();
    CanonicalRoute {
        path,
        methods,
        status: NormalizationStatus::Partial,
        notes,
        provenance,
    }
}

/// Splits a raw path into its canonical parts.
fn parse_path(raw: &str, syntax: ParameterSyntax) -> (CanonicalPath, Vec<NormalizationNote>) {
    let mut notes = Vec::new();
    let mut rest = raw;

    // Scheme and authority, if this is an absolute URL.
    let (scheme, authority) = match split_authority(rest) {
        Some((scheme, authority, remainder)) => {
            notes.push(NormalizationNote::AuthoritySeparated);
            rest = remainder;
            (Some(scheme), Some(authority))
        }
        None => (None, None),
    };

    // Fragment, then query — in that order, since a fragment may contain `?`.
    let fragment = match rest.split_once('#') {
        Some((before, frag)) => {
            notes.push(NormalizationNote::FragmentSeparated);
            rest = before;
            Some(frag.to_owned())
        }
        None => None,
    };
    let query = match rest.split_once('?') {
        Some((before, q)) => {
            notes.push(NormalizationNote::QuerySeparated);
            rest = before;
            Some(q.to_owned())
        }
        None => None,
    };

    let trailing_slash = rest.len() > 1 && rest.ends_with('/');

    let raw_segments: Vec<&str> = rest.split('/').collect();
    let non_empty: Vec<&str> = raw_segments
        .iter()
        .copied()
        .filter(|s| !s.is_empty())
        .collect();
    // `split` on a path with a leading and/or trailing slash always yields
    // empty edge elements; only *interior* empties indicate `//`.
    let interior_empties = raw_segments
        .iter()
        .enumerate()
        .filter(|(i, s)| s.is_empty() && *i != 0 && *i != raw_segments.len() - 1)
        .count();
    if interior_empties > 0 {
        notes.push(NormalizationNote::EmptySegmentsDropped);
    }

    let segments = non_empty
        .into_iter()
        .map(|s| classify_segment(s, syntax))
        .collect();

    (
        CanonicalPath {
            segments,
            trailing_slash,
            scheme,
            authority,
            query,
            fragment,
        },
        notes,
    )
}

/// Splits `scheme://authority/rest`, if present.
fn split_authority(raw: &str) -> Option<(String, String, &str)> {
    let (scheme, after) = raw.split_once("://")?;
    if scheme.is_empty()
        || !scheme
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '+' || c == '-' || c == '.')
    {
        return None;
    }
    let (authority, rest) = match after.find('/') {
        Some(i) => (&after[..i], &after[i..]),
        None => (after, ""),
    };
    Some((scheme.to_ascii_lowercase(), authority.to_owned(), rest))
}

/// Classifies one segment.
///
/// Percent-encoding is preserved rather than decoded: `%2F` is an encoded
/// slash, and decoding it would invent a segment boundary that the source did
/// not contain.
fn classify_segment(segment: &str, syntax: ParameterSyntax) -> Segment {
    if syntax == ParameterSyntax::Literal {
        // A client URL supplies values; parameter syntax in it is just text.
        return Segment::Static {
            value: segment.to_owned(),
        };
    }

    // `*` / `**` catch-alls.
    if segment == "*" || segment == "**" {
        return Segment::Wildcard { name: None };
    }

    // `{id}` — FastAPI, and `{id:path}` for catch-alls.
    if let Some(inner) = strip_pair(segment, '{', '}') {
        let (name, constraint) = split_converter(inner, ConverterOrder::NameFirst);
        if constraint.as_deref() == Some("path") {
            return Segment::Wildcard { name: name.clone() };
        }
        return Segment::Parameter { name, constraint };
    }

    // `<id>`, `<int:id>`, `<path:rest>` — Flask and Django converters.
    if let Some(inner) = strip_pair(segment, '<', '>') {
        let (name, constraint) = split_converter(inner, ConverterOrder::ConverterFirst);
        if constraint.as_deref() == Some("path") {
            return Segment::Wildcard { name: name.clone() };
        }
        return Segment::Parameter { name, constraint };
    }

    // `:id` — Express and friends. A bare `:` is not a parameter.
    if let Some(name) = segment.strip_prefix(':') {
        if !name.is_empty() {
            return Segment::Parameter {
                name: Some(name.to_owned()),
                constraint: None,
            };
        }
    }

    Segment::Static {
        value: segment.to_owned(),
    }
}

/// Which side of `converter:name` the converter sits on.
#[derive(Clone, Copy, PartialEq, Eq)]
enum ConverterOrder {
    /// `{name:converter}` — FastAPI/Starlette.
    NameFirst,
    /// `<converter:name>` — Flask/Django.
    ConverterFirst,
}

fn split_converter(inner: &str, order: ConverterOrder) -> (Option<String>, Option<String>) {
    let named = |s: &str| (!s.is_empty()).then(|| s.to_owned());
    match inner.split_once(':') {
        Some((left, right)) => match order {
            ConverterOrder::NameFirst => (named(left), named(right)),
            ConverterOrder::ConverterFirst => (named(right), named(left)),
        },
        None => (named(inner), None),
    }
}

fn strip_pair(segment: &str, open: char, close: char) -> Option<&str> {
    segment
        .strip_prefix(open)
        .and_then(|s| s.strip_suffix(close))
}

/// Does a Django URL-conf pattern look like a regular expression?
///
/// Deliberately generous: anything carrying regex metacharacters is treated as
/// regex and left uninterpreted. Building a regex-to-route compiler is out of
/// scope, and a wrong structural reading of a pattern is worse than none.
fn looks_like_regex(url: &UrlObservation) -> bool {
    let text = match url {
        UrlObservation::Literal { value } => value.as_str(),
        _ => return false,
    };
    text.contains(['^', '$', '\\', '[', ']', '+', '?', '(', ')'])
}

fn unsupported(
    provenance: RouteProvenance,
    methods: Methods,
    note: NormalizationNote,
    _span: Span,
) -> CanonicalRoute {
    CanonicalRoute {
        path: CanonicalPath {
            segments: Vec::new(),
            trailing_slash: false,
            scheme: None,
            authority: None,
            query: None,
            fragment: None,
        },
        methods,
        status: NormalizationStatus::Unsupported,
        notes: vec![note],
        provenance,
    }
}

fn raw_text(url: &UrlObservation) -> String {
    match url {
        UrlObservation::Literal { value } => value.clone(),
        UrlObservation::Dynamic { source } => source.clone(),
        UrlObservation::Template { parts } => parts
            .iter()
            .map(|p| match p {
                TemplatePart::Text { value } => value.clone(),
                TemplatePart::Expression { source, .. } => format!("${{{source}}}"),
            })
            .collect(),
    }
}

/// Normalises a method name to a hint, or `None` when it names no known method.
///
/// Case-insensitive: `get`, `GET` and `Get` are the same method. An unknown
/// name yields `None` rather than a default (RULE 009).
#[must_use]
pub fn normalize_method(name: &str) -> Option<HttpMethodHint> {
    HttpMethodHint::from_name(name)
}
