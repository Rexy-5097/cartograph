//! The transport seam, exercised without a transport.
//!
//! Nothing here opens a socket, and nothing in the crate could: there is no
//! HTTP client to open one with. Every guarantee checked below is one the
//! eventual `ureq` step inherits rather than re-establishes.

use cartograph_ask::groq::request_body;
use cartograph_ask::testing::{MockTransport, UnreachableTransport};
use cartograph_ask::{Header, HttpResponse, Question, Transport, TransportError};
use cartograph_core::{
    Confidence, EdgeId, EdgeKind, Evidence, NodeId, NodeKind, Provenance, SourceLocation,
};
use cartograph_graph::bundle::EvidenceBundle;
use cartograph_graph::{ArchitectureGraph, EdgeSpec};

// ---------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------

const URL: &str = "https://api.groq.com/openai/v1/chat/completions";

fn node(graph: &mut ArchitectureGraph, name: &str) -> NodeId {
    graph
        .node_for(
            NodeKind::Function,
            name,
            Some(SourceLocation::new("src/api/client.ts", 1).expect("location")),
        )
        .expect("node")
}

fn bundle_of(evidence: &str) -> (EvidenceBundle, EdgeId) {
    let mut graph = ArchitectureGraph::new();
    let source = node(&mut graph, "caller");
    let target = node(&mut graph, "callee");
    let edge = graph
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
        .expect("edge");
    let bundle = EvidenceBundle::from_edges(&graph, [edge]).expect("bundle");
    (bundle, edge)
}

/// A credential-shaped string, assembled at runtime.
///
/// QG-005 scans tracked files for exactly this shape and is right to.
fn fake_key() -> String {
    format!("{}{}", "ghp_", "abcdefghijklmnopqrstuvwxyz0123456789")
}

fn authorization(key: &str) -> String {
    format!("Bearer {key}")
}

// ---------------------------------------------------------------------------
// What reaches the transport
// ---------------------------------------------------------------------------

#[test]
fn the_url_reaches_the_transport_unchanged() {
    let transport = MockTransport::replying(200, b"{}".to_vec());

    transport.post_json(URL, &[], b"{}").expect("replied");

    assert_eq!(transport.only_call().url(), URL);
}

#[test]
fn header_names_reach_the_transport_in_order() {
    let key = fake_key();
    let auth = authorization(&key);
    let transport = MockTransport::replying(200, b"{}".to_vec());

    transport
        .post_json(
            URL,
            &[
                Header::new("Authorization", &auth),
                Header::new("Content-Type", "application/json"),
            ],
            b"{}",
        )
        .expect("replied");

    let call = transport.only_call();
    assert_eq!(call.header_names(), ["Authorization", "Content-Type"]);
}

#[test]
fn a_header_can_be_proved_present_without_anything_holding_its_value() {
    // The property that lets a test assert a credential was attached without a
    // credential ever entering a fixture, an assertion message or CI output.
    let key = fake_key();
    let auth = authorization(&key);
    let transport = MockTransport::replying(200, b"{}".to_vec());

    transport
        .post_json(URL, &[Header::new("Authorization", &auth)], b"{}")
        .expect("replied");

    let call = transport.only_call();
    assert!(
        call.has_header("authorization"),
        "matched case-insensitively"
    );
    assert!(call.has_header("AUTHORIZATION"));
    assert!(!call.has_header("X-Api-Key"));

    let recorded = format!("{call:?} {:?}", call.header_names());
    assert!(
        !recorded.contains(&key),
        "the recorder kept the value: {recorded}"
    );
    assert!(!recorded.contains("Bearer"), "the recorder kept the value");
}

#[test]
fn the_request_body_arrives_byte_for_byte() {
    // The whole reason `ureq`'s json feature stays off: the bytes the privacy
    // tests inspected are the bytes that go out.
    let (bundle, _) = bundle_of("fetch('/api/users')   resolves  -- \"quoted\"");
    let question = Question::new("What does this reach?").expect("question");
    let body = request_body(&bundle, &question).expect("serialised");

    let transport = MockTransport::replying(200, b"{}".to_vec());
    transport.post_json(URL, &[], &body).expect("replied");

    let observed = transport.only_call();
    assert_eq!(
        observed.body(),
        body.as_slice(),
        "the transport did not receive the bytes it was given"
    );
    assert_eq!(observed.body().len(), body.len());
}

#[test]
fn nothing_reformats_the_body_on_the_way_through() {
    // Deliberately not valid JSON, and deliberately awkward: a transport that
    // parsed and re-emitted would change it, and one that round-tripped it
    // through String would reject it.
    let awkward: &[u8] = b"{ \"a\" :  1 ,\t\"b\":[ ] }\n\x00\xff not utf 8";
    let transport = MockTransport::replying(200, Vec::new());

    transport.post_json(URL, &[], awkward).expect("replied");

    assert_eq!(transport.only_call().body(), awkward);
}

#[test]
fn an_empty_body_is_carried_faithfully() {
    let transport = MockTransport::replying(200, Vec::new());

    transport.post_json(URL, &[], b"").expect("replied");

    assert!(transport.only_call().body().is_empty());
}

