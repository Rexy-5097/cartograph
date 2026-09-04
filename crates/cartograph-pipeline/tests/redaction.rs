//! Redaction, proved at the sink rather than at the call site.
//!
//! The unit tests in `cartograph_core::sensitive` establish what the rule
//! recognises. These establish the thing that actually matters: that a value
//! logged by ordinary code does not reach the writer.
//!
//! Every test installs a real `fmt` subscriber over an in-memory writer and
//! asserts on the bytes it produced. Nothing here performs a network request,
//! reads a credential, or needs one to exist — the sensitive values are
//! assembled in the test, which is exactly the situation Slice 5 will create
//! for real.

use std::io;
use std::sync::{Arc, Mutex};

use cartograph_pipeline::redaction::Redacting;
use tracing_subscriber::fmt::MakeWriter;

/// An in-memory sink, so a test can read what a subscriber wrote.
#[derive(Clone, Default)]
struct Sink(Arc<Mutex<Vec<u8>>>);

impl Sink {
    fn contents(&self) -> String {
        String::from_utf8(self.0.lock().expect("not poisoned").clone()).expect("utf-8")
    }
}

impl io::Write for Sink {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.0.lock().expect("not poisoned").extend_from_slice(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl<'a> MakeWriter<'a> for Sink {
    type Writer = Self;

    fn make_writer(&'a self) -> Self::Writer {
        self.clone()
    }
}

/// Runs `body` under a redacting subscriber and returns everything written.
fn logged(body: impl FnOnce()) -> String {
    let sink = Sink::default();
    let subscriber = tracing_subscriber::fmt()
        .with_writer(sink.clone())
        .fmt_fields(Redacting)
        .with_ansi(false)
        .with_target(false)
        .without_time()
        // Every level, or a leak at a level the default filter drops would
        // pass by producing no output at all.
        .with_max_level(tracing::Level::TRACE)
        .finish();

    tracing::subscriber::with_default(subscriber, body);
    sink.contents()
}

// Credential shapes are assembled at runtime: QG-005 scans tracked files for
// exactly these forms, and it is right to.
fn github_token() -> String {
    format!("{}{}", "ghp_", "abcdefghijklmnopqrstuvwxyz0123456789")
}

fn bearer() -> String {
    format!("Bearer {}", github_token())
}

// ── secrets ─────────────────────────────────────────────────────────

#[test]
fn a_token_in_a_structured_field_never_reaches_the_writer() {
    let token = github_token();
    let out = logged(|| tracing::error!(token = %token, "request failed"));

    assert!(!out.contains(&token), "the token reached the sink: {out}");
    assert!(out.contains("<redacted>"), "nothing was redacted: {out}");
    assert!(
        out.contains("request failed"),
        "the message was lost: {out}"
    );
}

#[test]
fn a_token_in_the_message_itself_never_reaches_the_writer() {
    // The message is a field like any other, so it is covered too.
    let token = github_token();
    let out = logged(|| tracing::warn!("authenticating with {}", token));

    assert!(!out.contains(&token), "the token reached the sink: {out}");
    assert!(out.contains("<redacted>"));
}

#[test]
fn an_authorization_header_value_never_reaches_the_writer() {
    // The shape Slice 5 will actually produce.
    let header = bearer();
    let out = logged(|| tracing::debug!(authorization = %header, "sending"));

    assert!(
        !out.contains("ghp_"),
        "the credential reached the sink: {out}"
    );
    assert!(out.contains("<redacted>"));
}

#[test]
fn a_token_logged_through_debug_never_reaches_the_writer() {
    // `record_debug` is the catch-all every other value type falls back to, so
    // a type nobody thought about is still covered.
    let token = github_token();
    let out = logged(|| tracing::info!(payload = ?vec![token.clone()], "sent"));

    assert!(!out.contains(&token), "Debug bypassed redaction: {out}");
}

#[test]
fn a_token_inside_an_error_never_reaches_the_writer() {
    let token = github_token();
    let failure = io::Error::other(format!("refused for {token}"));
    let out = logged(|| tracing::error!(error = %failure, "call failed"));

    assert!(!out.contains(&token), "an error bypassed redaction: {out}");
}

#[test]
fn a_token_in_a_span_field_never_reaches_the_writer() {
    let token = github_token();
    let out = logged(|| {
        let span = tracing::info_span!("request", key = %token);
        let _entered = span.enter();
        tracing::info!("inside");
    });

    assert!(
        !out.contains(&token),
        "a span field bypassed redaction: {out}"
    );
}

// ── machine paths ───────────────────────────────────────────────────

#[test]
fn a_home_path_never_reaches_the_writer() {
    let out = logged(|| tracing::info!(root = "/home/username/private-work", "analysing"));

    assert!(
        !out.contains("/home/"),
        "a machine path reached the sink: {out}"
    );
    assert!(
        !out.contains("private-work"),
        "a partly redacted path still names the directory: {out}"
    );
    assert!(out.contains("analysing"));
}

#[test]
fn a_drive_rooted_path_never_reaches_the_writer() {
    let out = logged(|| tracing::info!(root = r"C:\Users\username\work", "analysing"));

    assert!(!out.contains(r":\"), "a drive path reached the sink: {out}");
    assert!(out.contains("<redacted>"));
}

// ── what must survive ───────────────────────────────────────────────

#[test]
fn a_route_survives_untouched() {
    // The product's own vocabulary. If this were redacted the layer would be
    // switched off within a week, and then nothing would be redacted.
    let out = logged(|| tracing::info!(route = "GET /api/orders", "matched"));

    assert!(
        out.contains("GET /api/orders"),
        "a route was redacted: {out}"
    );
    assert!(!out.contains("<redacted>"));
}

#[test]
fn ordinary_evidence_survives_untouched() {
    let evidence = "Order.objects.all(...) in list_orders at api/routes.py:8";
    let out = logged(|| tracing::info!(evidence = evidence, "resolved"));

    assert!(out.contains(evidence), "evidence was altered: {out}");
}

#[test]
fn a_repository_relative_path_survives_untouched() {
    let out = logged(|| tracing::info!(file = "crates/cartograph-graph/src/bundle.rs", "parsed"));

    assert!(out.contains("crates/cartograph-graph/src/bundle.rs"));
    assert!(!out.contains("<redacted>"));
}

#[test]
fn field_names_and_numbers_are_left_alone() {
    let out = logged(|| tracing::info!(files = 663, duration_ms = 29, "analysed"));

    assert!(out.contains("files=663"), "a number was mangled: {out}");
    assert!(out.contains("duration_ms=29"));
    assert!(!out.contains("<redacted>"));
}

// ── the layer is the mechanism ──────────────────────────────────────

#[test]
fn the_default_formatter_would_have_leaked_it() {
    // The point of the slice, stated as a test: without this layer the same
    // call reaches the sink intact. If this ever stops being true the layer
    // has become redundant, which is worth knowing either way.
    let token = github_token();
    let sink = Sink::default();
    let plain = tracing_subscriber::fmt()
        .with_writer(sink.clone())
        .with_ansi(false)
        .with_max_level(tracing::Level::TRACE)
        .finish();

    tracing::subscriber::with_default(plain, || tracing::error!(token = %token, "failed"));

    assert!(
        sink.contents().contains(&token),
        "precondition: the default formatter is expected to leak"
    );
    assert!(!logged(|| tracing::error!(token = %token, "failed")).contains(&token));
}
