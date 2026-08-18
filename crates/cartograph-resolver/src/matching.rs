//! Cross-language route matching — the first semantic join.
//!
//! ```text
//! client canonical route ─┐
//!                         ├─► candidate generation ─► compatibility ─► classification
//! backend route index ────┘
//! ```
//!
//! M03 canonicalised each observation on its own. M04 is the first stage
//! permitted to relate *two* observations, and everything here exists to make
//! that relation defensible rather than plausible.
//!
//! # The rules that keep a match honest
//!
//! - **Static segments compare exactly.** `/orders/{id}` never matches
//!   `/users/{id}`.
//! - **Direction matters.** A server's declared parameter accepts a client's
//!   concrete value (`POST /api/orders/123` reaches `POST /api/orders/{id}`).
//!   The reverse — a client's *unknown* value landing on a server's *literal*
//!   segment — is possible but unverified, so it can never be exact.
//! - **A method is never invented.** An undeclared method stays undeclared and
//!   downgrades the result; it is not read as GET.
//! - **Ambiguity is reported, not resolved.** When several routes remain
//!   plausible, the answer is [`MatchStatus::Ambiguous`] and **no edge**.
//!   Picking a winner would be the single most damaging thing this module
//!   could do, because every downstream confidence number would inherit a
//!   guess.
//! - **Unresolvable inputs are refused.** An unknown template prefix
//!   (`` `${BASE}/orders` ``) means the path's own length is unknown, so no
//!   comparison is safe. That is [`MatchStatus::Unsupported`] until M05
//!   evaluates the constant.
//!
//! # Not in this module
//!
//! No LSP, no symbol resolution, no constant propagation, no ORM, no language
//! model. Matching is deterministic: the same two observations always produce
//! the same result, and every result carries the reasons that produced it.

use std::collections::HashMap;

use cartograph_core::Confidence;

use crate::canonical::{
    CanonicalRoute, Methods, NormalizationNote, NormalizationStatus, ObservationKind, Segment,
};

/// How a client observation relates to the backend routes it was compared with.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum MatchStatus {
    /// Exactly one candidate, with an exactly-declared compatible method and a
    /// path whose every position is determined. The strongest claim M04 makes.
    Exact,
    /// Exactly one candidate, compatible but with something undetermined — an
    /// undeclared method, or a position whose value is unknown.
    Strong,
    /// The choice is not determined. **No edge is produced.** Either several
    /// candidates remain equally plausible, or the best one shares no static
    /// segment with the client path and so accepts a whole class of requests.
    /// The candidate set is returned so a human, or a later milestone with
    /// more evidence, can decide.
    Ambiguous,
    /// No backend route is compatible.
    NoMatch,
    /// The client observation could not be compared safely — an
    /// uncanonicalisable path, or one whose prefix is unknown.
    Unsupported,
}

impl MatchStatus {
    /// Whether this status permits building a graph edge.
    ///
    /// Only [`Exact`](MatchStatus::Exact) and [`Strong`](MatchStatus::Strong)
    /// do. Ambiguity is not a weak match to be recorded with low confidence —
    /// it is an unanswered question, and answering it arbitrarily would make
    /// the graph untrustworthy.
    #[must_use]
    pub fn is_accepted(self) -> bool {
        matches!(self, Self::Exact | Self::Strong)
    }
}

/// How two method declarations relate.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum MethodCompatibility {
    /// Both sides declared a method and they overlap.
    Exact,
    /// Both sides declared a method and they are disjoint. Disqualifying.
    Incompatible,
    /// The client did not declare a method; the server did.
    ClientUnknown,
    /// The server did not declare a method; the client did.
    ServerUnknown,
    /// Neither side declared a method.
    BothUnknown,
}

impl MethodCompatibility {
    fn of(client: &Methods, server: &Methods) -> Self {
        match (client.is_known(), server.is_known()) {
            (true, true) => {
                if client.compatible_with(server) {
                    Self::Exact
                } else {
                    Self::Incompatible
                }
            }
            (false, true) => Self::ClientUnknown,
            (true, false) => Self::ServerUnknown,
            (false, false) => Self::BothUnknown,
        }
    }

    /// Whether the pair can be a candidate at all.
    #[must_use]
    pub fn is_possible(self) -> bool {
        self != Self::Incompatible
    }
}

