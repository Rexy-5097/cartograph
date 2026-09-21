//! The Groq wire format: the exact bytes of a request, and the parsing of a
//! reply. **No transport.**
//!
//! # Why the bytes are built here and not by a client library
//!
//! [ADR-0021](../../../docs/adr/ADR-0021-ask-boundary-and-citation-contract.md)
//! Amendment 3, decision C2, leaves `ureq`'s `json` feature **off** so that
//! this repository serialises the request itself. The reason is testability of
//! a security property: decision C6 requires a test over *the exact bytes
//! sent*, and a test cannot check bytes it did not see. So [`request_body`]
//! returns a `Vec<u8>`, the privacy tests inspect that value, and a later step
//! hands the very same value to the transport.
//!
//! No SDK, for the same reason and one more: the request is four fields, and a
//! body this repository did not construct cannot be byte-checked.
//!
//! # Parsing is not trusting
//!
//! A reply that parses is a reply that had the right *shape*. Shape is not
//! truth: a model can emit a perfectly-formed citation quoting text no edge
//! contains, and Groq's strict structured output guarantees the schema, never
//! the content.
//!
//! So [`parse_answer`] returns a [`ClaimedAnswer`] — deliberately the
//! unvalidated type — and there is no path from a wire reply to an [`Answer`]
//! that skips [`validate`]:
//!
//! ```text
//! response bytes → parse_answer → ClaimedAnswer → validate → Answer
//! ```
//!
//! Nothing in this module can construct an `Answer`, because `Answer` has no
//! public constructor.
//!
//! # What crosses, and what cannot
//!
//! The request carries a constant system instruction, the user's question, and
//! the bundle's already-derived evidence. That is the whole payload, and
//! [`UserContent`] is a closed struct with two fields, so it is not a list to
//! be kept in sync with prose — adding a field would be a visible change here.
//!
//! Absent, and structurally rather than by omission: source text (`Evidence`
//! may not quote a file), the `ArchitectureGraph` (a bundle is a copy),
//! `RepositoryIdentity` (not nameable in this crate), absolute paths
//! (`SourceLocation` refuses them at construction), environment values
//! (`NodeKind::EnvVar` has no field for one), and the credential — which does
//! not appear in this module at all, because a credential belongs to a header
//! and there is no transport here to write one.
//!
//! Repository-**relative** paths do cross, in `location` and inside evidence
//! text. ADR-0021 Amendment 3 decision C4 records why: evidence must travel
//! byte for byte or a verbatim citation cannot be checked against it.
//!
//! # Determinism, stated exactly
//!
//! The **request** is deterministic: the same bundle and question serialise to
//! the same bytes, and a test asserts it. The **answer is not**, and this
//! module does not claim otherwise. Groq converts `temperature: 0` to `1e-8`
//! server-side, so identical requests may produce different sentences.
//! Determinism belongs to the analysis; the explanation is *checked*, not
//! reproduced.
//!
//! [`Answer`]: crate::Answer
//! [`ClaimedAnswer`]: crate::ClaimedAnswer
//! [`validate`]: crate::validate

use cartograph_core::{Confidence, EdgeId, EdgeKind, Provenance, Secret, SourceLocation};
use cartograph_graph::bundle::{BundledEdge, EvidenceBundle};
use serde::{Deserialize, Serialize};

use crate::answer::{Answer, ClaimedAnswer, ClaimedCitation, ClaimedItem, validate};
use crate::error::{ProviderError, ResponseFault};
use crate::provider::Provider;
use crate::question::Question;
use crate::transport::{Header, HttpResponse, Transport};

/// The model this repository sends to.
///
/// Recorded in ADR-0021 Amendment 3, decision C1, and verified there against
/// Groq's own documentation: a production model, 131,072-token context, 65,536
/// max completion tokens, strict structured output supported.
///
/// There is deliberately no model-selection interface and no fallback model.
pub const MODEL: &str = "openai/gpt-oss-120b";

/// The name Groq is given for the response schema. Cosmetic to the protocol,
/// fixed here so the request stays byte-stable.
pub const SCHEMA_NAME: &str = "cartograph_answer";

/// The ceiling on the reply. An explanation is a few sentences; this is
/// generous for that and far below the model's 65,536.
pub const MAX_COMPLETION_TOKENS: u32 = 2048;

