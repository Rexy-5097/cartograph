//! The one place in Cartograph that can open a socket.
//!
//! # Why this is the only such place
//!
//! [ADR-0021](../../../docs/adr/ADR-0021-ask-boundary-and-citation-contract.md)
//! Amendment 3, decision C3, puts the HTTP client in `cartograph-ask` and in no
//! other crate, so that `cartograph-cli` and `cartograph-mcp` remain incapable
//! of network egress **by their dependency graph** rather than by anyone
//! remembering. This module is what that decision was protecting: `ureq` is
//! reachable from here and from nowhere else in the workspace.
//!
//! # It knows nothing about any provider
//!
//! No Groq, no chat completions, no model, no `EvidenceBundle`, no
//! `RepositoryIdentity`, no credential store. A URL, some headers, some request
//! bytes, and a status with some response bytes. Everything provider-shaped
//! lives in [`crate::groq`], which reaches this type only through the
//! [`Transport`] trait.
//!
//! # The four settings that are not defaults
//!
//! | Setting | Value | Why |
//! |---|---|---|
//! | `https_only` | `true` | A plaintext request carrying `Authorization` is a credential in the clear. Refused before a socket is opened |
//! | `max_redirects` | `0` | A redirect on an authenticated request is a credential-forwarding hazard, and there is no legitimate redirect on this endpoint. At zero, `ureq` **returns** the 3xx rather than erroring, which is what the [`Transport`] contract asks for |
//! | `http_status_as_error` | `false` | **Load-bearing.** `ureq` otherwise turns a non-2xx into `Error::StatusCode` and discards the body — and Groq puts its error classification *in* that body, which [`crate::groq::parse_error`] reads |
//! | `timeout_global` | 60s | A request that never returns is a desktop that never repaints. The duration is a private constant, not a knob |
//!
//! The first three are named by decision C2. The timeout's *duration* is not:
//! C2 requires "a global timeout" and leaves the number open, so it is a
//! private constant here rather than a configuration surface.
//!
//! # Defaults inherited deliberately
//!
//! Reviewed rather than accepted by omission. None of these is changed, and the
//! reason is recorded for each:
//!
//! - **TLS roots — `webpki-roots`.** Bundled Mozilla trust anchors, identical
//!   on all three desktop targets. The cost, recorded in C2, is that they are
//!   updated by a Cartograph release rather than by the operating system.
//! - **Crypto provider — `ring`.** `rustls` is **not** pure Rust with this
//!   provider: `ring` contains C and assembly. The accurate claim, and the only
//!   one made, is that **`rustls` avoids reliance on the system TLS library** —
//!   no `schannel`, no Secure Transport, no OpenSSL.
//! - **Proxy from the environment.** `ureq` honours `HTTPS_PROXY` and friends.
//!   Left on: a user behind a corporate proxy cannot reach the internet without
//!   it, and the connection is still TLS end to end through a `CONNECT` tunnel,
//!   so the proxy sees a hostname and not a request. Disabling it would break
//!   legitimate networks to protect nothing.
//! - **`gzip` response decompression.** Saves bytes and changes no semantics;
//!   the body this module returns is the decompressed one, which is what a
//!   parser wants.
//! - **Protocol headers.** `ureq` sets `Host`, `Content-Length`, `User-Agent`
//!   and `Accept-Encoding` itself, as any HTTP client must. They carry nothing
//!   about the user, and the [`Transport`] contract's "add no header of your
//!   own" is about headers a *caller* would have opinions about.
//! - **10 MB body read limit.** `ureq`'s own guard against a server that
//!   answers forever. An ASK reply is a few kilobytes, so this is a ceiling
//!   rather than a budget.
//!
//! # Nothing here is logged
//!
//! Not the URL, not a header, not a request body, not a response body. The
//! module contains no `tracing` call and no `println!` at all, which is a
//! stronger guarantee than redacting one — and every `ureq` error is mapped to
//! a category, **discarding its payload**. That matters more than it looks:
//! `Error::BadUri` and `Error::RequireHttpsOnly` both carry the URL as a
//! `String`, and a URL can carry a query string.

use std::time::Duration;

use ureq::Agent;
use ureq::config::Config;

use crate::transport::{Header, HttpResponse, Transport, TransportError};