/// How two canonical paths relate, from client to server.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum PathCompatibility {
    /// Every position is static on both sides and equal. Nothing is left to
    /// assume.
    ExactStatic,
    /// Positions align and every server variable position receives a concrete
    /// client value — the `/api/orders/123` into `/api/orders/{id}` case.
    ParameterBound,
    /// Positions align, but at least one is unknown on the client side, so the
    /// path is compatible without being determined.
    VariableAligned,
    /// A trailing server wildcard consumes the client's remaining segments.
    WildcardSuffix,
    /// The paths cannot describe the same request.
    Incompatible,
}

impl PathCompatibility {
    #[must_use]
    fn is_possible(self) -> bool {
        self != Self::Incompatible
    }
}

/// A single reason contributing to a candidate's assessment.
///
/// Reasons are the raw material of the edge's evidence: together they answer
/// "why does Cartograph think these two things are connected?" without
/// requiring the reader to rerun the matcher.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum MatchReason {
    /// Client and server declared overlapping methods.
    MethodExact,
    /// One side did not declare a method, so the match rests on the path alone.
    MethodUndeclared(MethodCompatibility),
    /// Every path position is static and equal.
    StaticPathExact,
    /// Static segments agree at every position where both sides have one.
    StaticSegmentsAgree(usize),
    /// A server parameter received a concrete client value.
    ParameterBoundToLiteral(usize),
    /// A position is unknown on the client side.
    ClientValueUnknown(usize),
    /// A client unknown value was compared against a server literal — possible
    /// but unverified.
    UnknownValueAgainstLiteral(usize),
    /// A trailing server wildcard consumed client segments.
    WildcardConsumedSuffix(usize),
    /// Client and server disagree about a trailing slash.
    TrailingSlashDiffers,
    /// No static segment is shared, so nothing distinguishes this route from
    /// any other of the same shape.
    NoDiscriminatingStaticSegment,
}

/// One backend route considered for a client observation.
#[derive(Debug, Clone)]
pub struct Candidate {
    /// The backend route, canonicalised.
    pub route: CanonicalRoute,
    /// How the methods relate.
    pub method: MethodCompatibility,
    /// How the paths relate.
    pub path: PathCompatibility,
    /// Why this candidate was produced, in assessment order.
    pub reasons: Vec<MatchReason>,
    /// The uncalibrated prior for this candidate, from [`priors`].
    pub confidence: Confidence,
    /// Whether at least one static segment was matched.
    ///
    /// A route made entirely of variable positions — `/{name}`, or a bare
    /// catch-all `/{path...}` — accepts a whole class of paths, so aligning
    /// with it is not evidence about *this* client call. Such candidates are
    /// kept for inspection but never accepted; see [`MatchStatus::Ambiguous`].
    pub discriminating: bool,
}

/// The outcome of matching one client observation.
#[derive(Debug, Clone)]
pub struct MatchResult {
    /// The client observation, canonicalised.
    pub client: CanonicalRoute,
    /// The classification.
    pub status: MatchStatus,
    /// Candidates, strongest first. Retained for every status — including
    /// [`MatchStatus::Ambiguous`], where they are the whole point.
    pub candidates: Vec<Candidate>,
    /// Why the result is [`MatchStatus::Unsupported`], when it is.
    pub unsupported_reason: Option<UnsupportedReason>,
}

impl MatchResult {
    /// The single accepted candidate, if this result produced one.
    ///
    /// `None` for ambiguous, unmatched and unsupported results — an edge may
    /// only ever be built from this.
    #[must_use]
    pub fn accepted(&self) -> Option<&Candidate> {
        if self.status.is_accepted() {
            self.candidates.first()
        } else {
            None
        }
    }
}

/// Why a client observation could not be compared.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum UnsupportedReason {
    /// The client path could not be canonicalised (a dynamic expression, a
    /// regex).
    ClientPathNotCanonical,
    /// The client path's prefix is unknown, so its length is unknown and no
    /// alignment is safe. M05's constant propagation is what unlocks these.
    ClientPrefixUnresolved,
    /// The client URL names an absolute host, so there is no evidence it
    /// addresses this repository's service rather than a third party.
    ClientTargetsAbsoluteHost,
}