// ---------------------------------------------------------------------------
// What comes back
// ---------------------------------------------------------------------------

#[test]
fn a_successful_reply_returns_its_status_and_bytes() {
    let payload = br#"{"choices":[]}"#;
    let transport = MockTransport::replying(200, payload.to_vec());

    let response = transport.post_json(URL, &[], b"{}").expect("replied");

    assert_eq!(response.status(), 200);
    assert!(response.is_success());
    assert_eq!(response.body(), payload);
}

#[test]
fn a_non_success_reply_is_still_a_reply() {
    // The design decision this seam pins down. `ureq` turns a non-2xx into an
    // error and throws the body away by default; Groq puts its error
    // classification IN that body, and `groq::parse_error` exists to read it.
    // So a 4xx arrives here as Ok, and the implementation must configure
    // `http_status_as_error(false)`.
    let body = br#"{"error":{"message":"nope","type":"invalid_request_error"}}"#;
    let transport = MockTransport::replying(400, body.to_vec());

    let response = transport
        .post_json(URL, &[], b"{}")
        .expect("the server replied");

    assert_eq!(response.status(), 400);
    assert!(!response.is_success());
    assert_eq!(
        response.body(),
        body,
        "the error body must survive to reach groq::parse_error"
    );
}

#[test]
fn require_success_is_the_shortcut_for_callers_that_do_not_want_the_body() {
    let transport = MockTransport::replying(503, b"upstream is unwell".to_vec());

    let error = transport
        .post_json(URL, &[], b"{}")
        .expect("replied")
        .require_success()
        .expect_err("503 is not success");

    assert_eq!(error, TransportError::Status { status: 503 });
}

#[test]
fn the_success_range_is_exactly_two_hundreds() {
    for status in [200_u16, 201, 204, 299] {
        assert!(
            HttpResponse::new(status, Vec::new()).is_success(),
            "{status} should be success"
        );
    }
    for status in [100_u16, 199, 300, 301, 400, 404, 429, 500, 503] {
        assert!(
            !HttpResponse::new(status, Vec::new()).is_success(),
            "{status} should not be success"
        );
    }
}

#[test]
fn a_body_the_transport_cannot_understand_is_passed_on_untouched() {
    // The seam does not interpret a provider's schema. Handing back nonsense is
    // correct behaviour: deciding it is nonsense belongs to groq::parse_answer.
    let html = b"<html><body>502 Bad Gateway</body></html>";
    let transport = MockTransport::replying(502, html.to_vec());

    let response = transport.post_json(URL, &[], b"{}").expect("replied");

    assert_eq!(response.body(), html);
}

// ---------------------------------------------------------------------------
// Failures
// ---------------------------------------------------------------------------

#[test]
fn a_connection_failure_reaches_the_caller() {
    let transport = MockTransport::failing(TransportError::Connect);

    assert_eq!(
        transport.post_json(URL, &[], b"{}"),
        Err(TransportError::Connect)
    );
}

#[test]
fn every_failure_category_round_trips() {
    for error in [
        TransportError::Request,
        TransportError::Connect,
        TransportError::Timeout,
        TransportError::Read,
        TransportError::Status { status: 429 },
    ] {
        let transport = MockTransport::failing(error);
        assert_eq!(transport.post_json(URL, &[], b"{}"), Err(error));
    }
}

#[test]
fn a_failure_still_records_that_the_call_happened() {
    // So a test can distinguish "did not try" from "tried and failed" -- which
    // is the whole point of the degraded-mode guarantees.
    let transport = MockTransport::failing(TransportError::Timeout);

    let _ = transport.post_json(URL, &[], b"{}");

    assert_eq!(transport.call_count(), 1);
}

// ---------------------------------------------------------------------------
// Privacy
// ---------------------------------------------------------------------------

#[test]
fn no_transport_error_carries_a_body_a_header_or_a_url() {
    let key = fake_key();

    for error in [
        TransportError::Request,
        TransportError::Connect,
        TransportError::Timeout,
        TransportError::Read,
        TransportError::Status { status: 401 },
    ] {
        let rendered = format!("{error} {error:?}");
        for forbidden in [
            key.as_str(),
            "Bearer",
            "api.groq.com",
            "https://",
            "<html>",
            "C:\\",
            "/home/",
            "/Users/",
        ] {
            assert!(
                !rendered.contains(forbidden),
                "{error:?} carries {forbidden:?}: {rendered}"
            );
        }
    }
}

