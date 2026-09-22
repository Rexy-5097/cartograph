//! M16's live validation: one real Groq request through the production path.
//!
//! # This is not part of the test suite
//!
//! It is `#[ignore]`d, so `cargo test` never runs it and CI never runs it. It
//! exists to be run **once, by a developer, on their own machine**, to prove
//! the one M16 condition no offline test can: that the path from a real
//! keychain to a real provider and back through the citation contract actually
//! works against the live service.
//!
//! Everything it calls is production code. It adds no product feature, no
//! settings panel, no key field, no environment variable and no configuration
//! entry — §13 keeps credentials in the OS keychain, and a settings panel is a
//! frozen v1 non-goal. What it adds is the developer-only provisioning step
//! that the product deliberately does not ship.
//!
//! # How to run it
//!
//! The key is piped in, so it never reaches a command line, an environment
//! variable, a file, or this repository. On Windows, from the repository root:
//!
//! ```text
//! powershell -NoProfile -Command "$s = Read-Host -AsSecureString 'Groq API key'; [Runtime.InteropServices.Marshal]::PtrToStringBSTR([Runtime.InteropServices.Marshal]::SecureStringToBSTR($s))" | cargo test -p cartograph-desktop --test live_groq -- --ignored --nocapture
//! ```
//!
//! On macOS or Linux, `read -s` does the same job:
//!
//! ```text
//! (read -rs -p 'Groq API key: ' k; printf '%s' "$k") | cargo test -p cartograph-desktop --test live_groq -- --ignored --nocapture
//! ```
//!
//! `Read-Host -AsSecureString` and `read -s` both hide the typing. The key
//! travels one pipe into this process, is wrapped in [`Secret`] on the line it
//! is read, and is deleted from the keychain before the test returns.
//!
//! # What it deliberately does not do
//!
//! It does not print the key, the repository identity, the keychain account,
//! the request body or the response body. It prints a handful of yes/no facts
//! and nothing else, because a validation that has to show the secret to prove
//! it worked has not proved anything worth having.
//!
//! **One honest limitation:** the key lives in a `String` for the moment
//! between reading and storing it, and Rust cannot reliably zero that memory
//! without a dependency this validation does not justify. It is not written to
//! disk, not logged and not passed to another process.

use std::io::Read;
use std::path::{Path, PathBuf};

use cartograph_ask::error::ResponseFault;
use cartograph_ask::groq::request_body;
use cartograph_ask::transport::TransportError;
use cartograph_ask::{GroqProvider, Provider, ProviderError, Question, UreqTransport};
use cartograph_core::NodeId;
use cartograph_desktop::ask::{self, AiState, AskAnswer};
use cartograph_desktop::credential::{CredentialStore, Secret};
use cartograph_desktop::evidence::{AnalysisId, AnalysisSession};
use cartograph_desktop::keychain::KeychainStore;
use cartograph_desktop::optin::{self, GrantedRepository};
use cartograph_desktop::{repository, session};
use cartograph_graph::bundle::EvidenceBundle;
use cartograph_graph::trace::{DEFAULT_MAX_DEPTH, trace};

/// Removes the credential and restores the opt-in, whatever happens above it.
///
/// A `Drop` guard rather than a line at the end of the test: an assertion that
/// fails must not leave a developer's API key installed on their machine.
struct Cleanup {
    store: KeychainStore,
    grant: GrantedRepository,
}

impl Drop for Cleanup {
    fn drop(&mut self) {
        let removed = self.store.delete(self.grant.identity()).is_ok();
        let absent = matches!(self.store.get(self.grant.identity()), Ok(None));
        // The opt-in record stays — it is bookkeeping, not a credential — but
        // ASK goes back off, so this validation leaves nothing switched on.
        let _ = self.grant.set_ask_enabled(false);
        println!("credential deleted afterward: {}", yes(removed && absent));
        println!(
            "ask opt-in restored to off:  {}",
            yes(!self.grant.ask_enabled())
        );
    }
}