/// The contract the model is held to, as a compile-time constant.
///
/// It is **not** assembled from user input, and not templated: a system
/// instruction that varied with what a caller supplied would be an injection
/// surface, and one that varied at all would make the request non-deterministic
/// for no benefit. It is version-controlled here and changes only by a commit.
///
/// It states the rules; it does not enforce them. Every one of them is enforced
/// again locally by [`validate`], because an instruction is a
/// request and a check is a guarantee.
pub const SYSTEM_INSTRUCTION: &str = "\
You are given evidence that Cartograph already derived by static analysis of a \
repository. Explain it. You are not analysing code: you cannot see any source \
file, and none will be provided.

Rules, all of which are checked after you answer:

1. Use only the supplied evidence. Do not use outside knowledge about \
libraries, frameworks or naming conventions to assert a relationship.
2. Never invent evidence, an edge, or a relationship. If the evidence does not \
answer the question, say so plainly. Unknown stays unknown.
3. Every explanation item must carry at least one citation. An item with no \
citation is rejected, and one rejected item rejects the whole answer.
4. A citation quotes one supplied edge's `evidence` field exactly: a \
contiguous, byte-for-byte substring of it. Do not paraphrase, reflow, correct, \
translate, summarise or re-punctuate the text you quote.
5. Cite by the `edge` number given in the evidence. Do not cite an edge that \
was not supplied.
6. You may not add, remove or alter any relationship, and you may not change \
the evidence text. You are explaining a graph, not editing one.
7. You have no authority over which repository is being analysed and must not \
request, name or select one.

Confidence values are uncalibrated priors, not probabilities. Do not present \
one as a likelihood.";

// ---------------------------------------------------------------------------
// Request
// ---------------------------------------------------------------------------

/// One chat message. `role` is fixed by the caller and never user-supplied.
#[derive(Debug, Serialize)]
struct Message<'a> {
    role: &'static str,
    content: &'a str,
}

/// `response_format`, the strict structured-output request.
#[derive(Debug, Serialize)]
struct ResponseFormat {
    #[serde(rename = "type")]
    kind: &'static str,
    json_schema: JsonSchema,
}

/// The schema Groq constrains decoding to.
#[derive(Debug, Serialize)]
struct JsonSchema {
    name: &'static str,
    strict: bool,
    schema: serde_json::Value,
}

/// The chat-completions body.
///
/// Field order here is the field order on the wire, which is what makes the
/// request byte-stable.
#[derive(Debug, Serialize)]
struct ChatRequest<'a> {
    model: &'static str,
    messages: Vec<Message<'a>>,
    response_format: ResponseFormat,
    /// Always `false`. Groq does not support streaming with structured
    /// outputs, so this is required rather than chosen.
    stream: bool,
    /// Groq converts `0` to `1e-8` server-side. Sent to minimise variation,
    /// **not** to make the answer deterministic — it does not.
    temperature: f32,
    max_completion_tokens: u32,
}

/// One edge, as the request carries it.
///
/// A borrowed view over [`BundledEdge`] rather than a second evidence model:
/// the bundle is the one representation of derived evidence in the workspace,
/// and this names which of its fields cross the boundary. `BundledEdge` is not
/// itself `Serialize`, and leaving it that way keeps that choice explicit.
#[derive(Debug, Serialize)]
struct WireEvidence<'a> {
    /// The number a citation must use.
    edge: EdgeId,
    /// The source artefact's stable identity, `kind|name|file`.
    from: &'a str,
    /// The target artefact's stable identity.
    to: &'a str,
    kind: EdgeKind,
    /// The derived claim, byte for byte. A citation must quote this exactly.
    evidence: &'a str,
    provenance: Provenance,
    /// An uncalibrated prior, not a probability.
    confidence: Confidence,
    /// Repository-relative. `SourceLocation` refuses an absolute path at
    /// construction, so no machine layout can reach here.
    location: &'a SourceLocation,
}

impl<'a> From<&'a BundledEdge> for WireEvidence<'a> {
    fn from(edge: &'a BundledEdge) -> Self {
        Self {
            edge: edge.edge,
            from: edge.from.as_str(),
            to: edge.to.as_str(),
            kind: edge.kind,
            evidence: &edge.evidence,
            provenance: edge.provenance,
            confidence: edge.confidence,
            location: &edge.location,
        }
    }
}

