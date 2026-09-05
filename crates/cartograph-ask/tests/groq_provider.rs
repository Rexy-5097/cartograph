//! `GroqProvider`, exercised end to end without a network.
//!
//! Every test drives the real provider through [`MockTransport`]. Nothing here
//! opens a socket, needs a key, or reads a keychain — the credential is a
//! string assembled in the test, and the only thing that ever sees it is the
//! header the provider builds and drops.

use cartograph_ask::groq::{
    ENDPOINT, MAX_COMPLETION_TOKENS, MODEL, SCHEMA_NAME, SYSTEM_INSTRUCTION,
};
use cartograph_ask::testing::{MockTransport, UnreachableTransport};
use cartograph_ask::{
    GroqProvider, Provider, ProviderError, Question, ResponseFault, TransportError,
};
use cartograph_core::{
    Confidence, EdgeId, EdgeKind, Evidence, NodeId, NodeKind, Provenance, Secret, SourceLocation,
};
use cartograph_graph::bundle::{CitationError, EvidenceBundle};
use cartograph_graph::{ArchitectureGraph, EdgeSpec};

// ---------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------

const EVIDENCE: &str = "fetch('/api/users') resolves to the users route handler";

fn node(graph: &mut ArchitectureGraph, name: &str) -> NodeId {
    graph
        .node_for(
            NodeKind::Function,
            name,
            Some(SourceLocation::new("src/api/client.ts", 1).expect("location")),
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
            kind: EdgeKind::HttpCall,
            confidence: Confidence::new(0.9).expect("confidence"),
            provenance: Provenance::StaticImportResolution,
            evidence: Evidence::new(evidence).expect("evidence"),
            location: SourceLocation::new("src/api/client.ts", 42).expect("location"),
            commit: None,
        })
        .expect("edge")
}

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

fn one_edge() -> Fixture {
    fixture(&[EVIDENCE])
}

fn question() -> Question {
    Question::new("What does the client reach?").expect("question")
}

/// A credential-shaped string, assembled at runtime.
///
/// QG-005 scans tracked files for exactly this shape and is right to: a
/// credential-shaped literal in a fixture is still one in the history.
fn fake_key() -> String {
    format!("{}{}", "gsk_", "abcdefghijklmnopqrstuvwxyz0123456789")
}

/// A machine-shaped path, assembled at runtime.
///
/// The same treatment [`fake_key`] gets, and for the same reason: QG-005 scans
/// tracked files for exactly this shape and is right to. The test needs the real
/// shape to prove an error does not repeat it, and no half of it is a machine
/// path on its own.
fn machine_paths() -> (String, String) {
    let windows = format!("{}{}", "C:\\\\Users\\\\", "someone\\\\repo");
    let unix = format!("{}{}", "/home/", "someone/repo");
    (windows, unix)
}

/// A reply body, built through serde so an `EdgeId` is written the way the wire
/// writes it rather than the way `Display` writes it.
fn reply(items: &serde_json::Value) -> Vec<u8> {
    let content = serde_json::to_string(&serde_json::json!({ "items": items })).expect("fixture");
    envelope(&content)
}

fn envelope(content: &str) -> Vec<u8> {
    serde_json::to_vec(&serde_json::json!({
        "id": "chatcmpl-not-a-real-id",
        "object": "chat.completion",
        "model": MODEL,
        "choices": [{ "index": 0, "message": { "role": "assistant", "content": content } }],
        "usage": { "total_tokens": 15 }
    }))
    .expect("fixture")
}

fn item(text: &str, citations: &serde_json::Value) -> serde_json::Value {
    serde_json::json!({ "text": text, "citations": citations })
}

fn citation(edge: EdgeId, text: &str) -> serde_json::Value {
    serde_json::json!({ "edge": edge, "text": text })
}

/// A reply that holds up: one item, citing the fixture's evidence verbatim.
fn sound_reply(edge: EdgeId) -> Vec<u8> {
    reply(&serde_json::json!([item(
        "The client calls the users route.",
        &serde_json::json!([citation(edge, "the users route handler")])
    )]))
}