fn yes(value: bool) -> &'static str {
    if value { "yes" } else { "NO" }
}

/// The question, named once so the diagnosis below asks exactly the same one.
const QUESTION: &str = "What does this artefact depend on, and why?";

/// The repository this validation explains.
///
/// The parser fixtures: small, public, already in this repository, and the
/// same tree the offline suites analyse. Deliberately not the developer's own
/// work — what leaves the machine should be the least interesting thing that
/// still proves the path.
fn subject() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../cartograph-parser/tests/fixtures")
}

#[test]
#[ignore = "makes a real network request; run manually, see this file's header"]
fn one_real_groq_request_through_the_production_path() {
    // ---- the key, read from a pipe and wrapped immediately ----------------
    let credential = {
        // Read as **bytes**, not as text. `read_to_string` decodes as UTF-8 and
        // says nothing about what it decoded, and what a shell delivers here is
        // exactly the thing in question -- see `describe_bytes`.
        let mut raw = Vec::new();
        std::io::stdin()
            .read_to_end(&mut raw)
            .expect("the key must be piped in; see this file's header");
        describe_bytes(&raw);

        // Identical to what `read_to_string` did: valid UTF-8 or nothing. Kept
        // that way on purpose, so this diagnosis reports the pipeline rather
        // than quietly repairing it.
        let typed = String::from_utf8(raw).expect("stdin did not decode as UTF-8");
        let trimmed = typed.trim();
        assert!(
            !trimmed.is_empty(),
            "no key on stdin; see this file's header for the command"
        );
        describe_text(trimmed);
        Secret::new(trimmed)
    };
    let supplied_len = credential.expose().len();
    println!("credential supplied:         yes");

    // ---- the grant, through the production flow ---------------------------
    // `GrantedRepository::establish` is exactly what `analyze_repository`
    // calls. No identity is minted by hand and none is derived from the path.
    let settings = optin::settings_path();
    assert!(
        settings.is_some(),
        "this platform reports nowhere to persist settings"
    );
    let mut grant = GrantedRepository::establish(&subject(), settings)
        .expect("the production grant flow establishes an identity");
    let enabled = grant.set_ask_enabled(true).expect("the opt-in is recorded");
    assert!(enabled, "opt-in did not take");
    println!("repository grant established: yes");
    println!("ask opt-in enabled:           {}", yes(grant.ask_enabled()));

    // ---- the keychain, through the production store -----------------------
    let store = KeychainStore::open().expect("this machine has a usable OS keychain");
    store
        .set(grant.identity(), credential)
        .expect("the credential is stored");

    let retrieved = store
        .get(grant.identity())
        .expect("the keychain can be consulted");
    assert!(retrieved.is_some(), "the credential did not come back");
    println!("credential available:         yes");

    // From here on, a failure must still clean up.
    let cleanup = Cleanup { store, grant };

    // ---- the analysis -----------------------------------------------------
    let validated = repository::validate(&subject()).expect("the fixture is a repository");
    let analysis = AnalysisId::first();
    let (_, session) = session::analyze_as(&validated, analysis).expect("the fixture analyses");

    let node = session
        .graph()
        .nodes()
        .map(cartograph_core::Node::id)
        .find(|&id| {
            ask::answer(&session, analysis, id).is_ok_and(|answer| !answer.entries.is_empty())
        })
        .expect("the fixture has an artefact with derived relationships");

    let evidence = ask::answer(&session, analysis, node).expect("the degraded answer");

    // ---- the one real request ---------------------------------------------
    let started = std::time::Instant::now();
    let answer = ask::explain_with_os_keychain(
        &session,
        analysis,
        node,
        QUESTION,
        Some(cleanup.grant_ref()),
    )
    .expect("the ASK command returns");
    let elapsed = started.elapsed();

    println!("model:                        openai/gpt-oss-120b");
    println!(
        "http + parse + validate:      {}",
        yes(answer.ai == AiState::Answered)
    );
    println!("elapsed:                      {elapsed:?}");

    if answer.ai != AiState::Answered {
        diagnose(&session, node, cleanup.grant_ref(), supplied_len);
    }

    check(&answer, &evidence, cleanup.grant_ref());
}