/// The user message's content: the question, and the evidence to explain.
///
/// Serialised as JSON rather than prose so that the boundary between the
/// question and the evidence is unambiguous to the model and to a test, and so
/// that evidence text is escaped by a serialiser rather than by a format
/// string. **These two fields are the entire payload.**
#[derive(Debug, Serialize)]
pub struct UserContent<'a> {
    /// The user's question, byte for byte as they asked it.
    question: &'a str,
    /// The bundle's edges, in the bundle's own deterministic order.
    evidence: Vec<WireEvidence<'a>>,
}

/// The schema the reply must satisfy.
///
/// Strict mode requires every property to be listed in `required` and every
/// object to set `additionalProperties: false`; both are honoured here.
///
/// `edge` is an **integer**, not a string: `EdgeId` is `#[serde(transparent)]`
/// over a `u64`, so an integer is what the parser accepts and what a citation
/// round-trips as.
fn answer_schema() -> serde_json::Value {
    serde_json::json!({
        "type": "object",
        "properties": {
            "items": {
                "type": "array",
                "description": "One entry per claim. Every entry must cite.",
                "items": {
                    "type": "object",
                    "properties": {
                        "text": {
                            "type": "string",
                            "description": "One claim about the supplied evidence."
                        },
                        "citations": {
                            "type": "array",
                            "description": "At least one. An uncited claim is rejected.",
                            "items": {
                                "type": "object",
                                "properties": {
                                    "edge": {
                                        "type": "integer",
                                        "description": "The `edge` number of a supplied edge."
                                    },
                                    "text": {
                                        "type": "string",
                                        "description": "An exact substring of that edge's evidence."
                                    }
                                },
                                "required": ["edge", "text"],
                                "additionalProperties": false
                            }
                        }
                    },
                    "required": ["text", "citations"],
                    "additionalProperties": false
                }
            }
        },
        "required": ["items"],
        "additionalProperties": false
    })
}

/// Builds the exact bytes of one chat-completions request.
///
/// The credential is **not** a parameter: it belongs to an `Authorization`
/// header, and there is no transport in this module to write one. Nothing here
/// can put a secret in a body.
///
/// # Errors
///
/// [`ProviderError::MalformedRequest`] if the body could not be serialised.
/// Every field is a string, a number or a fieldless enum, so this is not
/// reachable in practice; it is returned rather than panicked because a library
/// that panics on a serialiser bug takes its caller down with it.
pub fn request_body(
    bundle: &EvidenceBundle,
    question: &Question,
) -> Result<Vec<u8>, ProviderError> {
    let content = user_content(bundle, question);
    let rendered = serde_json::to_string(&content).map_err(|_| ProviderError::MalformedRequest)?;

    let request = ChatRequest {
        model: MODEL,
        messages: vec![
            Message {
                role: "system",
                content: SYSTEM_INSTRUCTION,
            },
            Message {
                role: "user",
                content: &rendered,
            },
        ],
        response_format: ResponseFormat {
            kind: "json_schema",
            json_schema: JsonSchema {
                name: SCHEMA_NAME,
                strict: true,
                schema: answer_schema(),
            },
        },
        stream: false,
        temperature: 0.0,
        max_completion_tokens: MAX_COMPLETION_TOKENS,
    };

    serde_json::to_vec(&request).map_err(|_| ProviderError::MalformedRequest)
}

/// The payload, without the envelope — the shape the privacy tests inspect.
#[must_use]
pub fn user_content<'a>(bundle: &'a EvidenceBundle, question: &'a Question) -> UserContent<'a> {
    UserContent {
        question: question.text(),
        evidence: bundle.edges().iter().map(WireEvidence::from).collect(),
    }
}

// ---------------------------------------------------------------------------
// Response
// ---------------------------------------------------------------------------

