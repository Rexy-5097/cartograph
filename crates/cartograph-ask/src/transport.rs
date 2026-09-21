//! The seam a real HTTP client will be dropped into — and nothing more.
//!
//! # Why a seam before a client
//!
//! The parts of a provider that carry the security guarantees are the parts
//! that have nothing to do with sockets: which bytes are sent, which headers
//! are attached, and what an error is allowed to remember. Putting an
//! interface here first makes all three testable **offline and permanently**,
//! and leaves the eventual `ureq` step with nothing to prove except that it
//! moves bytes.
//!
//! It also buys the thing ADR-0021 Amendment 3, decision C10, needs: a
//! transport that **panics if it is called** turns *"no network when AI is
//! disabled"* from a claim into a test failure. See
//! [`UnreachableTransport`](crate::testing::UnreachableTransport).
//!
//! # This module knows nothing about any provider
//!
//! No Groq, no chat completions, no `EvidenceBundle`, no `Citation`, no
//! `RepositoryIdentity`, no credential store. A URL, some headers, some request
//! bytes, and a status with some response bytes. That is the whole vocabulary,
//! and keeping it that small is what stops this becoming a framework.
//!
//! # Cartograph owns the bytes, and the seam preserves them
//!
//! [`crate::groq::request_body`] already produced the exact `Vec<u8>` the
//! privacy tests inspect. This trait takes `&[u8]` and hands it on unchanged:
//! **no re-serialisation, no whitespace normalisation, no key reordering, no
//! UTF-8 round trip.** A transport that parsed and re-emitted the JSON would
//! silently invalidate every byte-level assertion made about the request, which
//! is exactly why decision C2 keeps `ureq`'s `json` feature off.
//!
//! # A non-2xx reply is a reply, not a transport failure
//!
//! This is a deliberate choice, and it constrains the implementation that
//! arrives later. `ureq` surfaces a non-2xx status as `Error::StatusCode` **by
//! default**, discarding the body with it — and Groq puts its error
//! classification *in* that body, which [`crate::groq::parse_error`] exists to
//! read. So [`Transport::post_json`] returns `Ok` whenever the server replied
//! at all, and [`TransportError`] covers only the cases where there was no
//! reply to read.
//!
//! **The implementation must therefore set `http_status_as_error(false)`.**
//! Recorded here rather than left to be rediscovered, because the default
//! behaviour would quietly make a merged function unreachable.
//!
//! A caller that does not care about an error body can still have the simple
//! shape, through [`HttpResponse::require_success`].
//!
//! # What an error may remember
//!
//! A status code and a category. **Never a body, never a header, never a URL.**
//! A response body must not reach a log (decision C8), and the surest way to
//! keep one out is to never put it in the error in the first place — the same
//! reasoning that makes [`crate::groq::parse_answer`] discard `serde`'s
//! body-quoting message rather than wrap it.

use std::fmt;

/// One request header, opaque to the transport.
///
/// # Why the value never prints
///
/// This type is what will carry `Authorization: Bearer …` when the provider
/// arrives. It does not carry one yet, and the guard is installed now anyway:
/// a header value that could be printed is a credential that will eventually be
/// printed, and the correct time to prevent that is before the credential
/// exists. `Debug` shows the name and a placeholder — the treatment
/// [`Secret`](cartograph_core::Secret) and `RepositoryIdentity` already get.
///
/// There is deliberately no `Display`, no `Serialize`, and no accessor that
/// hands out an owned value.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct Header<'a> {
    name: &'a str,
    value: &'a str,
}

impl<'a> Header<'a> {
    /// Names a header and its value.
    #[must_use]
    pub const fn new(name: &'a str, value: &'a str) -> Self {
        Self { name, value }
    }

    /// The header's name, which is not sensitive.
    #[must_use]
    pub const fn name(&self) -> &'a str {
        self.name
    }

    /// The header's value, for the one place that must send it.
    ///
    /// Named so a reader of the call site sees what is happening, and the only
    /// way out of this type.
    #[must_use]
    pub const fn value(&self) -> &'a str {
        self.value
    }
}

impl fmt::Debug for Header<'_> {
    /// The name, never the value.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Header {{ name: {:?}, value: {} }}",
            self.name,
            cartograph_core::REDACTED
        )
    }
}

/// What a server replied: a status, and the bytes that came with it.
///
/// Carries the body for **any** status, because a provider's error
/// classification lives in the body of a non-2xx reply.
#[derive(Clone, PartialEq, Eq)]
pub struct HttpResponse {
    status: u16,
    body: Vec<u8>,
}

impl HttpResponse {
    /// Records a reply.
    #[must_use]
    pub fn new(status: u16, body: impl Into<Vec<u8>>) -> Self {
        Self {
            status,
            body: body.into(),
        }
    }

    /// The HTTP status code.
    #[must_use]
    pub const fn status(&self) -> u16 {
        self.status
    }