/// Uncalibrated starting priors for M04.
///
/// # These are priors, not measurements
///
/// They come from the specification's confidence ladder and have **not** been
/// calibrated against labelled data. An edge marked 0.98 is not yet known to be
/// correct 98% of the time; establishing that is M08, which fits these against
/// CrossStack-Bench and publishes a reliability diagram. No accuracy claim may
/// rest on them before then.
///
/// The ordering, however, is meaningful and enforced by test: more determined
/// evidence always outranks less.
pub mod priors {
    use cartograph_core::Confidence;

    /// Exact method, every path position static and equal.
    pub const EXACT_ROUTE_AND_METHOD: Confidence = Confidence::ROUTE_METHOD_EXACT;
    /// Exact method; server parameters bound to concrete client values.
    pub const PARAMETER_BOUND: Confidence = Confidence::STATIC_IMPORT;
    /// Exact method; at least one client position unknown.
    pub const VARIABLE_ALIGNED: Confidence = Confidence::CONSTANT_PROPAGATION;
    /// Exact method; matched only through a trailing wildcard.
    pub const WILDCARD_SUFFIX: Confidence = Confidence::PARTIAL_TEMPLATE_MATCH;

    /// Penalty applied when a method was not declared on one or both sides.
    ///
    /// A missing method genuinely weakens the claim: the paths may align while
    /// the request never reaches the handler. Expressed as a multiplier so the
    /// ordering above is preserved.
    pub const UNDECLARED_METHOD_FACTOR: f32 = 0.8;
}

/// A backend route set, indexed for candidate generation.
///
/// Comparing every client observation against every backend route is
/// quadratic, and real repositories have hundreds of each. Routes are bucketed
/// by segment count, which is the cheapest property that must agree for any
/// non-wildcard match; wildcard routes have variable length and are kept in a
/// separate list consulted for every query.
///
/// Method is deliberately *not* an index key: an undeclared method on either
/// side is compatible with everything, so indexing on it would drop valid
/// candidates.
#[derive(Debug, Default)]
pub struct RouteIndex {
    routes: Vec<CanonicalRoute>,
    by_arity: HashMap<usize, Vec<usize>>,
    wildcard: Vec<usize>,
}

impl RouteIndex {
    /// Builds an index over backend route declarations.
    ///
    /// Observations that are not route declarations, and routes that could not
    /// be canonicalised, are skipped: neither can support a comparison.
    #[must_use]
    pub fn build(routes: impl IntoIterator<Item = CanonicalRoute>) -> Self {
        let mut index = Self::default();
        for route in routes {
            if route.provenance.kind != ObservationKind::RouteDeclaration
                || route.status == NormalizationStatus::Unsupported
            {
                continue;
            }
            let position = index.routes.len();
            if route
                .path
                .segments
                .iter()
                .any(|s| matches!(s, Segment::Wildcard { .. }))
            {
                index.wildcard.push(position);
            } else {
                index
                    .by_arity
                    .entry(route.path.arity())
                    .or_default()
                    .push(position);
            }
            index.routes.push(route);
        }
        index
    }

    /// How many routes are indexed.
    #[must_use]
    pub fn len(&self) -> usize {
        self.routes.len()
    }

    /// Whether the index holds no routes.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.routes.is_empty()
    }

    /// Routes worth comparing against a client path of this length.
    fn candidates_for(&self, arity: usize) -> impl Iterator<Item = &CanonicalRoute> {
        let exact = self.by_arity.get(&arity).into_iter().flatten();
        // A trailing wildcard consumes one or more segments, so a wildcard
        // route can only match a strictly longer client path.
        let wildcard = self
            .wildcard
            .iter()
            .filter(move |&&i| self.routes[i].path.arity() <= arity);
        exact.chain(wildcard).map(move |&i| &self.routes[i])
    }
}

