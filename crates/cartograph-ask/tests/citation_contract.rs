//! The citation contract, exercised through a provider that never leaves the
//! machine.
//!
//! Every test here is offline by construction: the only [`Provider`] in this
//! file returns a scripted claim, and there is no HTTP client in the crate to
//! reach for. A suite that failed on an aeroplane would make QG-004 and QG-009
//! report the weather.

use cartograph_ask::{
    Answer, ClaimedAnswer, ClaimedCitation, ClaimedItem, Provider, ProviderError, Question,
    revalidate, validate,
};
use cartograph_core::{
    Confidence, EdgeId, EdgeKind, Evidence, NodeId, NodeKind, Provenance, Secret, SourceLocation,
};
use cartograph_graph::bundle::{CitationError, EvidenceBundle};
use cartograph_graph::{ArchitectureGraph, EdgeSpec};

// ---------------------------------------------------------------------------
// Fixtures — small, deterministic, and built the way cartograph-graph's own
// bundle tests build theirs.
// ---------------------------------------------------------------------------

fn node(graph: &mut ArchitectureGraph, name: &str) -> NodeId {
    graph
        .node_for(
            NodeKind::Function,
            name,
            Some(SourceLocation::new("app.py", 1).expect("location")),
        )
        .expect("node")
}

fn edge_with(graph: &mut ArchitectureGraph, from: &str, to: &str, evidence: &str) -> EdgeId {
    let source = node(graph, from);
    let target = node(graph, to);
    graph
        .add_edge(EdgeSpec {
            source,
            target,
            kind: EdgeKind::Call,
            confidence: Confidence::new(0.9).expect("confidence"),
            provenance: Provenance::StaticImportResolution,
            evidence: Evidence::new(evidence).expect("evidence"),
            location: SourceLocation::new("app.py", 1).expect("location"),
            commit: None,
        })
        .expect("edge")
}

/// A graph, its bundle, and the edges in it — the three things every test wants.
struct Fixture {
    bundle: EvidenceBundle,
    edges: Vec<EdgeId>,
}

fn fixture(evidence: &[&str]) -> Fixture {
    let mut graph = ArchitectureGraph::new();
    let edges: Vec<EdgeId> = evidence
        .iter()
        .enumerate()
        .map(|(i, text)| {
            edge_with(
                &mut graph,
                &format!("caller{i}"),
                &format!("callee{i}"),
                text,
            )
        })
        .collect();
    let bundle = EvidenceBundle::from_edges(&graph, edges.iter().copied()).expect("bundle");
    Fixture { bundle, edges }
}

/// The evidence text every single-edge test quotes from.
const EVIDENCE: &str = "fetch('/api/users') resolves to the users route handler";

fn one_edge() -> Fixture {
    fixture(&[EVIDENCE])
}

fn cite(edge: EdgeId, text: &str) -> ClaimedCitation {
    ClaimedCitation {
        edge,
        text: text.to_owned(),
    }
}

fn item(text: &str, citations: Vec<ClaimedCitation>) -> ClaimedItem {
    ClaimedItem {
        text: text.to_owned(),
        citations,
    }
}

fn answer_of(items: Vec<ClaimedItem>) -> ClaimedAnswer {
    ClaimedAnswer { items }
}

/// A credential-shaped string, assembled rather than written down.
///
/// QG-005 scans tracked files for exactly this shape, and it is right to: a
/// credential-shaped literal in a fixture is still one in the history.
fn fake_key() -> String {
    format!("{}{}", "ghp_", "abcdefghijklmnopqrstuvwxyz0123456789")
}

// ---------------------------------------------------------------------------
// The mock provider — provider-independent, offline, and unable to cheat.
// ---------------------------------------------------------------------------

/// Returns a scripted claim and validates it like any provider must.
///
/// It cannot return an unvalidated answer, because `Answer` has no public
/// constructor. That is the property being demonstrated: a mock is held to the
/// same contract a real provider will be.
struct ScriptedProvider {
    claim: ClaimedAnswer,
}

