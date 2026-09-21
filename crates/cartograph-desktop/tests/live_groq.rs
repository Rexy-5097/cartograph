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

use cartograph_desktop::ask::{self, AiState, AskAnswer};
use cartograph_desktop::credential::{CredentialStore, Secret};
use cartograph_desktop::evidence::AnalysisId;
use cartograph_desktop::keychain::KeychainStore;
use cartograph_desktop::optin::{self, GrantedRepository};
use cartograph_desktop::{repository, session};

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
        let mut typed = String::new();
        std::io::stdin()
            .read_to_string(&mut typed)
            .expect("the key must be piped in; see this file's header");
        let trimmed = typed.trim();
        assert!(
            !trimmed.is_empty(),
            "no key on stdin; see this file's header for the command"
        );
        Secret::new(trimmed)
    };
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
        "What does this artefact depend on, and why?",
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

    check(&answer, &evidence, cleanup.grant_ref());
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