/// Matches one client observation against the indexed backend routes.
///
/// Deterministic: the same inputs always produce the same result, including
/// candidate order.
#[must_use]
pub fn match_client(client: &CanonicalRoute, index: &RouteIndex) -> MatchResult {
    if let Some(reason) = unsupported_reason(client) {
        return MatchResult {
            client: client.clone(),
            status: MatchStatus::Unsupported,
            candidates: Vec::new(),
            unsupported_reason: Some(reason),
        };
    }

    let mut candidates: Vec<Candidate> = index
        .candidates_for(client.path.arity())
        .filter_map(|route| assess(client, route))
        .collect();

    // Strongest first; ties broken on the route's own path so ordering cannot
    // depend on index insertion order.
    candidates.sort_by(|a, b| {
        b.confidence
            .get()
            .partial_cmp(&a.confidence.get())
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.route.path.to_string().cmp(&b.route.path.to_string()))
    });

    let status = classify(&candidates);
    MatchResult {
        client: client.clone(),
        status,
        candidates,
        unsupported_reason: None,
    }
}

/// Why this client observation cannot be compared at all.
fn unsupported_reason(client: &CanonicalRoute) -> Option<UnsupportedReason> {
    if client.status == NormalizationStatus::Unsupported {
        return Some(UnsupportedReason::ClientPathNotCanonical);
    }
    // An unknown leading substitution means the path's own length is unknown.
    // Aligning segments against it would be guessing how many segments the
    // prefix expands to. M05 resolves the constant; until then, refuse.
    if client
        .notes
        .contains(&NormalizationNote::TemplatePrefixUnknown)
    {
        return Some(UnsupportedReason::ClientPrefixUnresolved);
    }
    // A call to an absolute host is not evidence about this repository's
    // service. Matching it to a local route would assume a deployment topology
    // no observation supports.
    if client.path.authority.is_some() {
        return Some(UnsupportedReason::ClientTargetsAbsoluteHost);
    }
    None
}

/// Assesses one client/route pair, or rejects it.
fn assess(client: &CanonicalRoute, route: &CanonicalRoute) -> Option<Candidate> {
    let method = MethodCompatibility::of(&client.methods, &route.methods);
    if !method.is_possible() {
        return None;
    }

    let (path, mut reasons, static_matches) = compare_paths(client, route);
    if !path.is_possible() {
        return None;
    }
    // The empty path is discriminating by equality: `/` matches only `/`.
    let discriminating = static_matches > 0 || route.path.arity() == 0;
    if !discriminating {
        reasons.push(MatchReason::NoDiscriminatingStaticSegment);
    }

    reasons.insert(
        0,
        match method {
            MethodCompatibility::Exact => MatchReason::MethodExact,
            other => MatchReason::MethodUndeclared(other),
        },
    );
    if client.path.trailing_slash != route.path.trailing_slash {
        reasons.push(MatchReason::TrailingSlashDiffers);
    }

    Some(Candidate {
        route: route.clone(),
        method,
        path,
        reasons,
        confidence: confidence_for(path, method),
        discriminating,
    })
}