/// Names the layer a failure happened in, when the production path did not
/// answer.
///
/// # Why this exists
///
/// [`ask::explain`] collapses every provider failure onto one state on
/// purpose: a message from a third party about a request the window never made
/// is not something the window should show. That is right for the product and
/// useless for a diagnosis, so this reads the classification one layer down,
/// where it still exists.
///
/// # What it may print
///
/// A category, a status code, a bounded classification the provider itself
/// supplied, and yes/no facts. **Never** the key, the `Authorization` header,
/// the request body, the response body, the identity, the keychain account or
/// a path. That is structural rather than careful: [`ProviderError`] and
/// [`TransportError`] have no field that could hold one, and the provider's
/// own `type` string reaches here only through `ApiError::safe_kind`, which
/// drops anything that is not a short lowercase identifier.
///
/// # It makes one further request
///
/// One, through the production assembly — `GroqProvider::new(
/// UreqTransport::new())`, the same two lines [`ask::explain_with_os_keychain`]
/// builds — and not a mock, because the point is to learn what the real stack
/// did. The two checks above it need no network at all, so a failure they
/// explain is a failure this call will reproduce without reaching one either.
fn diagnose(session: &AnalysisSession, node: NodeId, grant: &GrantedRepository, supplied: usize) {
    println!("--- diagnosis (test-only) ----------------------------------");

    let graph = session.graph();
    let walk = trace(graph, node, DEFAULT_MAX_DEPTH);
    let bundle = EvidenceBundle::from_trace(graph, &walk).expect("the evidence bundles");
    let question = Question::new(QUESTION).expect("the question is within bounds");

    // Layer 1: the bytes. Built before any transport is touched, so a failure
    // here would mean the request never existed to be sent.
    let body = request_body(&bundle, &question);
    println!("request bytes built:          {}", yes(body.is_ok()));
    println!("edges sent:                   {}", bundle.edges().len());

    // Layer 2: the credential, in the shape a header would carry it. `http`
    // refuses a header value holding a byte outside 0x20..=0x7E, and `ureq`
    // reports that refusal before it opens a socket — which is one of the two
    // ways this can fail without any network at all.
    let store = KeychainStore::open().expect("this machine has a usable OS keychain");
    let credential = store
        .get(grant.identity())
        .expect("the keychain can be consulted")
        .expect("the credential is still stored");
    println!(
        "stored printable ascii:       {}",
        yes(printable_ascii(credential.expose()))
    );
    println!(
        "stored header-legal:          {}",
        yes(header_legal(credential.expose()))
    );
    println!(
        "stored credential intact:     {}",
        yes(credential.expose().len() == supplied)
    );

    // Layer 3: the one call, through the assembly the product ships.
    let provider = GroqProvider::new(UreqTransport::new());
    let started = std::time::Instant::now();
    let outcome = provider.explain(&bundle, &question, &credential);
    let elapsed = started.elapsed();

    match outcome {
        Ok(_) => println!("provider error:               none — it answered this time"),
        Err(error) => report(&error),
    }
    println!("provider call elapsed:        {elapsed:?}");
}

/// Prints one failure as a category and a set of yes/no facts.
fn report(error: &ProviderError) {
    println!("provider error:               {}", label(error));
    println!(
        "http status:                  {}",
        status_of(error).map_or_else(|| "-".to_owned(), |status| status.to_string())
    );
    println!(
        "UreqTransport entered:        {}",
        yes(!matches!(error, ProviderError::MalformedRequest))
    );
    println!("external request attempted:   {}", yes(attempted(error)));
    println!("http response received:       {}", yes(responded(error)));
    println!("parse_answer entered:         {}", yes(parsed(error)));
    println!("citations checked:            {}", yes(checked(error)));
}

