//! The Groq wire format, exercised without a network.
//!
//! Nothing here reaches Groq. There is no HTTP client in the crate to reach
//! with, and every byte checked below was produced locally — which is the
//! point: ADR-0021 Amendment 3 decision C2 keeps serialisation in this
//! repository so that a test can inspect *the exact bytes that will be sent*.

use cartograph_ask::error::ResponseFault;
use cartograph_ask::groq::{
    MAX_COMPLETION_TOKENS, MODEL, SCHEMA_NAME, SYSTEM_INSTRUCTION, parse_answer,
    parse_answer_content, parse_error, request_body, user_content,
};
use cartograph_ask::{ProviderError, Question, validate};
use cartograph_core::{
    Confidence, EdgeId, EdgeKind, Evidence, NodeId, NodeKind, Provenance, SourceLocation,
};
use cartograph_graph::bundle::EvidenceBundle;
use cartograph_graph::{ArchitectureGraph, EdgeSpec};

// ---------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------

const EVIDENCE: &str = "fetch('/api/users') resolves to the users route handler";

fn node(graph: &mut ArchitectureGraph, name: &str, file: &str) -> NodeId {
    graph
        .node_for(
            NodeKind::Function,
            name,
            Some(SourceLocation::new(file, 1).expect("location")),
        )
        .expect("node")
}