#[test]
fn a_response_never_prints_its_body() {
    // The realistic leak is nobody printing the response, and something
    // printing the structure that holds one.
    let key = fake_key();
    let body = format!(r#"{{"error":"leaked {key}","html":"<html>500</html>"}}"#);
    let response = HttpResponse::new(500, body.clone().into_bytes());

    let debugged = format!("{:?}", Some(vec![response]));

    assert!(!debugged.contains(&key), "the body printed: {debugged}");
    assert!(!debugged.contains("leaked"), "the body printed: {debugged}");
    assert!(!debugged.contains("<html>"), "the body printed: {debugged}");
    assert!(debugged.contains("bytes"), "the size is still useful");
}

#[test]
fn a_header_never_prints_its_value() {
    let key = fake_key();
    let auth = authorization(&key);
    let header = Header::new("Authorization", &auth);

    let debugged = format!("{:?}", vec![header]);

    assert!(!debugged.contains(&key), "the value printed: {debugged}");
    assert!(
        !debugged.contains("Bearer"),
        "the value printed: {debugged}"
    );
    assert!(debugged.contains("Authorization"), "the name is not secret");
    assert!(debugged.contains("<redacted>"));
}

#[test]
fn a_large_response_does_not_reach_a_debug_line() {
    let response = HttpResponse::new(500, "x".repeat(256 * 1024).into_bytes());

    let debugged = format!("{response:?}");

    assert!(
        debugged.len() < 100,
        "the body reached Debug: {} chars",
        debugged.len()
    );
    assert!(debugged.contains("262144 bytes"));
}

#[test]
fn the_transport_adds_nothing_to_what_it_was_given() {
    // Neither a machine path nor an identity can be introduced here, because
    // the seam has no source for either. Asserted so that an implementation
    // that started decorating requests would be caught.
    let (bundle, _) = bundle_of("caller reaches callee");
    let question = Question::new("What does this reach?").expect("question");
    let body = request_body(&bundle, &question).expect("serialised");
    let transport = MockTransport::replying(200, b"{}".to_vec());

    transport.post_json(URL, &[], &body).expect("replied");

    let observed = transport.only_call();
    let text = String::from_utf8(observed.body().to_vec()).expect("UTF-8");
    for forbidden in ["C:\\", "/home/", "/Users/", "RepositoryIdentity", "Bearer"] {
        assert!(
            !text.contains(forbidden),
            "the transport added {forbidden:?}"
        );
    }
    assert_eq!(observed.body(), body.as_slice());
}

#[test]
fn a_credential_does_not_become_part_of_the_body_by_itself() {
    // Headers and bodies are separate inputs, and nothing copies one into the
    // other. Stated as a test because the day a helper "conveniently" merged
    // them is the day a key ships in a request payload.
    let key = fake_key();
    let auth = authorization(&key);
    let transport = MockTransport::replying(200, b"{}".to_vec());

    transport
        .post_json(URL, &[Header::new("Authorization", &auth)], b"{\"a\":1}")
        .expect("replied");

    let observed = transport.only_call();
    let text = String::from_utf8(observed.body().to_vec()).expect("UTF-8");
    assert_eq!(text, "{\"a\":1}");
    assert!(!text.contains(&key));
}

// ---------------------------------------------------------------------------
// Determinism, and the no-network guarantee
// ---------------------------------------------------------------------------

#[test]
fn the_mock_answers_the_same_way_every_time() {
    let transport = MockTransport::replying(200, b"stable".to_vec());

    let first = transport.post_json(URL, &[], b"a").expect("replied");
    let second = transport.post_json(URL, &[], b"b").expect("replied");
    let third = transport.post_json(URL, &[], b"c").expect("replied");

    assert_eq!(first, second);
    assert_eq!(second, third);
    assert_eq!(transport.call_count(), 3);
    assert_eq!(transport.calls()[0].body(), b"a");
    assert_eq!(transport.calls()[2].body(), b"c");
}

#[test]
#[should_panic(expected = "the network was reached on a path that must never reach it")]
fn the_unreachable_transport_fails_the_test_if_it_is_called() {
    let _ = UnreachableTransport.post_json(URL, &[], b"{}");
}

#[test]
fn a_path_that_must_not_reach_the_network_can_be_proved_not_to() {
    // The shape every degraded-mode test will take: wire in a transport that
    // cannot be called, run the path, and let a panic be the failure. Stands in
    // here for the desktop's opt-in and credential checks, which arrive later.
    fn degraded_ask(ai_enabled: bool, transport: &dyn Transport) -> &'static str {
        if !ai_enabled {
            return "raw evidence";
        }
        let _ = transport.post_json(URL, &[], b"{}");
        "explained"
    }

    assert_eq!(degraded_ask(false, &UnreachableTransport), "raw evidence");
}

// ---------------------------------------------------------------------------
// Provider independence
// ---------------------------------------------------------------------------

#[test]
fn the_seam_carries_anything_and_understands_none_of_it() {
    // Not a provider vocabulary: no chat completions, no schema, no evidence.
    // Bytes go in, bytes come out, and a body that means nothing to anyone is
    // handled exactly like one that means something.
    let transport = MockTransport::replying(200, b"anything at all".to_vec());

    let response = transport
        .post_json(
            "https://example.invalid/some/other/api",
            &[Header::new("X-Whatever", "1")],
            b"not json, not a chat completion, not evidence",
        )
        .expect("replied");

    assert_eq!(response.body(), b"anything at all");
    assert_eq!(
        transport.only_call().body(),
        b"not json, not a chat completion, not evidence"
    );
}