impl Provider for ScriptedProvider {
    fn explain(
        &self,
        bundle: &EvidenceBundle,
        _question: &Question,
        _credential: &Secret,
    ) -> Result<Answer, ProviderError> {
        validate(bundle, self.claim.clone())
    }
}

/// A provider that cannot be reached — the transport-failure path, without a
/// transport.
struct UnreachableProvider;

impl Provider for UnreachableProvider {
    fn explain(
        &self,
        _bundle: &EvidenceBundle,
        _question: &Question,
        _credential: &Secret,
    ) -> Result<Answer, ProviderError> {
        Err(ProviderError::Unavailable)
    }
}

fn ask(provider: &dyn Provider, bundle: &EvidenceBundle) -> Result<Answer, ProviderError> {
    let question = Question::new("What does this reach?").expect("question");
    provider.explain(bundle, &question, &Secret::new(fake_key()))
}

// ---------------------------------------------------------------------------
// Accepted
// ---------------------------------------------------------------------------

#[test]
fn an_answer_citing_evidence_verbatim_is_accepted() {
    let f = one_edge();

    let answer = validate(
        &f.bundle,
        answer_of(vec![item(
            "The client calls the users route.",
            vec![cite(f.edges[0], "resolves to the users route handler")],
        )]),
    )
    .expect("a verbatim citation holds");

    assert_eq!(answer.len(), 1);
    assert_eq!(answer.items()[0].citations().len(), 1);
    assert_eq!(
        answer.items()[0].citations()[0].text(),
        "resolves to the users route handler"
    );
}

#[test]
fn an_answer_may_carry_several_items() {
    let f = fixture(&["alpha reaches beta", "beta reaches gamma"]);

    let answer = validate(
        &f.bundle,
        answer_of(vec![
            item("First hop.", vec![cite(f.edges[0], "alpha reaches beta")]),
            item("Second hop.", vec![cite(f.edges[1], "beta reaches gamma")]),
        ]),
    )
    .expect("both items hold");

    assert_eq!(answer.len(), 2);
}

#[test]
fn one_item_may_cite_several_edges() {
    // The case that makes "at least one citation" the right rule rather than
    // "exactly one": a claim spanning three hops honestly cites all three.
    let f = fixture(&[
        "alpha reaches beta",
        "beta reaches gamma",
        "gamma writes users",
    ]);

    let answer = validate(
        &f.bundle,
        answer_of(vec![item(
            "The request reaches the users table through two hops.",
            vec![
                cite(f.edges[0], "alpha reaches beta"),
                cite(f.edges[1], "beta reaches gamma"),
                cite(f.edges[2], "gamma writes users"),
            ],
        )]),
    )
    .expect("all three hold");

    assert_eq!(answer.items()[0].citations().len(), 3);
}

#[test]
fn a_citation_may_quote_the_whole_evidence() {
    let f = one_edge();

    let answer = validate(
        &f.bundle,
        answer_of(vec![item("All of it.", vec![cite(f.edges[0], EVIDENCE)])]),
    )
    .expect("the whole string is a substring of itself");

    assert_eq!(answer.items()[0].citations()[0].text(), EVIDENCE);
}

// ---------------------------------------------------------------------------
// Refused
// ---------------------------------------------------------------------------

#[test]
fn an_answer_with_no_items_is_refused() {
    let f = one_edge();

    assert_eq!(
        validate(&f.bundle, answer_of(vec![])),
        Err(ProviderError::NoItems)
    );
}

#[test]
fn an_item_that_cites_nothing_is_refused() {
    // RULE 007 made mechanical: an uncited sentence is interpretation wearing
    // the appearance of a derived fact.
    let f = one_edge();

    assert_eq!(
        validate(
            &f.bundle,
            answer_of(vec![item("It just does, trust me.", vec![])])
        ),
        Err(ProviderError::UncitedItem { item: 0 })
    );
}