/// Compares a client path against a server route path, client-to-server.
///
/// Returns the compatibility, the reasons, and how many static segments were
/// matched — the last being the discriminating evidence.
fn compare_paths(
    client: &CanonicalRoute,
    route: &CanonicalRoute,
) -> (PathCompatibility, Vec<MatchReason>, usize) {
    let client_segments = &client.path.segments;
    let route_segments = &route.path.segments;
    let mut reasons = Vec::new();

    // A trailing wildcard is the only way lengths may differ.
    let wildcard_tail = matches!(route_segments.last(), Some(Segment::Wildcard { .. }));
    if !wildcard_tail && client_segments.len() != route_segments.len() {
        return (PathCompatibility::Incompatible, reasons, 0);
    }
    if wildcard_tail && client_segments.len() < route_segments.len() {
        return (PathCompatibility::Incompatible, reasons, 0);
    }
    // A wildcard anywhere but the tail is not a shape M04 compares; refusing is
    // safer than inventing a rule for it.
    if route_segments
        .iter()
        .take(route_segments.len().saturating_sub(1))
        .any(|s| matches!(s, Segment::Wildcard { .. }))
    {
        return (PathCompatibility::Incompatible, reasons, 0);
    }

    let fixed = if wildcard_tail {
        route_segments.len() - 1
    } else {
        route_segments.len()
    };

    let mut all_static = true;
    let mut bound_parameters = false;
    let mut unknown_client_value = false;
    let mut static_matches = 0_usize;

    for i in 0..fixed {
        match (&client_segments[i], &route_segments[i]) {
            // Both literal: must be identical.
            (Segment::Static { value: c }, Segment::Static { value: s }) => {
                if c != s {
                    return (PathCompatibility::Incompatible, reasons, 0);
                }
                static_matches += 1;
                reasons.push(MatchReason::StaticSegmentsAgree(i));
            }
            // The canonical case: a concrete client value into a declared
            // server parameter.
            (Segment::Static { .. }, Segment::Parameter { .. }) => {
                all_static = false;
                bound_parameters = true;
                reasons.push(MatchReason::ParameterBoundToLiteral(i));
            }
            // An unknown client value into a declared parameter: aligned, but
            // the value itself is not determined.
            (Segment::Dynamic { .. }, Segment::Parameter { .. }) => {
                all_static = false;
                unknown_client_value = true;
                reasons.push(MatchReason::ClientValueUnknown(i));
            }
            // An unknown client value against a server literal. The value could
            // be that literal, or anything else. Possible, never verified.
            (Segment::Dynamic { .. }, Segment::Static { .. }) => {
                all_static = false;
                unknown_client_value = true;
                reasons.push(MatchReason::UnknownValueAgainstLiteral(i));
            }
            // A client cannot declare a parameter or a wildcard: M03 records
            // those only for route declarations. Anything else is a shape M04
            // has no rule for, so it is refused rather than assumed.
            _ => return (PathCompatibility::Incompatible, reasons, 0),
        }
    }

    if wildcard_tail {
        reasons.push(MatchReason::WildcardConsumedSuffix(
            client_segments.len() - fixed,
        ));
        return (PathCompatibility::WildcardSuffix, reasons, static_matches);
    }
    if all_static {
        reasons.clear();
        reasons.push(MatchReason::StaticPathExact);
        return (PathCompatibility::ExactStatic, reasons, static_matches);
    }
    if unknown_client_value {
        return (PathCompatibility::VariableAligned, reasons, static_matches);
    }
    debug_assert!(bound_parameters);
    (PathCompatibility::ParameterBound, reasons, static_matches)
}

/// The prior for a candidate, from its path and method evidence.
fn confidence_for(path: PathCompatibility, method: MethodCompatibility) -> Confidence {
    let base = match path {
        PathCompatibility::ExactStatic => priors::EXACT_ROUTE_AND_METHOD,
        PathCompatibility::ParameterBound => priors::PARAMETER_BOUND,
        PathCompatibility::VariableAligned => priors::VARIABLE_ALIGNED,
        // Incompatible candidates are filtered out before this point; if one
        // ever reached here it must not outrank real evidence, so it shares
        // the weakest prior.
        PathCompatibility::WildcardSuffix | PathCompatibility::Incompatible => {
            priors::WILDCARD_SUFFIX
        }
    };
    if method == MethodCompatibility::Exact {
        return base;
    }
    // Clamped into the unit interval by construction; the factor is < 1.
    Confidence::new(base.get() * priors::UNDECLARED_METHOD_FACTOR).unwrap_or(base)
}

/// Classifies a scored candidate set.
///
/// The ambiguity rule is deliberately blunt: if the strongest candidate does
/// not stand alone at its confidence, the answer is ambiguous. Separating two
/// equally-evidenced routes needs information M04 does not have, and choosing
/// between them on tie-break order would be arbitrary.
fn classify(candidates: &[Candidate]) -> MatchStatus {
    let Some(best) = candidates.first() else {
        return MatchStatus::NoMatch;
    };
    // A route sharing no static segment accepts a whole class of paths, so
    // aligning with it says nothing about this particular call. Real
    // repositories showed why this matters: a single bare catch-all route
    // otherwise becomes a magnet for every client call in the codebase, and a
    // single-segment `/{name}` route swallows every one-segment request.
    if !best.discriminating {
        return MatchStatus::Ambiguous;
    }
    let tied = candidates
        .iter()
        .filter(|c| (c.confidence.get() - best.confidence.get()).abs() < f32::EPSILON)
        .count();
    if tied > 1 {
        return MatchStatus::Ambiguous;
    }
    if best.path == PathCompatibility::ExactStatic && best.method == MethodCompatibility::Exact {
        MatchStatus::Exact
    } else {
        MatchStatus::Strong
    }
}