// ---------------------------------------------------------------------------
// The request the provider builds
// ---------------------------------------------------------------------------

#[test]
fn the_request_goes_to_the_recorded_endpoint() {
    let f = one_edge();
    let transport = MockTransport::replying(200, sound_reply(f.edges[0]));
    let provider = GroqProvider::new(transport);

    provider
        .explain(&f.bundle, &question(), &Secret::new(fake_key()))
        .expect("a sound answer");

    let sent = provider.transport().only_call();
    assert_eq!(sent.url(), ENDPOINT);
    assert_eq!(ENDPOINT, "https://api.groq.com/openai/v1/chat/completions");
}

#[test]
fn the_request_carries_the_headers_a_groq_call_needs() {
    let f = one_edge();
    let provider = GroqProvider::new(MockTransport::replying(200, sound_reply(f.edges[0])));

    provider
        .explain(&f.bundle, &question(), &Secret::new(fake_key()))
        .expect("a sound answer");

    let sent = provider.transport().only_call();
    assert!(
        sent.has_header("Authorization"),
        "no credential was attached"
    );
    assert!(
        sent.has_header("Content-Type"),
        "no content type was declared"
    );
    assert_eq!(
        sent.header_names().len(),
        2,
        "an unexpected header was added"
    );
}

#[test]
fn the_request_body_is_the_one_the_wire_module_builds() {
    let f = one_edge();
    let expected = cartograph_ask::groq::request_body(&f.bundle, &question()).expect("serialised");
    let provider = GroqProvider::new(MockTransport::replying(200, sound_reply(f.edges[0])));

    provider
        .explain(&f.bundle, &question(), &Secret::new(fake_key()))
        .expect("a sound answer");

    assert_eq!(
        provider.transport().only_call().body(),
        expected.as_slice(),
        "the provider did not send the bytes the privacy tests inspect"
    );
}

#[test]
fn the_provider_owns_the_model_the_schema_and_the_streaming_flag() {
    let f = one_edge();
    let provider = GroqProvider::new(MockTransport::replying(200, sound_reply(f.edges[0])));

    provider
        .explain(&f.bundle, &question(), &Secret::new(fake_key()))
        .expect("a sound answer");

    let sent = provider.transport().only_call();
    let body: serde_json::Value = serde_json::from_slice(sent.body()).expect("valid JSON");

    assert_eq!(body["model"], MODEL);
    assert_eq!(body["stream"], false);
    assert_eq!(body["max_completion_tokens"], MAX_COMPLETION_TOKENS);
    assert_eq!(body["response_format"]["type"], "json_schema");
    assert_eq!(body["response_format"]["json_schema"]["strict"], true);
    assert_eq!(body["response_format"]["json_schema"]["name"], SCHEMA_NAME);
    assert_eq!(body["messages"][0]["content"], SYSTEM_INSTRUCTION);
}

// ---------------------------------------------------------------------------
// The credential
// ---------------------------------------------------------------------------

#[test]
fn the_credential_is_attached_but_never_recoverable() {
    // The header is proved present without anything holding what it said.
    let f = one_edge();
    let key = fake_key();
    let provider = GroqProvider::new(MockTransport::replying(200, sound_reply(f.edges[0])));

    provider
        .explain(&f.bundle, &question(), &Secret::new(key.clone()))
        .expect("a sound answer");

    let sent = provider.transport().only_call();
    assert!(sent.has_header("authorization"));

    let rendered = format!("{sent:?} {:?} {:?}", sent.header_names(), provider);
    assert!(
        !rendered.contains(&key),
        "the credential is recoverable: {rendered}"
    );
    assert!(
        !rendered.contains("Bearer"),
        "the credential is recoverable"
    );
}

#[test]
fn the_credential_never_enters_the_request_body() {
    let f = one_edge();
    let key = fake_key();
    let provider = GroqProvider::new(MockTransport::replying(200, sound_reply(f.edges[0])));

    provider
        .explain(&f.bundle, &question(), &Secret::new(key.clone()))
        .expect("a sound answer");

    let body = String::from_utf8(provider.transport().only_call().body().to_vec()).expect("UTF-8");
    assert!(!body.contains(&key), "the credential reached the body");
    assert!(!body.contains("Bearer"));
    assert!(!body.contains("Authorization"));
}