/// Whether the credential could travel in a header at all.
///
/// This is `http`'s own rule, not a guess at it: a header value may hold a tab
/// or any byte from 0x20 up except 0x7F, and **a byte above 0x7F is legal**.
/// So a UTF-8 BOM or a mojibaked key would be *sent*, not refused — measured
/// rather than assumed, because assuming the opposite is what sent this
/// diagnosis down the wrong path first. A control character is the one thing
/// that stops a request before a socket opens.
fn header_legal(value: &str) -> bool {
    value
        .bytes()
        .all(|byte| byte == b'\t' || (byte >= 0x20 && byte != 0x7F))
}

/// Whether the credential is plain printable ASCII.
///
/// Not a send/no-send rule — see [`header_legal`] — but the thing that
/// separates "the key arrived intact" from "something in the pipe re-encoded
/// it". A Groq key is ASCII, so anything else here is the pipeline's doing.
fn printable_ascii(value: &str) -> bool {
    value.bytes().all(|byte| (0x20..=0x7E).contains(&byte))
}

/// The HTTP status, when one was seen.
fn status_of(error: &ProviderError) -> Option<u16> {
    match error {
        ProviderError::Refused { status, .. }
        | ProviderError::Transport {
            cause: TransportError::Status { status },
        } => Some(*status),
        _ => None,
    }
}

/// Whether anything left the machine.
///
/// `TransportError::Request` is the category `ureq` reports when it refuses a
/// request *before* connecting — a URL it will not send to, or a header value
/// the HTTP layer rejected. Every other category means a name was resolved or
/// a socket was opened.
fn attempted(error: &ProviderError) -> bool {
    !matches!(
        error,
        ProviderError::MalformedRequest
            | ProviderError::Transport {
                cause: TransportError::Request
            }
    )
}

/// Whether a server replied.
fn responded(error: &ProviderError) -> bool {
    checked(error)
        || matches!(
            error,
            ProviderError::Refused { .. }
                | ProviderError::InvalidResponse { .. }
                | ProviderError::Transport {
                    cause: TransportError::Status { .. }
                }
        )
}

/// `parse_answer` runs only on a 2xx reply, so reaching it is itself a fact.
fn parsed(error: &ProviderError) -> bool {
    checked(error) || matches!(error, ProviderError::InvalidResponse { .. })
}

/// Whether the reply got as far as the citation contract.
fn checked(error: &ProviderError) -> bool {
    matches!(
        error,
        ProviderError::NoItems
            | ProviderError::EmptyItem { .. }
            | ProviderError::UncitedItem { .. }
            | ProviderError::InvalidCitation { .. }
            | ProviderError::ScopeMismatch
    )
}

/// One failure as a bounded, machine-readable category.
fn label(error: &ProviderError) -> String {
    match error {
        ProviderError::NoItems => "validation/no-items".to_owned(),
        ProviderError::EmptyItem { item } => format!("validation/empty-item[{item}]"),
        ProviderError::UncitedItem { item } => format!("validation/uncited-item[{item}]"),
        ProviderError::InvalidCitation {
            item,
            citation,
            cause,
        } => format!("validation/invalid-citation[{item}:{citation}] {cause}"),
        ProviderError::ScopeMismatch => "validation/scope-mismatch".to_owned(),
        ProviderError::MalformedRequest => "request/malformed".to_owned(),
        ProviderError::InvalidResponse { fault } => format!("parse/{}", fault_label(*fault)),
        ProviderError::Unavailable => "provider/unavailable".to_owned(),
        ProviderError::Transport { cause } => format!("transport/{}", transport_label(*cause)),
        // `kind` is the provider's own `type`, already bounded to a short
        // lowercase identifier by `ApiError::safe_kind`. Its `message` is not
        // modelled anywhere in this workspace, so it cannot appear here.
        ProviderError::Refused { status, kind } => kind.as_ref().map_or_else(
            || format!("refused/{status}"),
            |kind| format!("refused/{status}/{kind}"),
        ),
        // `ProviderError` is `#[non_exhaustive]`.
        _ => "unclassified".to_owned(),
    }
}