#[test]
fn an_item_that_says_nothing_is_refused() {
    let f = one_edge();

    for empty in ["", "   ", "\n\t "] {
        assert_eq!(
            validate(
                &f.bundle,
                answer_of(vec![item(empty, vec![cite(f.edges[0], EVIDENCE)])])
            ),
            Err(ProviderError::EmptyItem { item: 0 }),
            "empty item text {empty:?} was accepted"
        );
    }
}

#[test]
fn a_paraphrase_is_refused() {
    let f = one_edge();

    let error = validate(
        &f.bundle,
        answer_of(vec![item(
            "Close enough.",
            vec![cite(f.edges[0], "resolves to the users route handlers")],
        )]),
    )
    .expect_err("a paraphrase is not a quotation");

    assert_eq!(
        error,
        ProviderError::InvalidCitation {
            item: 0,
            citation: 0,
            cause: CitationError::NotVerbatim { edge: f.edges[0] },
        }
    );
}

#[test]
fn a_citation_differing_only_in_whitespace_is_refused() {
    // The specific failure "byte for byte" exists to catch: text that would
    // survive any normalising comparison, and is still not what the evidence
    // says.
    let f = one_edge();

    let error = validate(
        &f.bundle,
        answer_of(vec![item(
            "Reflowed.",
            vec![cite(f.edges[0], "resolves to the  users route handler")],
        )]),
    )
    .expect_err("a reflowed quotation is not verbatim");

    assert!(matches!(
        error,
        ProviderError::InvalidCitation {
            cause: CitationError::NotVerbatim { .. },
            ..
        }
    ));
}

#[test]
fn a_citation_naming_an_edge_outside_the_bundle_is_refused() {
    // The graph holds two edges and the bundle carries only the first, so the
    // second is an id that exists in the graph and not in what was sent.
    let mut graph = ArchitectureGraph::new();
    let sent = edge_with(&mut graph, "a", "b", "alpha reaches beta");
    let withheld = edge_with(&mut graph, "c", "d", "gamma reaches delta");
    let bundle = EvidenceBundle::from_edges(&graph, [sent]).expect("bundle");

    let error = validate(
        &bundle,
        answer_of(vec![item(
            "About an edge that was never sent.",
            vec![cite(withheld, "gamma reaches delta")],
        )]),
    )
    .expect_err("an edge outside the bundle cannot be cited");

    assert_eq!(
        error,
        ProviderError::InvalidCitation {
            item: 0,
            citation: 0,
            cause: CitationError::UnknownEdge { edge: withheld },
        }
    );
}

#[test]
fn an_edge_id_from_another_graph_collides_and_is_still_refused() {
    // ADR-0011, demonstrated rather than described: EdgeId is graph-local and
    // restarts at zero, so an id taken from a different analysis *resolves*
    // here -- as a completely different relationship. Membership cannot catch
    // that, and does not: the verbatim check does, because the other graph's
    // evidence is not this edge's evidence.
    let f = one_edge();
    let mut other = ArchitectureGraph::new();
    let foreign = edge_with(&mut other, "x", "y", "x reaches y");

    assert_eq!(
        foreign, f.edges[0],
        "the point of this test is that the ids collide"
    );

    let error = validate(
        &f.bundle,
        answer_of(vec![item(
            "From somewhere else.",
            vec![cite(foreign, "x reaches y")],
        )]),
    )
    .expect_err("evidence from another graph is not this bundle's evidence");

    assert!(
        matches!(
            error,
            ProviderError::InvalidCitation {
                cause: CitationError::NotVerbatim { .. },
                ..
            }
        ),
        "expected the quotation check to catch it, got {error:?}"
    );
}