#[test]
fn the_provider_holds_no_credential_between_calls() {
    // There is no field one could be stored in, and `Debug` proves the struct
    // carries only a transport.
    let f = one_edge();
    let key = fake_key();
    let provider = GroqProvider::new(MockTransport::replying(200, sound_reply(f.edges[0])));

    provider
        .explain(&f.bundle, &question(), &Secret::new(key.clone()))
        .expect("first");
    provider
        .explain(&f.bundle, &question(), &Secret::new(key.clone()))
        .expect("second");

    assert_eq!(provider.transport().call_count(), 2);
    assert!(!format!("{provider:?}").contains(&key));
}

// ---------------------------------------------------------------------------
// The happy path
// ---------------------------------------------------------------------------

#[test]
fn a_sound_reply_becomes_a_validated_answer() {
    let f = one_edge();
    let provider = GroqProvider::new(MockTransport::replying(200, sound_reply(f.edges[0])));

    let answer = provider
        .explain(&f.bundle, &question(), &Secret::new(fake_key()))
        .expect("a sound answer");

    assert_eq!(answer.len(), 1);
    assert_eq!(answer.scope(), f.bundle.scope());
    assert_eq!(
        answer.items()[0].text(),
        "The client calls the users route."
    );
    assert_eq!(
        answer.items()[0].citations()[0].text(),
        "the users route handler"
    );
}

#[test]
fn several_items_with_several_citations_are_accepted() {
    let f = fixture(&["alpha reaches beta", "beta reaches gamma"]);
    let body = reply(&serde_json::json!([
        item(
            "Two hops.",
            &serde_json::json!([
                citation(f.edges[0], "alpha reaches beta"),
                citation(f.edges[1], "beta reaches gamma")
            ])
        ),
        item(
            "And the second alone.",
            &serde_json::json!([citation(f.edges[1], "beta reaches")])
        )
    ]));
    let provider = GroqProvider::new(MockTransport::replying(200, body));

    let answer = provider
        .explain(&f.bundle, &question(), &Secret::new(fake_key()))
        .expect("both items hold");

    assert_eq!(answer.len(), 2);
    assert_eq!(answer.items()[0].citations().len(), 2);
}

#[test]
fn unicode_and_long_text_survive_the_round_trip() {
    let f = fixture(&["appelle « utilisateurs » — 日本語 🛰"]);
    let long = "x".repeat(8192);
    let body = reply(&serde_json::json!([item(
        &long,
        &serde_json::json!([citation(f.edges[0], "« utilisateurs » — 日本語 🛰")])
    )]));
    let provider = GroqProvider::new(MockTransport::replying(200, body));

    let answer = provider
        .explain(&f.bundle, &question(), &Secret::new(fake_key()))
        .expect("valid");

    assert_eq!(answer.items()[0].text().len(), long.len());
    assert_eq!(
        answer.items()[0].citations()[0].text(),
        "« utilisateurs » — 日本語 🛰"
    );
}

// ---------------------------------------------------------------------------
// Answers that do not hold
// ---------------------------------------------------------------------------

#[test]
fn an_invented_quotation_is_refused() {
    let f = one_edge();
    let body = reply(&serde_json::json!([item(
        "It writes to the orders table.",
        &serde_json::json!([citation(f.edges[0], "the orders table")])
    )]));
    let provider = GroqProvider::new(MockTransport::replying(200, body));

    let error = provider
        .explain(&f.bundle, &question(), &Secret::new(fake_key()))
        .expect_err("nothing in the evidence says that");

    assert!(matches!(
        error,
        ProviderError::InvalidCitation {
            cause: CitationError::NotVerbatim { .. },
            ..
        }
    ));
}