/// Which of the three ways a reply failed to be one.
fn fault_label(fault: ResponseFault) -> &'static str {
    match fault {
        ResponseFault::NotJson => "not-json",
        ResponseFault::NoChoices => "no-choices",
        ResponseFault::NotTheSchema => "not-the-schema",
        _ => "unclassified",
    }
}

/// Which of the transport's five categories.
fn transport_label(cause: TransportError) -> String {
    match cause {
        TransportError::Request => "request-not-sent".to_owned(),
        TransportError::Connect => "connect".to_owned(),
        TransportError::Timeout => "timeout".to_owned(),
        TransportError::Read => "read".to_owned(),
        TransportError::Status { status } => format!("status-{status}"),
        _ => "unclassified".to_owned(),
    }
}

/// Checks the live answer rather than trusting it.
///
/// `cartograph_ask` already enforced the citation contract before this value
/// could exist. Re-checking it here proves the same property against the
/// records the panel renders, which is the thing a reader would compare.
fn check(answer: &AskAnswer, evidence: &AskAnswer, grant: &GrantedRepository) {
    assert_eq!(
        answer.ai,
        AiState::Answered,
        "the provider did not produce a validated answer"
    );
    assert!(
        !answer.explanation.is_empty(),
        "an answered state with no explanation"
    );

    for item in &answer.explanation {
        assert!(!item.text.trim().is_empty(), "an empty explanation item");
        assert!(
            !item.citations.is_empty(),
            "an explanation item with no citation"
        );
        for citation in &item.citations {
            assert!(!citation.text.is_empty(), "an empty citation");
            let quoted = evidence
                .entries
                .iter()
                .find(|entry| entry.edge.as_u64() == citation.edge)
                .expect("a citation named an edge that was not sent");
            assert!(
                quoted.evidence.contains(&citation.text),
                "a citation was not a byte-for-byte substring of its evidence"
            );
        }
    }
    println!("items:                        {}", answer.explanation.len());
    println!("every item cited:             yes");
    println!("citations verbatim in bundle: yes");

    // What the window would receive, checked on the actual serialised payload.
    let wire = serde_json::to_string(answer).expect("the payload serialises");
    assert!(
        !wire.contains(grant.identity().keychain_subject()),
        "the repository identity reached the window"
    );
    // The evidence is still underneath the explanation, as on every other path.
    assert!(!answer.entries.is_empty());
    println!("frontend payload safe:        yes");
}

impl Cleanup {
    /// The grant, borrowed for the production call.
    fn grant_ref(&self) -> &GrantedRepository {
        &self.grant
    }
}

// ---------------------------------------------------------------------------
// What the shell actually delivered
// ---------------------------------------------------------------------------