#[test]
fn a_citation_that_quotes_nothing_is_refused() {
    // An empty quotation is a substring of every string, so it would satisfy a
    // naive check against any evidence at all.
    let f = one_edge();

    let error = validate(
        &f.bundle,
        answer_of(vec![item("Nothing quoted.", vec![cite(f.edges[0], "")])]),
    )
    .expect_err("an empty quotation is not a citation");

    assert_eq!(
        error,
        ProviderError::InvalidCitation {
            item: 0,
            citation: 0,
            cause: CitationError::Empty,
        }
    );
}

#[test]
fn one_bad_citation_rejects_the_entire_answer() {
    // The property that matters most: a partially valid answer is never
    // returned, because it would look checked.
    let f = fixture(&["alpha reaches beta", "beta reaches gamma"]);

    let error = validate(
        &f.bundle,
        answer_of(vec![
            item("Sound.", vec![cite(f.edges[0], "alpha reaches beta")]),
            item("Sound.", vec![cite(f.edges[1], "beta reaches gamma")]),
            item("Invented.", vec![cite(f.edges[0], "alpha reaches omega")]),
        ]),
    )
    .expect_err("the third item is not supported");

    assert!(
        matches!(error, ProviderError::InvalidCitation { item: 2, .. }),
        "expected the third item to be named, got {error:?}"
    );
}

#[test]
fn the_position_of_the_failure_is_reported() {
    let f = fixture(&["alpha reaches beta"]);

    let error = validate(
        &f.bundle,
        answer_of(vec![item(
            "Two citations, the second invented.",
            vec![
                cite(f.edges[0], "alpha reaches beta"),
                cite(f.edges[0], "alpha reaches omega"),
            ],
        )]),
    )
    .expect_err("the second citation is not supported");

    assert!(matches!(
        error,
        ProviderError::InvalidCitation {
            item: 0,
            citation: 1,
            ..
        }
    ));
}

// ---------------------------------------------------------------------------
// Scope — an answer must not outlive its analysis
// ---------------------------------------------------------------------------

#[test]
fn an_answer_checked_against_another_bundle_is_refused() {
    // ADR-0011's hazard: EdgeId restarts at zero for every analysis, so a
    // stale citation almost certainly resolves -- as a different relationship.
    let first = one_edge();
    let second = fixture(&["something else entirely"]);

    let answer = validate(
        &first.bundle,
        answer_of(vec![item("Held.", vec![cite(first.edges[0], EVIDENCE)])]),
    )
    .expect("valid against its own bundle");

    assert_eq!(
        revalidate(&second.bundle, &answer),
        Err(ProviderError::ScopeMismatch)
    );
}

#[test]
fn an_answer_rechecked_against_its_own_bundle_still_holds() {
    let f = one_edge();

    let answer = validate(
        &f.bundle,
        answer_of(vec![item("Held.", vec![cite(f.edges[0], EVIDENCE)])]),
    )
    .expect("valid");

    assert_eq!(revalidate(&f.bundle, &answer), Ok(()));
}

// ---------------------------------------------------------------------------
// Through the trait, offline
// ---------------------------------------------------------------------------

#[test]
fn a_provider_returning_a_sound_claim_produces_an_answer() {
    let f = one_edge();
    let provider = ScriptedProvider {
        claim: answer_of(vec![item(
            "The client calls the users route.",
            vec![cite(f.edges[0], "the users route handler")],
        )]),
    };

    let answer = ask(&provider, &f.bundle).expect("the claim holds");

    assert_eq!(answer.len(), 1);
    assert_eq!(answer.scope(), f.bundle.scope());
}

#[test]
fn a_provider_returning_an_invented_claim_produces_nothing() {
    let f = one_edge();
    let provider = ScriptedProvider {
        claim: answer_of(vec![item(
            "It writes to the orders table.",
            vec![cite(f.edges[0], "the orders table")],
        )]),
    };

    assert!(matches!(
        ask(&provider, &f.bundle),
        Err(ProviderError::InvalidCitation { .. })
    ));
}