fn edge_with(graph: &mut ArchitectureGraph, from: &str, to: &str, evidence: &str) -> EdgeId {
    let source = node(graph, from, "src/api/client.ts");
    let target = node(graph, to, "src/api/routes.py");
    graph
        .add_edge(EdgeSpec {
            source,
            target,
            kind: EdgeKind::HttpCall,
            confidence: Confidence::new(0.92).expect("confidence"),
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

fn body_json(bundle: &EvidenceBundle, question: &Question) -> serde_json::Value {
    let bytes = request_body(bundle, question).expect("serialised");
    serde_json::from_slice(&bytes).expect("the request we build is valid JSON")
}

/// A credential-shaped string, assembled at runtime.
///
/// QG-005 scans tracked files for exactly this shape and is right to: a
/// credential-shaped literal in a fixture is still one in the history.
fn fake_key() -> String {
    format!("{}{}", "ghp_", "abcdefghijklmnopqrstuvwxyz0123456789")
}

/// The `content` of a reply, built through serde.
///
/// Deliberately not built with `format!`: `EdgeId`'s `Display` writes `e0`,
/// while its wire form is `#[serde(transparent)]` over a `u64` and writes `0`.
/// Interpolating the human form produces a document that is not JSON at all,
/// which would test the parser against the wrong input.
fn reply(items: &serde_json::Value) -> String {
    serde_json::to_string(&serde_json::json!({ "items": items })).expect("fixture")
}

/// One item, with its citations.
fn reply_item(text: &str, citations: &serde_json::Value) -> serde_json::Value {
    serde_json::json!({ "text": text, "citations": citations })
}

/// One citation, with the edge written as the wire writes it.
fn reply_citation(edge: EdgeId, text: &str) -> serde_json::Value {
    serde_json::json!({ "edge": edge, "text": text })
}

/// A wire reply carrying `content` inside a chat-completions envelope,
/// including fields this repository does not model.
fn envelope(content: &str) -> Vec<u8> {
    serde_json::to_vec(&serde_json::json!({
        "id": "chatcmpl-not-a-real-id",
        "object": "chat.completion",
        "created": 1_767_225_600_u64,
        "model": MODEL,
        "choices": [{
            "index": 0,
            "message": { "role": "assistant", "content": content },
            "finish_reason": "stop"
        }],
        "usage": { "prompt_tokens": 10, "completion_tokens": 5, "total_tokens": 15 },
        "system_fingerprint": "fp_notreal"
    }))
    .expect("fixture")
}

// ---------------------------------------------------------------------------
// Request — the fields that must be there
// ---------------------------------------------------------------------------

#[test]
fn the_request_names_the_recorded_model() {
    let f = one_edge();

    let body = body_json(&f.bundle, &question());

    assert_eq!(body["model"], MODEL);
    assert_eq!(MODEL, "openai/gpt-oss-120b");
}

#[test]
fn streaming_is_off() {
    // Not a preference: Groq does not support streaming with structured
    // outputs, so a request that streamed would be rejected.
    let f = one_edge();

    assert_eq!(body_json(&f.bundle, &question())["stream"], false);
}

#[test]
fn the_response_format_is_a_strict_json_schema() {
    let f = one_edge();

    let body = body_json(&f.bundle, &question());
    let format = &body["response_format"];

    assert_eq!(format["type"], "json_schema");
    assert_eq!(format["json_schema"]["name"], SCHEMA_NAME);
    assert_eq!(format["json_schema"]["strict"], true);
}

#[test]
fn the_schema_obeys_strict_modes_two_requirements() {
    // Strict mode requires every property listed in `required`, and every
    // object closed with `additionalProperties: false`. A schema that broke
    // either would be rejected by the API rather than by us, so it is checked
    // here where the failure is legible.
    let f = one_edge();
    let body = body_json(&f.bundle, &question());
    let schema = &body["response_format"]["json_schema"]["schema"];

    let objects = [
        schema.clone(),
        schema["properties"]["items"]["items"].clone(),
        schema["properties"]["items"]["items"]["properties"]["citations"]["items"].clone(),
    ];

    for object in objects {
        assert_eq!(object["type"], "object", "not an object: {object}");
        assert_eq!(
            object["additionalProperties"], false,
            "object is not closed: {object}"
        );
        let properties = object["properties"]
            .as_object()
            .expect("an object schema has properties");
        let required: Vec<&str> = object["required"]
            .as_array()
            .expect("an object schema has required")
            .iter()
            .map(|v| v.as_str().expect("a name"))
            .collect();
        for name in properties.keys() {
            assert!(
                required.contains(&name.as_str()),
                "property {name} is not required, which strict mode forbids"
            );
        }
    }
}

#[test]
fn the_schema_asks_for_an_integer_edge() {
    // EdgeId is `#[serde(transparent)]` over a u64, so a string would not
    // round-trip into a ClaimedCitation.
    let f = one_edge();
    let body = body_json(&f.bundle, &question());

    assert_eq!(
        body["response_format"]["json_schema"]["schema"]["properties"]["items"]["items"]["properties"]
            ["citations"]["items"]["properties"]["edge"]["type"],
        "integer"
    );
}

#[test]
fn the_system_message_is_the_constant_and_nothing_else() {
    let f = one_edge();

    let body = body_json(&f.bundle, &question());

    assert_eq!(body["messages"][0]["role"], "system");
    assert_eq!(body["messages"][0]["content"], SYSTEM_INSTRUCTION);
}

#[test]
fn the_system_instruction_states_the_contract() {
    // The instruction does not enforce anything -- validate() does -- but it
    // must at least say what is being enforced, or the model is being marked
    // against a rubric it was never shown.
    for required in [
        "Use only the supplied evidence",
        "Never invent evidence",
        "at least one citation",
        "byte-for-byte substring",
        "Do not paraphrase",
        "may not add, remove or alter any relationship",
        "must not request, name or select one",
    ] {
        assert!(
            SYSTEM_INSTRUCTION.contains(required),
            "the system instruction does not say: {required}"
        );
    }
}

#[test]
fn the_user_message_carries_the_question_and_the_evidence() {
    let f = one_edge();
    let q = question();

    let body = body_json(&f.bundle, &q);
    assert_eq!(body["messages"][1]["role"], "user");

    let content: serde_json::Value =
        serde_json::from_str(body["messages"][1]["content"].as_str().expect("a string"))
            .expect("the user content is a JSON document");

    assert_eq!(content["question"], q.text());
    assert_eq!(content["evidence"][0]["evidence"], EVIDENCE);
    assert_eq!(content["evidence"][0]["edge"], f.edges[0].as_u64());
}

#[test]
fn the_payload_has_exactly_two_fields() {
    // The whole request model, asserted rather than described: adding a field
    // to what the provider sees fails here.
    let f = one_edge();

    let content = serde_json::to_value(user_content(&f.bundle, &question())).expect("serialised");
    let mut fields: Vec<&String> = content.as_object().expect("an object").keys().collect();
    fields.sort();

    assert_eq!(fields, ["evidence", "question"]);
}

#[test]
fn each_evidence_entry_carries_only_the_recorded_fields() {
    let f = one_edge();

    let content = serde_json::to_value(user_content(&f.bundle, &question())).expect("serialised");
    let mut fields: Vec<&String> = content["evidence"][0]
        .as_object()
        .expect("an object")
        .keys()
        .collect();
    fields.sort();

    assert_eq!(
        fields,
        [
            "confidence",
            "edge",
            "evidence",
            "from",
            "kind",
            "location",
            "provenance",
            "to"
        ]
    );
}

#[test]
fn every_bundled_edge_reaches_the_request() {
    let f = fixture(&[
        "alpha reaches beta",
        "beta reaches gamma",
        "gamma writes users",
    ]);

    let content = serde_json::to_value(user_content(&f.bundle, &question())).expect("serialised");

    let sent = content["evidence"].as_array().expect("an array");
    assert_eq!(sent.len(), 3, "an edge was dropped on the way out");
    for (i, text) in [
        "alpha reaches beta",
        "beta reaches gamma",
        "gamma writes users",
    ]
    .iter()
    .enumerate()
    {
        assert_eq!(&sent[i]["evidence"], text);
    }
}

#[test]
fn evidence_text_crosses_byte_for_byte() {
    // Interior spacing, punctuation and quoting are part of the evidence. A
    // citation must be an exact substring of it, so anything that "tidied" it
    // here would make every citation unverifiable.
    let awkward = "calls   foo( 'a\\b' )  -- see line 3;  \"quoted\"";
    let f = fixture(&[awkward]);

    let content = serde_json::to_value(user_content(&f.bundle, &question())).expect("serialised");

    assert_eq!(
        content["evidence"][0]["evidence"]
            .as_str()
            .expect("text")
            .as_bytes(),
        awkward.as_bytes()
    );
}

#[test]
fn a_question_crosses_byte_for_byte() {
    let written = "  Why does this reach the users table?  ";
    let f = one_edge();
    let q = Question::new(written).expect("question");

    let content = serde_json::to_value(user_content(&f.bundle, &q)).expect("serialised");

    assert_eq!(content["question"], written);
}

#[test]
fn unicode_survives_the_round_trip() {
    let evidence = "appelle « utilisateurs » — 日本語 — emoji 🛰 included";
    let f = fixture(&[evidence]);

    let bytes = request_body(&f.bundle, &question()).expect("serialised");
    let body: serde_json::Value = serde_json::from_slice(&bytes).expect("valid JSON");
    let content: serde_json::Value =
        serde_json::from_str(body["messages"][1]["content"].as_str().expect("a string"))
            .expect("valid JSON");

    assert_eq!(content["evidence"][0]["evidence"], evidence);
}

// ---------------------------------------------------------------------------
// Request — determinism of the bytes, and only of the bytes
// ---------------------------------------------------------------------------

#[test]
fn the_same_request_serialises_to_the_same_bytes() {
    // The request is deterministic. The ANSWER is not, and nothing here claims
    // it is: Groq converts `temperature: 0` to 1e-8 server-side, so identical
    // requests may produce different sentences.
    let f = fixture(&["alpha reaches beta", "beta reaches gamma"]);
    let q = question();

    let first = request_body(&f.bundle, &q).expect("serialised");
    let second = request_body(&f.bundle, &q).expect("serialised");
    let third = request_body(&f.bundle, &q).expect("serialised");

    assert_eq!(first, second);
    assert_eq!(second, third);
}

#[test]
fn two_bundles_with_the_same_contents_serialise_identically() {
    let a = fixture(&["alpha reaches beta"]);
    let b = fixture(&["alpha reaches beta"]);
    let q = question();

    assert_eq!(
        request_body(&a.bundle, &q).expect("serialised"),
        request_body(&b.bundle, &q).expect("serialised")
    );
}

#[test]
fn the_request_is_capped_and_bounded() {
    let f = one_edge();

    assert_eq!(
        body_json(&f.bundle, &question())["max_completion_tokens"],
        MAX_COMPLETION_TOKENS
    );
}

// ---------------------------------------------------------------------------
// Request — privacy
// ---------------------------------------------------------------------------

#[test]
fn the_request_carries_no_credential_no_identity_and_no_machine_path() {
    let f = fixture(&["alpha reaches beta", "beta reaches gamma"]);
    let key = fake_key();

    let bytes = request_body(&f.bundle, &question()).expect("serialised");
    let text = String::from_utf8(bytes).expect("UTF-8");

    for forbidden in [
        key.as_str(),
        "Bearer",
        "Authorization",
        "authorization",
        "C:\\",
        "/home/",
        "/Users/",
        "repository_identity",
        "repositoryIdentity",
        "RepositoryIdentity",
        "api_key",
        "apiKey",
    ] {
        assert!(
            !text.contains(forbidden),
            "the request carries {forbidden:?}"
        );
    }
}

#[test]
fn the_request_carries_no_absolute_path() {
    let f = one_edge();

    let content = serde_json::to_value(user_content(&f.bundle, &question())).expect("serialised");

    for entry in content["evidence"].as_array().expect("an array") {
        let file = entry["location"]["file"].as_str().expect("a file");
        assert!(!file.starts_with('/'), "absolute path sent: {file}");
        assert!(!file.starts_with('\\'), "absolute path sent: {file}");
        assert!(!file.contains(':'), "drive-absolute path sent: {file}");
        assert!(!file.contains(".."), "escaping path sent: {file}");
    }
}

#[test]
fn repository_relative_evidence_is_still_present() {
    // The other half of the privacy rule, and the reason it is stated as "no
    // ABSOLUTE path" rather than "no path": relative locations must survive,
    // or the answer cannot say where anything is.
    let f = one_edge();

    let content = serde_json::to_value(user_content(&f.bundle, &question())).expect("serialised");

    assert_eq!(
        content["evidence"][0]["location"]["file"],
        "src/api/client.ts"
    );
    assert_eq!(content["evidence"][0]["location"]["line"], 42);
    assert!(
        content["evidence"][0]["from"]
            .as_str()
            .expect("an identity")
            .contains("src/api/client.ts"),
        "the source identity lost its file"
    );
}

// ---------------------------------------------------------------------------
// Response — accepted
// ---------------------------------------------------------------------------

#[test]
fn a_well_formed_reply_parses_into_a_claim() {
    let f = one_edge();
    let content = reply(&serde_json::json!([reply_item(
        "It calls the route.",
        &serde_json::json!([reply_citation(f.edges[0], "the users route handler")])
    )]));

    let claim = parse_answer(&envelope(&content)).expect("a well-formed reply");

    assert_eq!(claim.items.len(), 1);
    assert_eq!(claim.items[0].text, "It calls the route.");
    assert_eq!(claim.items[0].citations[0].edge, f.edges[0]);
    assert_eq!(claim.items[0].citations[0].text, "the users route handler");
}

#[test]
fn several_items_and_several_citations_parse() {
    let f = fixture(&["alpha reaches beta", "beta reaches gamma"]);
    let content = reply(&serde_json::json!([
        reply_item(
            "One.",
            &serde_json::json!([
                reply_citation(f.edges[0], "alpha"),
                reply_citation(f.edges[1], "beta")
            ])
        ),
        reply_item(
            "Two.",
            &serde_json::json!([reply_citation(f.edges[1], "gamma")])
        )
    ]));

    let claim = parse_answer_content(&content).expect("well formed");

    assert_eq!(claim.items.len(), 2);
    assert_eq!(claim.items[0].citations.len(), 2);
    assert_eq!(claim.items[1].citations.len(), 1);
}

#[test]
fn json_whitespace_does_not_matter() {
    let f = one_edge();
    let spaced = format!(
        "{{\n  \"items\" : [ {{\n    \"text\" : \"Held.\" ,\n    \"citations\" : [ {{ \"edge\" : {} , \"text\" : \"fetch\" }} ]\n  }} ]\n}}",
        f.edges[0].as_u64()
    );

    let claim = parse_answer_content(&spaced).expect("whitespace is not significant in JSON");

    assert_eq!(claim.items[0].text, "Held.");
}

#[test]
fn escaped_characters_and_unicode_parse_back_exactly() {
    let f = one_edge();
    let content = serde_json::to_string(&serde_json::json!({
        "items": [{
            "text": "Quotes \" backslash \\ newline \n and 日本語 🛰",
            "citations": [{ "edge": 0, "text": "a\tb" }]
        }]
    }))
    .expect("fixture");

    let claim = parse_answer_content(&content).expect("well formed");

    assert_eq!(
        claim.items[0].text,
        "Quotes \" backslash \\ newline \n and 日本語 🛰"
    );
    assert_eq!(claim.items[0].citations[0].text, "a\tb");
    let _ = f;
}

#[test]
fn a_long_reply_parses() {
    let long = "x".repeat(64 * 1024);
    let content = serde_json::to_string(&serde_json::json!({
        "items": [{ "text": long.clone(), "citations": [{ "edge": 0, "text": "y" }] }]
    }))
    .expect("fixture");

    let claim = parse_answer_content(&content).expect("length is not a schema violation");

    assert_eq!(claim.items[0].text.len(), long.len());
}

#[test]
fn an_empty_item_list_parses_and_is_rejected_only_by_validation() {
    // The schema permits `{"items": []}`; the citation contract does not. That
    // split is the architecture: shape here, truth there.
    let f = one_edge();

    let claim = parse_answer_content(r#"{"items":[]}"#).expect("shape is fine");

    assert_eq!(
        validate(&f.bundle, claim),
        Err(ProviderError::NoItems),
        "an empty answer must be refused by validation, not by the parser"
    );
}

// ---------------------------------------------------------------------------
// Response — malformed
// ---------------------------------------------------------------------------

#[test]
fn a_body_that_is_not_json_is_refused() {
    for body in [
        &b"not json at all"[..],
        &b""[..],
        &b"{"[..],
        &b"<html>502 Bad Gateway</html>"[..],
    ] {
        assert_eq!(
            parse_answer(body),
            Err(ProviderError::InvalidResponse {
                fault: ResponseFault::NotJson
            }),
            "accepted a non-JSON body"
        );
    }
}

#[test]
fn an_envelope_with_no_choices_is_refused() {
    let body = serde_json::to_vec(&serde_json::json!({ "choices": [] })).expect("fixture");

    assert_eq!(
        parse_answer(&body),
        Err(ProviderError::InvalidResponse {
            fault: ResponseFault::NoChoices
        })
    );
}

#[test]
fn a_document_that_is_not_the_schema_is_refused() {
    for content in [
        r"{}",                                                               // missing items
        r#"{"items":"not an array"}"#,                                       // wrong type
        r#"{"items":[{"citations":[]}]}"#,                                   // missing text
        r#"{"items":[{"text":42,"citations":[]}]}"#,                         // wrong text type
        r#"{"items":[{"text":"a"}]}"#,                                       // missing citations
        r#"{"items":[{"text":"a","citations":"no"}]}"#,                      // wrong citations type
        r#"{"items":[{"text":"a","citations":[{"text":"b"}]}]}"#,            // missing edge
        r#"{"items":[{"text":"a","citations":[{"edge":0}]}]}"#, // missing citation text
        r#"{"items":[{"text":"a","citations":[{"edge":"0","text":"b"}]}]}"#, // edge as string
        r#"{"explanations":[]}"#,                               // wrong field name
        r#"{"items":[],"extra":true}"#,                         // undeclared field
        r#"{"items":[{"text":"a","citations":[],"note":"x"}]}"#, // undeclared nested field
    ] {
        assert_eq!(
            parse_answer_content(content),
            Err(ProviderError::InvalidResponse {
                fault: ResponseFault::NotTheSchema
            }),
            "accepted a document that is not the schema: {content}"
        );
    }
}

#[test]
fn a_malformed_reply_never_panics() {
    // Every failure above is a Result. Asserted as a property rather than
    // implied by the tests passing, because a panic in a provider path would
    // take the desktop down with it.
    for body in [
        &b"\xff\xfe not utf 8"[..],
        &b"[]"[..],
        &b"null"[..],
        &b"1234"[..],
        &br#"{"choices":[{"message":{}}]}"#[..],
        &br#"{"choices":[{"message":{"content":"not json"}}]}"#[..],
    ] {
        assert!(parse_answer(body).is_err(), "expected a refusal");
    }
}

#[test]
fn a_parse_failure_never_carries_the_body() {
    // The specific hazard: serde's own message quotes the input it choked on.
    // Discarding it costs a coarser diagnostic and keeps a response body out
    // of every log that will ever be written.
    let key = fake_key();
    let body = format!(r#"{{"items":[{{"text":"leak {key}","citations":"#);

    let error = parse_answer_content(&body).expect_err("malformed");
    let rendered = format!("{error} {error:?}");

    assert!(
        !rendered.contains(&key),
        "the error carries the body: {rendered}"
    );
    assert!(
        !rendered.contains("leak"),
        "the error carries the body: {rendered}"
    );
}

#[test]
fn an_api_error_keeps_its_classification_and_drops_its_message() {
    let key = fake_key();
    let body = serde_json::to_vec(&serde_json::json!({
        "error": {
            "message": format!("something about {key}"),
            "type": "invalid_request_error"
        }
    }))
    .expect("fixture");

    let error = parse_error(&body).expect("an error document");

    assert_eq!(error.kind(), "invalid_request_error");
    let rendered = format!("{error:?}");
    assert!(
        !rendered.contains(&key),
        "the API error retained its message: {rendered}"
    );
}

// ---------------------------------------------------------------------------
// The boundary: parsing is not trusting
// ---------------------------------------------------------------------------

#[test]
fn a_reply_can_parse_perfectly_and_still_be_refused() {
    // The property Step 3 exists to preserve. Groq's strict schema guarantees
    // the shape; it cannot guarantee the content. A citation quoting text no
    // edge contains parses without complaint and is rejected by validation.
    let f = one_edge();
    let content = reply(&serde_json::json!([reply_item(
        "It writes to orders.",
        &serde_json::json!([reply_citation(f.edges[0], "the orders table")])
    )]));

    let claim = parse_answer(&envelope(&content)).expect("the shape is impeccable");

    assert!(
        matches!(
            validate(&f.bundle, claim),
            Err(ProviderError::InvalidCitation { .. })
        ),
        "an invented quotation survived validation"
    );
}

#[test]
fn a_parsed_reply_becomes_an_answer_only_through_validation() {
    let f = one_edge();
    let content = reply(&serde_json::json!([reply_item(
        "It calls the route.",
        &serde_json::json!([reply_citation(f.edges[0], "the users route handler")])
    )]));

    let claim = parse_answer(&envelope(&content)).expect("well formed");
    let answer = validate(&f.bundle, claim).expect("and true");

    assert_eq!(answer.len(), 1);
    assert_eq!(answer.scope(), f.bundle.scope());
}

#[test]
fn an_uncited_item_survives_parsing_and_is_refused_by_validation() {
    let f = one_edge();

    let claim =
        parse_answer_content(r#"{"items":[{"text":"Trust me.","citations":[]}]}"#).expect("shape");

    assert_eq!(
        validate(&f.bundle, claim),
        Err(ProviderError::UncitedItem { item: 0 })
    );
}