/// Describes the bytes on stdin without reproducing any of them.
///
/// # Why this is counted rather than printed
///
/// Everything here is a length, a count or a yes/no. No byte of the input is
/// echoed, in any encoding, at any point -- the input may be a credential, and
/// a diagnostic that prints one to find out what is wrong with it has created
/// a worse problem than the one it was solving.
///
/// # What the numbers distinguish
///
/// | shape | signature |
/// |---|---|
/// | ASCII | all bytes below 0x80, no NUL, no BOM |
/// | UTF-8 | some byte at or above 0x80, no NUL |
/// | UTF-8 with BOM | first three bytes `ef bb bf` |
/// | UTF-16LE | every second byte NUL, starting at index 1 |
/// | UTF-16BE | every second byte NUL, starting at index 0 |
///
/// UTF-16LE of ASCII text is the case worth naming, because it is **valid
/// UTF-8**: every byte of it is below 0x80, so `String::from_utf8` accepts it
/// and hands back a string of interleaved `U+0000`. Nothing downstream
/// complains -- `str::trim` does not strip a NUL, because a NUL is not
/// whitespace -- until `http` refuses to put a control character in a header
/// value, which is where this run stopped.
fn describe_bytes(raw: &[u8]) {
    let bom_utf8 = raw.starts_with(&[0xEF, 0xBB, 0xBF]);
    let core = strip_framing(raw);
    let (le, be) = utf16_shape(core);

    println!("--- stdin, as bytes (test-only) ----------------------------");
    println!("bytes received:               {}", raw.len());
    println!("bytes == 0x00 (NUL):          {}", count(raw, 0x00));
    println!("bytes == 0x0a (LF):           {}", count(raw, 0x0A));
    println!("bytes == 0x0d (CR):           {}", count(raw, 0x0D));
    println!("leading utf-8 BOM ef bb bf:   {}", yes(bom_utf8));
    println!(
        "leading utf-16le BOM ff fe:   {}",
        yes(raw.starts_with(&[0xFF, 0xFE]))
    );
    println!(
        "leading utf-16be BOM fe ff:   {}",
        yes(raw.starts_with(&[0xFE, 0xFF]))
    );
    println!("utf-16le alternating shape:   {}", yes(le));
    println!("utf-16be alternating shape:   {}", yes(be));
    println!(
        "every byte ascii (< 0x80):    {}",
        yes(raw.iter().all(u8::is_ascii))
    );
    println!("control bytes seen:           {}", control_census(raw));
    println!(
        "first control byte at index:  {}",
        raw.iter()
            .position(|byte| *byte < 0x20 || *byte == 0x7F)
            .map_or_else(|| "-".to_owned(), |at| at.to_string())
    );
}

/// Which control bytes arrived, and how many of each.
///
/// # Why naming these leaks nothing
///
/// A Groq key is printable ASCII, so a control byte is never part of one.
/// What it is instead is a signature of the thing that produced the stream:
///
/// | byte | what it means here |
/// |---|---|
/// | `1b` | an escape sequence — a bracketed-paste marker (`ESC [ 2 0 0 ~`) or a cursor key |
/// | `0d` / `0a` | a line ending the shell added |
/// | `00` | the UTF-16 artefact this harness already met |
/// | `03` / `04` | an interrupt or end-of-transmission reaching the reader |
///
/// Counting them identifies what the terminal did without revealing a single
/// character of the payload, which is why this prints byte values where
/// nothing else in this file does.
fn control_census(raw: &[u8]) -> String {
    let mut counts = [0_usize; 256];
    for byte in raw {
        if *byte < 0x20 || *byte == 0x7F {
            counts[*byte as usize] += 1;
        }
    }

    let census = counts
        .iter()
        .enumerate()
        .filter(|(_, seen)| **seen > 0)
        .map(|(byte, seen)| format!("{byte:02x}:{seen}"))
        .collect::<Vec<_>>()
        .join(" ");

    if census.is_empty() {
        "none".to_owned()
    } else {
        census
    }
}

/// Describes the decoded string, again without reproducing it.
fn describe_text(text: &str) {
    println!("--- stdin, as decoded text (test-only) ---------------------");
    println!("decoded chars:                {}", text.chars().count());
    println!("decoded utf-8 bytes:          {}", text.len());
    println!("contains non-ascii:           {}", yes(!text.is_ascii()));
    println!(
        "contains control characters:  {}",
        yes(text.chars().any(char::is_control))
    );
    println!(
        "printable ascii:              {}",
        yes(printable_ascii(text))
    );
    println!("header-legal:                 {}", yes(header_legal(text)));
}

/// How many bytes equal `wanted`.
fn count(raw: &[u8], wanted: u8) -> usize {
    raw.iter()
        .fold(0, |total, byte| total + usize::from(*byte == wanted))
}