#[test]
fn a_reworded_quotation_is_refused() {
    let f = one_edge();
    let body = reply(&serde_json::json!([item(
        "Close enough.",
        &serde_json::json!([citation(f.edges[0], "the  users route handler")])
    )]));
    let provider = GroqProvider::new(MockTransport::replying(200, body));

    assert!(matches!(
        provider.explain(&f.bundle, &question(), &Secret::new(fake_key())),
        Err(ProviderError::InvalidCitation { .. })
    ));
}

#[test]
fn a_citation_naming_an_edge_that_was_not_sent_is_refused() {
    let mut graph = ArchitectureGraph::new();
    let sent = edge_with(&mut graph, "a", "b", "alpha reaches beta");
    let withheld = edge_with(&mut graph, "c", "d", "gamma reaches delta");
    let bundle = EvidenceBundle::from_edges(&graph, [sent]).expect("bundle");
    let body = reply(&serde_json::json!([item(
        "About an edge never sent.",
        &serde_json::json!([citation(withheld, "gamma reaches delta")])
    )]));
    let provider = GroqProvider::new(MockTransport::replying(200, body));

    let error = provider
        .explain(&bundle, &question(), &Secret::new(fake_key()))
        .expect_err("that edge was not in the bundle");

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
fn an_uncited_item_is_refused() {
    let f = one_edge();
    let body = reply(&serde_json::json!([item(
        "Trust me.",
        &serde_json::json!([])
    )]));
    let provider = GroqProvider::new(MockTransport::replying(200, body));

    assert_eq!(
        provider.explain(&f.bundle, &question(), &Secret::new(fake_key())),
        Err(ProviderError::UncitedItem { item: 0 })
    );
}

#[test]
fn an_empty_citation_is_refused() {
    let f = one_edge();
    let body = reply(&serde_json::json!([item(
        "Nothing quoted.",
        &serde_json::json!([citation(f.edges[0], "")])
    )]));
    let provider = GroqProvider::new(MockTransport::replying(200, body));

    assert_eq!(
        provider.explain(&f.bundle, &question(), &Secret::new(fake_key())),
        Err(ProviderError::InvalidCitation {
            item: 0,
            citation: 0,
            cause: CitationError::Empty,
        })
    );
}

#[test]
fn an_empty_answer_is_refused() {
    let f = one_edge();
    let provider = GroqProvider::new(MockTransport::replying(200, reply(&serde_json::json!([]))));

    assert_eq!(
        provider.explain(&f.bundle, &question(), &Secret::new(fake_key())),
        Err(ProviderError::NoItems)
    );
}

#[test]
fn one_bad_item_rejects_the_whole_answer() {
    // Nothing partial is ever returned, even when most of the answer holds.
    let f = fixture(&["alpha reaches beta", "beta reaches gamma"]);
    let body = reply(&serde_json::json!([
        item(
            "Sound.",
            &serde_json::json!([citation(f.edges[0], "alpha reaches beta")])
        ),
        item(
            "Sound.",
            &serde_json::json!([citation(f.edges[1], "beta reaches gamma")])
        ),
        item(
            "Invented.",
            &serde_json::json!([citation(f.edges[0], "alpha reaches omega")])
        )
    ]));
    let provider = GroqProvider::new(MockTransport::replying(200, body));

    assert!(matches!(
        provider.explain(&f.bundle, &question(), &Secret::new(fake_key())),
        Err(ProviderError::InvalidCitation { item: 2, .. })
    ));
}

// ---------------------------------------------------------------------------
// Replies that are not answers
// ---------------------------------------------------------------------------

#[test]
fn a_body_that_is_not_json_is_refused_safely() {
    let f = one_edge();
    let provider = GroqProvider::new(MockTransport::replying(200, b"<html>hello</html>".to_vec()));

    assert_eq!(
        provider.explain(&f.bundle, &question(), &Secret::new(fake_key())),
        Err(ProviderError::InvalidResponse {
            fault: ResponseFault::NotJson
        })
    );
}

#[test]
fn an_envelope_with_no_choice_is_refused_safely() {
    let f = one_edge();
    let body = serde_json::to_vec(&serde_json::json!({ "choices": [] })).expect("fixture");
    let provider = GroqProvider::new(MockTransport::replying(200, body));

    assert_eq!(
        provider.explain(&f.bundle, &question(), &Secret::new(fake_key())),
        Err(ProviderError::InvalidResponse {
            fault: ResponseFault::NoChoices
        })
    );
}

#[test]
fn structured_output_that_is_not_the_schema_is_refused_safely() {
    let f = one_edge();
    let provider = GroqProvider::new(MockTransport::replying(
        200,
        envelope(r#"{"explanations":[]}"#),
    ));

    assert_eq!(
        provider.explain(&f.bundle, &question(), &Secret::new(fake_key())),
        Err(ProviderError::InvalidResponse {
            fault: ResponseFault::NotTheSchema
        })
    );
}

// ---------------------------------------------------------------------------
// HTTP status
// ---------------------------------------------------------------------------

#[test]
fn a_groq_error_reply_keeps_its_status_and_classification() {
    let f = one_edge();
    let body = serde_json::to_vec(&serde_json::json!({
        "error": { "message": "Invalid API Key", "type": "invalid_request_error" }
    }))
    .expect("fixture");
    let provider = GroqProvider::new(MockTransport::replying(401, body));

    let error = provider
        .explain(&f.bundle, &question(), &Secret::new(fake_key()))
        .expect_err("401");

    assert_eq!(
        error,
        ProviderError::Refused {
            status: 401,
            kind: Some("invalid_request_error".to_owned()),
        }
    );
}

#[test]
fn every_refusal_status_is_reported() {
    let f = one_edge();
    for status in [400_u16, 401, 403, 413, 422, 429, 500, 502, 503] {
        let body = serde_json::to_vec(&serde_json::json!({
            "error": { "message": "no", "type": "rate_limit_exceeded" }
        }))
        .expect("fixture");
        let provider = GroqProvider::new(MockTransport::replying(status, body));

        assert_eq!(
            provider.explain(&f.bundle, &question(), &Secret::new(fake_key())),
            Err(ProviderError::Refused {
                status,
                kind: Some("rate_limit_exceeded".to_owned()),
            }),
            "status {status} was not reported"
        );
    }
}

#[test]
fn a_refusal_with_an_unreadable_body_still_reports_its_status() {
    let f = one_edge();
    let provider = GroqProvider::new(MockTransport::replying(502, b"<html>502</html>".to_vec()));

    assert_eq!(
        provider.explain(&f.bundle, &question(), &Secret::new(fake_key())),
        Err(ProviderError::Refused {
            status: 502,
            kind: None,
        })
    );
}

#[test]
fn an_implausible_classification_is_dropped_rather_than_repeated() {
    // `type` is chosen by whoever answered. A server that put a paragraph, a
    // credential or a path there would have turned a classification field into
    // the response-body channel decision C8 exists to close.
    let f = one_edge();
    let key = fake_key();
    for hostile in [
        format!("your key is {key}"),
        machine_paths().0,
        "x".repeat(4096),
        "Has Spaces And Capitals".to_owned(),
        String::new(),
    ] {
        let body = serde_json::to_vec(&serde_json::json!({
            "error": { "message": "m", "type": hostile }
        }))
        .expect("fixture");
        let provider = GroqProvider::new(MockTransport::replying(400, body));

        let error = provider
            .explain(&f.bundle, &question(), &Secret::new(fake_key()))
            .expect_err("400");

        assert_eq!(
            error,
            ProviderError::Refused {
                status: 400,
                kind: None
            },
            "an implausible classification was kept"
        );
        let rendered = format!("{error} {error:?}");
        assert!(!rendered.contains(&key), "a credential reached the error");
        assert!(!rendered.contains("C:\\"), "a path reached the error");
    }
}

// ---------------------------------------------------------------------------
// Transport failures
// ---------------------------------------------------------------------------

#[test]
fn every_transport_failure_reaches_the_caller_as_itself() {
    let f = one_edge();
    for cause in [
        TransportError::Request,
        TransportError::Connect,
        TransportError::Timeout,
        TransportError::Read,
    ] {
        let provider = GroqProvider::new(MockTransport::failing(cause));

        assert_eq!(
            provider.explain(&f.bundle, &question(), &Secret::new(fake_key())),
            Err(ProviderError::Transport { cause })
        );
    }
}

// ---------------------------------------------------------------------------
// Nothing leaks
// ---------------------------------------------------------------------------

#[test]
fn no_error_from_a_hostile_reply_carries_the_body_or_the_key() {
    // The realistic worst case: a reply designed to be quoted back.
    let f = one_edge();
    let key = fake_key();
    let (windows_path, unix_path) = machine_paths();
    let hostile = format!(
        "<html>leaked {key} at {windows_path} and {unix_path}</html>{}",
        "padding".repeat(4096)
    );

    for status in [200_u16, 400, 500] {
        let provider = GroqProvider::new(MockTransport::replying(
            status,
            hostile.clone().into_bytes(),
        ));

        let error = provider
            .explain(&f.bundle, &question(), &Secret::new(key.clone()))
            .expect_err("not an answer");

        let rendered = format!("{error} {error:?}");
        for forbidden in [
            key.as_str(),
            "leaked",
            "<html>",
            "C:\\",
            "/home/",
            "padding",
        ] {
            assert!(
                !rendered.contains(forbidden),
                "status {status}: the error carries {forbidden:?}: {rendered}"
            );
        }
        assert!(rendered.len() < 200, "the error is suspiciously large");
    }
}

#[test]
fn the_request_carries_no_identity_and_no_machine_path() {
    let f = fixture(&["alpha reaches beta", "beta reaches gamma"]);
    let key = fake_key();
    let provider = GroqProvider::new(MockTransport::replying(200, sound_reply(f.edges[0])));

    let _ = provider.explain(&f.bundle, &question(), &Secret::new(key.clone()));

    let body = String::from_utf8(provider.transport().only_call().body().to_vec()).expect("UTF-8");
    for forbidden in [
        key.as_str(),
        "Bearer",
        "C:\\",
        "/home/",
        "/Users/",
        "RepositoryIdentity",
    ] {
        assert!(
            !body.contains(forbidden),
            "the request carries {forbidden:?}"
        );
    }
    // And the evidence it must carry is still there.
    assert!(body.contains("alpha reaches beta"));
    assert!(body.contains("src/api/client.ts"));
}

// ---------------------------------------------------------------------------
// No network unless the path genuinely calls the provider
// ---------------------------------------------------------------------------

#[test]
fn a_degraded_path_makes_no_transport_call() {
    // The shape the desktop's opt-in and credential checks will take at step
    // 10: the provider is never constructed with a reachable transport unless
    // all three conditions hold, and `UnreachableTransport` turns a mistake
    // into a panic rather than a silent request.
    fn ask_if_allowed(
        opted_in: bool,
        credential: Option<Secret>,
        provider: &dyn Provider,
        f: &Fixture,
    ) -> Option<Result<cartograph_ask::Answer, ProviderError>> {
        let credential = credential?;
        if !opted_in {
            return None;
        }
        Some(provider.explain(&f.bundle, &question(), &credential))
    }

    let f = one_edge();
    let provider = GroqProvider::new(UnreachableTransport);

    assert!(ask_if_allowed(false, Some(Secret::new(fake_key())), &provider, &f).is_none());
    assert!(ask_if_allowed(true, None, &provider, &f).is_none());
    assert!(ask_if_allowed(false, None, &provider, &f).is_none());
}

#[test]
fn a_refused_answer_still_made_exactly_one_call() {
    // Distinguishes "did not try" from "tried and was refused" -- the two
    // outcomes a degraded-mode test must never confuse.
    let f = one_edge();
    let body = reply(&serde_json::json!([item(
        "Invented.",
        &serde_json::json!([citation(f.edges[0], "nowhere in the evidence")])
    )]));
    let provider = GroqProvider::new(MockTransport::replying(200, body));

    let _ = provider.explain(&f.bundle, &question(), &Secret::new(fake_key()));

    assert_eq!(provider.transport().call_count(), 1);
}