/// How long one exchange may take, in total.
///
/// Deliberately not configurable. Decision C2 requires "a global timeout" and
/// does not name a duration, so this is an implementation detail rather than a
/// decision, and a private constant rather than a knob. Sixty seconds is
/// roughly an order of magnitude more than a 2048-token completion needs at the
/// model's published speed, which leaves room for queueing without leaving a
/// user staring at a frozen panel.
const TIMEOUT: Duration = Duration::from_secs(60);

/// The agent's configuration, in one place so a test can read the same values
/// the production agent is built from.
fn config() -> Config {
    Agent::config_builder()
        // A plaintext request carrying `Authorization` is a credential in the
        // clear. Refused before a socket is opened.
        .https_only(true)
        // No redirect is followed. At zero, `ureq` returns the 3xx response
        // rather than raising `TooManyRedirects`, which is exactly what the
        // `Transport` contract asks for.
        .max_redirects(0)
        // Load-bearing: otherwise a non-2xx becomes an error and its body is
        // discarded, and the body is where a provider's error classification
        // lives.
        .http_status_as_error(false)
        .timeout_global(Some(TIMEOUT))
        .build()
}

/// A [`Transport`] over `ureq`, with TLS by `rustls`.
///
/// Holds one [`Agent`], which pools connections across calls. Holds no
/// credential, no URL and nothing about a provider: everything it needs for a
/// request arrives as arguments.
#[derive(Debug, Clone)]
pub struct UreqTransport {
    agent: Agent,
}

impl UreqTransport {
    /// Builds a transport with the configuration above.
    ///
    /// There is deliberately no constructor that takes options. The four
    /// settings that matter are decisions, not preferences, and a caller that
    /// could relax `https_only` could send a credential in the clear.
    #[must_use]
    pub fn new() -> Self {
        Self {
            agent: Agent::new_with_config(config()),
        }
    }
}

impl Default for UreqTransport {
    fn default() -> Self {
        Self::new()
    }
}

/// Maps a `ureq` failure onto the transport's five categories.
///
/// **Every payload is discarded.** `BadUri` and `RequireHttpsOnly` carry the
/// URL, `Io` carries an OS error, `Rustls` carries a TLS error — and any of
/// them could end up in a log if it were carried along. The category is what a
/// caller can act on; the detail is what must not travel. An `Io` error's
/// *kind* is read before it is dropped, because that is the one piece of it
/// that changes which category is correct.
///
/// `ureq::Error` is `#[non_exhaustive]`, and some of its variants exist only
/// behind features this workspace does not enable — `NativeTls` and `Der` are
/// not in scope here, which is itself a small confirmation that `native-tls` is
/// absent. The final arm is therefore a catch-all rather than a promise that
/// the list is complete.
fn categorise(error: &ureq::Error) -> TransportError {
    match error {
        // The request could not be formed or was refused before it was sent.
        ureq::Error::BadUri(_)
        | ureq::Error::RequireHttpsOnly(_)
        | ureq::Error::Http(_)
        | ureq::Error::BodyExceedsLimit(_)
        | ureq::Error::RedirectFailed
        | ureq::Error::TooManyRedirects
        | ureq::Error::InvalidProxyUrl => TransportError::Request,

        // The other end could not be reached, or TLS to it could not be
        // established.
        ureq::Error::ConnectionFailed
        | ureq::Error::HostNotFound
        | ureq::Error::ConnectProxyFailed(_)
        | ureq::Error::Tls(_)
        | ureq::Error::TlsRequired
        | ureq::Error::Rustls(_)
        | ureq::Error::Pem(_) => TransportError::Connect,

        ureq::Error::Timeout(_) => TransportError::Timeout,

        // A status that reached here means `http_status_as_error` was somehow
        // true. It is mapped faithfully rather than hidden, and the body is
        // gone by then -- which is why the setting above is load-bearing.
        ureq::Error::StatusCode(status) => TransportError::Status { status: *status },

        // `Io` is not one thing, and treating it as one gets the commonest
        // failure of all wrong. A refused connection arrives here as
        // `Io(ConnectionRefused)` rather than as `ConnectionFailed`, which
        // `ureq` reserves for a connector chain that gave up -- so mapping the
        // whole variant to `Read` would tell a user their provider replied
        // badly when in fact it was never reached.
        ureq::Error::Io(io) => match io.kind() {
            std::io::ErrorKind::ConnectionRefused
            | std::io::ErrorKind::ConnectionAborted
            | std::io::ErrorKind::NotConnected
            | std::io::ErrorKind::AddrNotAvailable
            | std::io::ErrorKind::HostUnreachable
            | std::io::ErrorKind::NetworkUnreachable
            | std::io::ErrorKind::NetworkDown => TransportError::Connect,
            std::io::ErrorKind::TimedOut => TransportError::Timeout,
            // A reply began and could not be read to the end.
            _ => TransportError::Read,
        },

        // A reply began and could not be read to the end.
        _ => TransportError::Read,
    }
}