/// The payload, with a UTF-8 BOM and trailing line endings removed.
///
/// Both are added by a shell rather than by whoever typed the value, and the
/// alternating test below has to run on what is left or the framing decides
/// the answer.
fn strip_framing(raw: &[u8]) -> &[u8] {
    let body = raw.strip_prefix(&[0xEF, 0xBB, 0xBF]).unwrap_or(raw);
    let end = body
        .iter()
        .rposition(|byte| *byte != 0x0A && *byte != 0x0D)
        .map_or(0, |last| last + 1);
    &body[..end]
}

/// Whether the bytes carry the alternating NUL of UTF-16, and which way round.
///
/// Returns `(little-endian, big-endian)`. Both are false for ASCII or UTF-8,
/// and the "not every byte is NUL" clause keeps a run of padding from reading
/// as text in either order.
fn utf16_shape(core: &[u8]) -> (bool, bool) {
    if core.len() < 4 || core.len() % 2 != 0 {
        return (false, false);
    }
    let odd_nul = core.iter().skip(1).step_by(2).all(|byte| *byte == 0);
    let even_nul = core.iter().step_by(2).all(|byte| *byte == 0);
    (odd_nul && !even_nul, even_nul && !odd_nul)
}

/// The sentinel the offline check below looks for.
const SENTINEL: &str = "TEST_ASCII_123";

/// What this shell does to fourteen ASCII bytes. **No credential involved.**
///
/// Run exactly as the live test is run, but with the sentinel in place of a
/// key, so the transformation can be located without a secret anywhere near
/// it:
///
/// ```text
/// powershell -NoProfile -Command "[Console]::Out.Write('TEST_ASCII_123')" | cargo test -p cartograph-desktop --test live_groq -- --ignored --nocapture what_the_shell_delivers
/// ```
///
/// ```text
/// printf '%s' 'TEST_ASCII_123' | cargo test -p cartograph-desktop --test live_groq -- --ignored --nocapture what_the_shell_delivers
/// ```
///
/// # Why it prints bytes here and nowhere else
///
/// Only when it can prove they are the sentinel's. The hex is shown if every
/// byte is either one the sentinel contains or one a shell adds as framing --
/// NUL, CR, LF, or a BOM. Anything else and the input is not the sentinel, so
/// it might be a key, so nothing is printed. A developer who pipes the wrong
/// thing in here gets counts, not a leak.
#[test]
#[ignore = "an offline shell check; see this function's documentation"]
fn what_the_shell_delivers() {
    let mut raw = Vec::new();
    std::io::stdin()
        .read_to_end(&mut raw)
        .expect("pipe the sentinel in; see this function's documentation");

    describe_bytes(&raw);

    let decoded = String::from_utf8(raw.clone());
    println!("decodes as utf-8:             {}", yes(decoded.is_ok()));
    let matched = decoded
        .as_deref()
        .map(str::trim)
        .is_ok_and(|text| text == SENTINEL);
    println!("arrives as the sentinel:      {}", yes(matched));

    if sentinel_only(&raw) {
        println!("bytes:                        {}", hex(&raw));
    } else {
        println!("bytes:                        withheld -- not the sentinel");
    }

    assert!(
        matched,
        "the shell did not deliver {SENTINEL} unchanged -- the counts above say how"
    );
}

/// Whether every byte is one the sentinel contains, or one a shell adds.
fn sentinel_only(raw: &[u8]) -> bool {
    raw.iter().all(|byte| {
        SENTINEL.as_bytes().contains(byte)
            || matches!(byte, 0x00 | 0x0A | 0x0D | 0xEF | 0xBB | 0xBF)
    })
}

/// The bytes, for the one case that has proved itself safe to show.
fn hex(raw: &[u8]) -> String {
    use std::fmt::Write as _;

    raw.iter().fold(String::new(), |mut rendered, byte| {
        let _ = write!(rendered, "{byte:02x}");
        rendered
    })
}