/// The answer schema, exactly as it arrives.
///
/// `deny_unknown_fields` because this document is produced under a strict
/// schema: a field that is not in the schema means the reply is not the thing
/// that was asked for, and accepting it quietly would hide that.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct WireAnswer {
    items: Vec<WireItem>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct WireItem {
    text: String,
    citations: Vec<WireCitation>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct WireCitation {
    edge: EdgeId,
    text: String,
}

/// The chat-completions envelope, modelled only as far as the content.
///
/// Unknown fields are **allowed** here, unlike in the answer: `id`, `usage`,
/// `system_fingerprint` and whatever the API adds next are not this
/// repository's business, and failing on them would break the client every time
/// Groq added a field.
#[derive(Debug, Clone, Deserialize)]
struct ChatCompletion {
    choices: Vec<Choice>,
}

#[derive(Debug, Clone, Deserialize)]
struct Choice {
    message: ChoiceMessage,
}

#[derive(Debug, Clone, Deserialize)]
struct ChoiceMessage {
    content: String,
}

/// An error reply, reduced to the one field that is safe to keep.
///
/// Groq returns `{"error": {"message": ..., "type": ...}}`. **`message` is
/// deliberately not modelled.** It is a response body, response bodies are
/// never logged (ADR-0021 Amendment 3, decision C8), and the surest way to
/// keep one out of a log is to never hold it. `type` is a fixed API
/// classification and carries nothing about the request.
#[derive(Debug, Clone, Deserialize)]
pub struct ApiError {
    error: ApiErrorBody,
}

#[derive(Debug, Clone, Deserialize)]
struct ApiErrorBody {
    #[serde(rename = "type")]
    kind: String,
}

impl ApiError {
    /// The API's own classification, exactly as it arrived.
    ///
    /// Server-controlled text. Prefer [`safe_kind`](Self::safe_kind) anywhere
    /// the value might reach an error or a log.
    #[must_use]
    pub fn kind(&self) -> &str {
        &self.error.kind
    }

    /// The classification, but only if it looks like one.
    ///
    /// `type` is chosen by whoever answered, and an error built from it can
    /// reach a log — so a server that put a paragraph, a credential or a
    /// machine path there would have turned a classification field into the
    /// response-body channel decision C8 exists to close. Anything longer than
    /// [`MAX_ERROR_KIND_BYTES`] or outside `[a-z0-9_]` is therefore dropped
    /// rather than trusted.
    ///
    /// Groq's own values — `invalid_request_error` and its siblings — pass
    /// unchanged.
    #[must_use]
    pub fn safe_kind(&self) -> Option<&str> {
        let kind = self.error.kind.as_str();
        let plausible = !kind.is_empty()
            && kind.len() <= MAX_ERROR_KIND_BYTES
            && kind
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_');
        plausible.then_some(kind)
    }
}

/// Reads an error reply, keeping only its classification.
///
/// # Errors
///
/// [`ProviderError::InvalidResponse`] if the body is not an error document.
pub fn parse_error(body: &[u8]) -> Result<ApiError, ProviderError> {
    serde_json::from_slice(body).map_err(|_| ProviderError::InvalidResponse {
        fault: ResponseFault::NotTheSchema,
    })
}

/// Reads a successful reply into the **unvalidated** claim it contains.
///
/// Returns a [`ClaimedAnswer`] and never an [`Answer`]: parsing establishes
/// shape, and [`validate`] establishes truth. The
/// two are separate on purpose, and a test proves a reply can parse cleanly and
/// still be rejected.
///
/// # Errors
///
/// [`ProviderError::InvalidResponse`], carrying which of the three things went
/// wrong — the body was not JSON, the envelope carried no choice, or the
/// document was not the schema that was asked for.
///
/// The underlying `serde_json` error is **discarded rather than wrapped**. Its
/// message quotes the input, and a response body must never reach a log; the
/// cost is a coarser diagnostic, and that is the trade decision C8 requires.
pub fn parse_answer(body: &[u8]) -> Result<ClaimedAnswer, ProviderError> {
    let completion: ChatCompletion =
        serde_json::from_slice(body).map_err(|_| ProviderError::InvalidResponse {
            fault: ResponseFault::NotJson,
        })?;

    let choice = completion
        .choices
        .first()
        .ok_or(ProviderError::InvalidResponse {
            fault: ResponseFault::NoChoices,
        })?;

    parse_answer_content(&choice.message.content)
}

/// Reads just the structured document a choice's `content` holds.
///
/// Separated from [`parse_answer`] so the answer schema can be exercised
/// without building an envelope around it.
///
/// # Errors
///
/// [`ProviderError::InvalidResponse`] with [`ResponseFault::NotTheSchema`].
pub fn parse_answer_content(content: &str) -> Result<ClaimedAnswer, ProviderError> {
    let answer: WireAnswer =
        serde_json::from_str(content).map_err(|_| ProviderError::InvalidResponse {
            fault: ResponseFault::NotTheSchema,
        })?;

    Ok(ClaimedAnswer {
        items: answer
            .items
            .into_iter()
            .map(|item| ClaimedItem {
                text: item.text,
                citations: item
                    .citations
                    .into_iter()
                    .map(|citation| ClaimedCitation {
                        edge: citation.edge,
                        text: citation.text,
                    })
                    .collect(),
            })
            .collect(),
    })
}

// ---------------------------------------------------------------------------
// The provider
// ---------------------------------------------------------------------------

/// Where the request goes. Owned by this module, and not configurable.
///
/// There is deliberately no way for a caller to change the endpoint, the model,
/// the schema or the streaming flag: those are the decision ADR-0021 Amendment
/// 3 recorded, not a preference a caller expresses. A configurable endpoint on
/// an authenticated request is also a credential-forwarding hazard — point it
/// somewhere else and the key goes with it.
pub const ENDPOINT: &str = "https://api.groq.com/openai/v1/chat/completions";

/// How much of a provider's error classification is worth keeping.
///
/// Groq's `type` values are short `snake_case` identifiers. A cap plus a shape
/// check means a server cannot use the field as a channel for arbitrary text
/// that would then reach a log — see [`ApiError::safe_kind`].
///
/// Public because it is part of that method's observable behaviour: a caller
/// deciding whether to trust a classification should be able to see the bound
/// it was checked against.
pub const MAX_ERROR_KIND_BYTES: usize = 64;

/// Explains derived evidence by asking Groq, and checks every word of the
/// answer before returning it.
///
/// # What it owns, and what it borrows
///
/// It owns a [`Transport`] and **nothing else**. In particular it never holds a
/// credential: [`Provider::explain`] takes one by reference, the header value
/// is built inside the smallest possible block, and that block ends before the
/// reply is even parsed. There is no field a key could be stored in, so
/// "forgot to clear the key" is not a mistake this type can make.
///
/// # The whole flow
///
/// ```text
/// bundle + question  → request_body → exact bytes
///                                   ↓
///                    Authorization: Bearer <secret>   (built, used, dropped)
///                                   ↓
///                          Transport::post_json
///                                   ↓
///          2xx → parse_answer → ClaimedAnswer → validate → Answer
///        non-2xx → parse_error → status + classification only
/// ```
///
/// The last line of that diagram is the point: **nothing produces an [`Answer`]
/// except [`validate`], against the bundle that was actually sent.** A model
/// that returns beautifully-formed JSON quoting text no edge contains gets a
/// refusal, not a rendering.
#[derive(Debug, Clone, Copy)]
pub struct GroqProvider<T> {
    transport: T,
}

impl<T: Transport> GroqProvider<T> {
    /// Wraps a transport.
    ///
    /// No credential, no configuration, no options. Everything else this
    /// provider needs is a constant in this module.
    pub const fn new(transport: T) -> Self {
        Self { transport }
    }

    /// The transport, for a caller that needs to inspect it after a call.
    pub const fn transport(&self) -> &T {
        &self.transport
    }
}

/// Turns a refusal into an error that remembers only what is safe.
///
/// The status always; the provider's classification when the body offered a
/// plausible one. **Never the message, never the body.**
fn refusal(response: &HttpResponse) -> ProviderError {
    ProviderError::Refused {
        status: response.status(),
        kind: parse_error(response.body())
            .ok()
            .and_then(|error| error.safe_kind().map(ToOwned::to_owned)),
    }
}

impl<T: Transport> Provider for GroqProvider<T> {
    fn explain(
        &self,
        bundle: &EvidenceBundle,
        question: &Question,
        credential: &Secret,
    ) -> Result<Answer, ProviderError> {
        let body = request_body(bundle, question)?;

        // The credential exists inside this block and nowhere else. It is built
        // after the body (so a serialisation failure never constructs one) and
        // dropped before the reply is parsed, so no code that handles a
        // provider's output is in scope with a key.
        //
        // Rust does not zero the buffer on drop, and this does not pretend
        // otherwise; what it does buy is that the value has no path out of this
        // block -- no field holds it, and nothing returned from here borrows it.
        let response = {
            let authorization = format!("Bearer {}", credential.expose());
            let headers = [
                Header::new("Authorization", &authorization),
                Header::new("Content-Type", "application/json"),
            ];
            self.transport.post_json(ENDPOINT, &headers, &body)
        }
        .map_err(|cause| ProviderError::Transport { cause })?;

        if !response.is_success() {
            return Err(refusal(&response));
        }

        // Parsed, then checked. The two are separate on purpose: a reply that
        // parses has the right shape, and shape is not truth.
        let claimed = parse_answer(response.body())?;
        validate(bundle, claimed)
    }
}
