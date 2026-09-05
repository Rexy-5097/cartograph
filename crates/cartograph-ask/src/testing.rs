//! Transports for tests: one that answers from a script, one that refuses to
//! exist.
//!
//! # Why these ship in the library rather than in a test file
//!
//! [`UnreachableTransport`] is the mechanism ADR-0021 Amendment 3, decision
//! C10, relies on: *"no network when AI is disabled"* becomes a test failure
//! rather than a reviewer's observation. The tests that most need it are the
//! degraded-mode tests, and those belong to the crate that owns the opt-in and
//! the credential — `cartograph-desktop` — which cannot reach into this
//! crate's `#[cfg(test)]`.
//!
//! So it is a public module, and the cost is honest: two test doubles in the
//! public API. The alternative is every downstream crate writing its own
//! "transport that must not be called", which would make the guarantee a thing
//! each caller re-implements instead of a thing this crate provides.
//!
//! Neither type opens a socket. Neither can: nothing in this crate has an HTTP
//! client to open one with.
//!
//! # What is recorded, and what is not
//!
//! [`MockTransport`] records the URL, the **header names**, and the request
//! body. It does **not** record header values, and there is no method that
//! returns one. That is not an oversight to be fixed when a test wants it: the
//! header that will eventually be there is `Authorization: Bearer …`, and a
//! recorder that kept it would put a credential in a test fixture's memory, in
//! an assertion message, and eventually in CI output.
//!
//! A test can prove the header *was sent* — [`RecordedRequest::has_header`] —
//! without anything ever holding what it said.

use std::sync::Mutex;

use crate::transport::{Header, HttpResponse, Transport, TransportError};

/// What a [`MockTransport`] saw, minus anything it must not remember.
#[derive(Clone, PartialEq, Eq)]
pub struct RecordedRequest {
    url: String,
    header_names: Vec<String>,
    body: Vec<u8>,
}

impl RecordedRequest {
    /// Where the request was addressed.
    #[must_use]
    pub fn url(&self) -> &str {
        &self.url
    }

    /// The names of the headers that were sent, in order. Never their values.
    #[must_use]
    pub fn header_names(&self) -> &[String] {
        &self.header_names
    }

    /// Whether a header was sent, matched case-insensitively as HTTP requires.
    ///
    /// The way to assert that a credential was attached without holding one.
    #[must_use]
    pub fn has_header(&self, name: &str) -> bool {
        self.header_names
            .iter()
            .any(|sent| sent.eq_ignore_ascii_case(name))
    }

    /// The request body, byte for byte as it was handed to the transport.
    #[must_use]
    pub fn body(&self) -> &[u8] {
        &self.body
    }
}

impl std::fmt::Debug for RecordedRequest {
    /// Never the body, and never a header value.
    ///
    /// The body is not secret — it is a question and derived evidence — but a
    /// `Debug` that printed it would put a request payload into every panic
    /// message that happened to hold one, and the habit is worth more than the
    /// convenience. A test that wants the bytes asserts on [`Self::body`].
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "RecordedRequest {{ url: {:?}, headers: {:?}, body: {} bytes }}",
            self.url,
            self.header_names,
            self.body.len()
        )
    }
}

/// A transport that answers from a script and remembers what it was asked.
///
/// Deterministic: the same reply for every call, so a test never depends on
/// call ordering it did not set up.
#[derive(Debug)]
pub struct MockTransport {
    reply: Result<HttpResponse, TransportError>,
    calls: Mutex<Vec<RecordedRequest>>,
}

impl MockTransport {
    /// Answers every call with this status and body.
    #[must_use]
    pub fn replying(status: u16, body: impl Into<Vec<u8>>) -> Self {
        Self {
            reply: Ok(HttpResponse::new(status, body)),
            calls: Mutex::new(Vec::new()),
        }
    }

    /// Fails every call the same way.
    #[must_use]
    pub fn failing(error: TransportError) -> Self {
        Self {
            reply: Err(error),
            calls: Mutex::new(Vec::new()),
        }
    }

    /// Every request this transport was given, in order.
    ///
    /// # Panics
    ///
    /// If a previous call panicked while recording, poisoning the lock. In a
    /// test that is the honest outcome: the recording is untrustworthy.
    #[must_use]
    pub fn calls(&self) -> Vec<RecordedRequest> {
        self.calls
            .lock()
            .expect("recording was not poisoned")
            .clone()
    }

    /// How many times it was called.
    ///
    /// # Panics
    ///
    /// See [`Self::calls`].
    #[must_use]
    pub fn call_count(&self) -> usize {
        self.calls.lock().expect("recording was not poisoned").len()
    }

    /// The only request, for the common case of asserting about one call.
    ///
    /// # Panics
    ///
    /// If it was not called exactly once — which is itself the assertion most
    /// tests want, and failing loudly beats returning `None` and being ignored.
    #[must_use]
    pub fn only_call(&self) -> RecordedRequest {
        let calls = self.calls();
        assert_eq!(calls.len(), 1, "expected exactly one request");
        calls.into_iter().next().expect("just checked the length")
    }
}

impl Transport for MockTransport {
    fn post_json(
        &self,
        url: &str,
        headers: &[Header<'_>],
        body: &[u8],
    ) -> Result<HttpResponse, TransportError> {
        self.calls
            .lock()
            .expect("recording was not poisoned")
            .push(RecordedRequest {
                url: url.to_owned(),
                header_names: headers.iter().map(|h| h.name().to_owned()).collect(),
                // Copied verbatim. Nothing here parses, reformats or
                // round-trips the bytes, so a test comparing them to what was
                // sent is comparing the real thing.
                body: body.to_vec(),
            });
        self.reply.clone()
    }
}

/// A transport that must never be reached, and fails the test if it is.
///
/// The mechanism behind decision C10's degraded-mode guarantees. Wire this in
/// wherever the rule is *"this path does not touch the network"* — AI disabled,
/// no opt-in, no credential — and the rule stops being a claim in a comment.
///
/// It panics rather than returning an error on purpose. An error would be
/// handled by the very degradation logic under test, and the test would pass
/// while the call happened.
#[derive(Debug, Clone, Copy, Default)]
pub struct UnreachableTransport;

impl Transport for UnreachableTransport {
    /// # Panics
    ///
    /// Always. Reaching this line is the failure.
    fn post_json(
        &self,
        _url: &str,
        _headers: &[Header<'_>],
        _body: &[u8],
    ) -> Result<HttpResponse, TransportError> {
        panic!("the network was reached on a path that must never reach it");
    }
}