impl Transport for UreqTransport {
    fn post_json(
        &self,
        url: &str,
        headers: &[Header<'_>],
        body: &[u8],
    ) -> Result<HttpResponse, TransportError> {
        let mut request = self.agent.post(url);
        for header in headers {
            request = request.header(header.name(), header.value());
        }

        // `body` is handed over as bytes. Nothing parses it, re-serialises it
        // or converts it through `String`: the caller already checked these
        // exact bytes, and `ureq`'s `json` feature is off so that it stays that
        // way (decision C2).
        let mut response = request.send(body).map_err(|error| categorise(&error))?;

        let status = response.status().as_u16();
        // Any status, including 4xx and 5xx -- the body of a refusal is where a
        // provider's error classification lives.
        let bytes = response
            .body_mut()
            .read_to_vec()
            .map_err(|error| categorise(&error))?;

        Ok(HttpResponse::new(status, bytes))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The four settings that are not defaults, read from the same function the
    /// production agent is built from.
    ///
    /// Asserted here rather than in an integration test because the values are
    /// visible without a server, and the behaviours they produce are not.
    #[test]
    fn the_agent_requires_https() {
        assert!(config().https_only(), "a credential could go out in clear");
    }

    #[test]
    fn the_agent_follows_no_redirects() {
        assert_eq!(
            config().max_redirects(),
            0,
            "a redirect would forward the Authorization header"
        );
    }

    #[test]
    fn a_status_is_not_an_error() {
        // The setting `groq::parse_error` depends on: with this true, ureq
        // would discard the body a refusal carries.
        assert!(
            !config().http_status_as_error(),
            "an error body would be discarded before it could be classified"
        );
    }

    #[test]
    fn the_exchange_is_bounded() {
        assert_eq!(config().timeouts().global, Some(TIMEOUT));
        assert_eq!(TIMEOUT, Duration::from_secs(60));
    }

    #[test]
    fn a_plaintext_url_is_refused_without_opening_a_socket() {
        // `https_only` is enforced by the client before it connects, so this
        // runs offline and deterministically -- there is no host to reach.
        let transport = UreqTransport::new();

        let refused = transport.post_json("http://example.invalid/path", &[], b"{}");

        assert_eq!(refused, Err(TransportError::Request));
    }

    #[test]
    fn a_malformed_url_is_refused_without_opening_a_socket() {
        let transport = UreqTransport::new();

        for bad in ["not a url", "", "https://", "ftp://example.invalid/"] {
            assert_eq!(
                transport.post_json(bad, &[], b"{}"),
                Err(TransportError::Request),
                "accepted {bad:?}"
            );
        }
    }

    #[test]
    fn an_unreachable_host_maps_to_connect() {
        // Loopback, port 1, by address rather than name: no DNS, no external
        // network, and an immediate refusal from the OS. Whether it is refused
        // at TCP or at TLS, both land on `Connect`, so the outcome does not
        // depend on what is or is not listening.
        let transport = UreqTransport::new();

        let error = transport
            .post_json("https://127.0.0.1:1/", &[], b"{}")
            .expect_err("nothing serves port 1");

        assert_eq!(error, TransportError::Connect);
    }

    #[test]
    fn a_refusal_carries_neither_the_url_nor_a_header_value() {
        // The specific hazard: `RequireHttpsOnly` and `BadUri` both carry the
        // URL as a String, and a URL can carry a query string.
        let transport = UreqTransport::new();
        let secret = format!("{}{}", "gsk_", "abcdefghijklmnopqrstuvwxyz0123456789");
        let url = format!("http://example.invalid/path?key={secret}");
        let authorization = format!("Bearer {secret}");

        let error = transport
            .post_json(&url, &[Header::new("Authorization", &authorization)], b"{}")
            .expect_err("plaintext is refused");

        let rendered = format!("{error} {error:?}");
        assert!(!rendered.contains(&secret), "the error leaked: {rendered}");
        assert!(
            !rendered.contains("example.invalid"),
            "the error leaked a URL"
        );
        assert!(!rendered.contains("Bearer"), "the error leaked a header");
    }
}