#[test]
fn a_provider_that_cannot_be_reached_fails_without_an_answer() {
    let f = one_edge();

    assert_eq!(
        ask(&UnreachableProvider, &f.bundle),
        Err(ProviderError::Unavailable)
    );
}

// ---------------------------------------------------------------------------
// Nothing is rewritten, and nothing leaks
// ---------------------------------------------------------------------------

#[test]
fn evidence_is_unchanged_by_validation() {
    let f = one_edge();
    let before: Vec<String> = f
        .bundle
        .edges()
        .iter()
        .map(|e| e.evidence.clone())
        .collect();

    let _ = validate(
        &f.bundle,
        answer_of(vec![item("Held.", vec![cite(f.edges[0], EVIDENCE)])]),
    )
    .expect("valid");
    let _ = validate(&f.bundle, answer_of(vec![]));

    let after: Vec<String> = f
        .bundle
        .edges()
        .iter()
        .map(|e| e.evidence.clone())
        .collect();
    assert_eq!(before, after, "validation modified the evidence");
}

#[test]
fn citation_text_survives_byte_for_byte() {
    // Punctuation and interior spacing are part of the quotation. Nothing
    // trims, normalises or re-encodes them.
    let quoted = "fetch('/api/users') resolves";
    let f = one_edge();

    let answer = validate(
        &f.bundle,
        answer_of(vec![item("Held.", vec![cite(f.edges[0], quoted)])]),
    )
    .expect("valid");

    assert_eq!(
        answer.items()[0].citations()[0].text().as_bytes(),
        quoted.as_bytes()
    );
}

#[test]
fn item_text_survives_byte_for_byte() {
    let written = "  It reaches the users table.  ";
    let f = one_edge();

    let answer = validate(
        &f.bundle,
        answer_of(vec![item(written, vec![cite(f.edges[0], EVIDENCE)])]),
    )
    .expect("valid");

    assert_eq!(answer.items()[0].text(), written);
}

#[test]
fn no_error_carries_a_credential_or_a_machine_path() {
    let f = one_edge();
    let key = fake_key();

    let errors = [
        ProviderError::NoItems,
        ProviderError::EmptyItem { item: 3 },
        ProviderError::UncitedItem { item: 3 },
        ProviderError::ScopeMismatch,
        ProviderError::Unavailable,
        ProviderError::InvalidCitation {
            item: 0,
            citation: 0,
            cause: CitationError::NotVerbatim { edge: f.edges[0] },
        },
    ];

    for error in errors {
        let rendered = format!("{error} {error:?}");
        assert!(
            !rendered.contains(&key),
            "error carries a credential: {rendered}"
        );
        assert!(
            !rendered.contains("C:\\"),
            "error carries a path: {rendered}"
        );
        assert!(
            !rendered.contains("/home/"),
            "error carries a path: {rendered}"
        );
        assert!(
            !rendered.contains(EVIDENCE),
            "error carries evidence text: {rendered}"
        );
    }
}

#[test]
fn the_bundle_a_provider_receives_carries_no_absolute_path() {
    // SourceLocation refuses absolute paths at construction, so this holds by
    // construction rather than by inspection -- asserted anyway, because the
    // guarantee is what lets a bundle leave the machine at all.
    let f = one_edge();

    for edge in f.bundle.edges() {
        let file = edge.location.file();
        assert!(!file.starts_with('/'), "absolute path in a bundle: {file}");
        assert!(!file.starts_with('\\'), "absolute path in a bundle: {file}");
        assert!(
            !file.contains(':'),
            "drive-absolute path in a bundle: {file}"
        );
        assert!(!file.contains(".."), "escaping path in a bundle: {file}");
    }
}

#[test]
fn a_credential_never_becomes_text_in_this_crate() {
    let secret = Secret::new(fake_key());

    let debugged = format!("{secret:?}");

    assert_eq!(debugged, "Secret(<redacted>)");
    assert!(!debugged.contains("ghp_"));
}