    /// Whether the status is 2xx.
    #[must_use]
    pub const fn is_success(&self) -> bool {
        self.status >= 200 && self.status < 300
    }

    /// The reply's bytes, exactly as they arrived.
    #[must_use]
    pub fn body(&self) -> &[u8] {
        &self.body
    }

    /// Takes the reply's bytes.
    #[must_use]
    pub fn into_body(self) -> Vec<u8> {
        self.body
    }

    /// Passes a 2xx reply through, and turns anything else into an error.
    ///
    /// For a caller that does not want to read a provider's error document.
    /// A caller that does — because the classification it needs is in there —
    /// simply does not call this and reads [`body`](Self::body) instead.
    ///
    /// # Errors
    ///
    /// [`TransportError::Status`], carrying the code and **not** the body.
    pub fn require_success(self) -> Result<Self, TransportError> {
        if self.is_success() {
            Ok(self)
        } else {
            Err(TransportError::Status {
                status: self.status,
            })
        }
    }
}

impl fmt::Debug for HttpResponse {
    /// Status and size. Never the body.
    ///
    /// A response body must never reach a log, and a `Debug` that printed one
    /// would put it there the first time anything held a response inside a
    /// larger structure.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "HttpResponse {{ status: {}, body: {} bytes }}",
            self.status,
            self.body.len()
        )
    }
}

/// Why bytes could not be exchanged.
///
/// The categories are chosen against `ureq`'s own error surface, which has
/// twenty-nine variants, so that the eventual implementation is a mapping
/// rather than a redesign:
///
/// | `ureq` | here |
/// |---|---|
/// | `BadUri`, `Http`, `BodyExceedsLimit`, `RedirectFailed`, `TooManyRedirects` | [`Request`](Self::Request) |
/// | `ConnectionFailed`, `HostNotFound`, `Tls`, `Rustls`, `NativeTls`, `TlsRequired` | [`Connect`](Self::Connect) |
/// | `Timeout` | [`Timeout`](Self::Timeout) |
/// | `Io`, `Protocol`, `LargeResponseHeader`, `Other` | [`Read`](Self::Read) |
/// | `StatusCode` | [`Status`](Self::Status), and only via [`HttpResponse::require_success`] |
///
/// Coarser than `ureq`'s on purpose. A caller can retry, degrade, or tell the
/// user the machine is offline; nothing it does depends on which of six TLS
/// failures occurred, and each extra variant is another thing that could be
/// tempted to carry a detail it should not.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum TransportError {
    /// The request could not be sent — a malformed URL, or a header the HTTP
    /// layer refused.
    Request,
    /// The host could not be reached: DNS, TCP or TLS.
    Connect,
    /// The exchange took too long.
    ///
    /// No timeout is configured at this step and none is exposed here: how long
    /// to wait is the implementation's business, and the seam only has to be
    /// able to *report* that it ran out. `ureq`'s `timeout_global` is where it
    /// will be set.
    Timeout,
    /// A reply began but could not be read to the end.
    Read,
    /// The server replied, and the status was not 2xx.
    ///
    /// Produced only by [`HttpResponse::require_success`], never by
    /// [`Transport::post_json`], because a non-2xx reply still has a body worth
    /// reading. Carries the code and **never** the body.
    Status {
        /// The HTTP status code.
        status: u16,
    },
}

impl fmt::Display for TransportError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Request => f.write_str("the request could not be sent"),
            Self::Connect => f.write_str("the provider could not be reached"),
            Self::Timeout => f.write_str("the provider did not answer in time"),
            Self::Read => f.write_str("the provider's reply could not be read"),
            Self::Status { status } => {
                write!(f, "the provider answered with status {status}")
            }
        }
    }
}

impl std::error::Error for TransportError {}

/// Sends bytes somewhere and reports what came back.
///
/// # Contract on an implementation
///
/// - **Send `body` unchanged.** Do not parse it, re-serialise it, reformat it
///   or convert it through `String`. The caller has already checked those exact
///   bytes.
/// - **Add no header of your own.** Every header the request carries is in
///   `headers`, `Content-Type: application/json` included. An implementation
///   that helpfully supplied one would be a header a caller's test never saw,
///   and the caller is the one that has to prove what it sent.
/// - **Return `Ok` for any reply**, including 4xx and 5xx. With `ureq` that
///   means configuring `http_status_as_error(false)`.
/// - **Never log** the body, the reply, or any header.
pub trait Transport {
    /// Posts `body` to `url` with `headers`.
    ///
    /// # Errors
    ///
    /// [`TransportError`] when there was no reply to read. A reply with an
    /// unwelcome status is `Ok`; see [`HttpResponse::require_success`].
    fn post_json(
        &self,
        url: &str,
        headers: &[Header<'_>],
        body: &[u8],
    ) -> Result<HttpResponse, TransportError>;
}
