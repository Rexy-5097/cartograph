//! The desktop ASK flow, gate by gate.
//!
//! **No test here touches the network or a real keychain.** Every degraded
//! path is proved with [`UnreachableTransport`], which panics rather than
//! returning an error — an error would be swallowed by the very degradation
//! logic under test, and the test would pass while the request happened. The
//! one path that is allowed to reach a provider uses `MockTransport`, so the
//! bytes are inspected rather than sent.

use std::path::{Path, PathBuf};

use cartograph_ask::GroqProvider;
use cartograph_ask::testing::{MockTransport, UnreachableTransport};
use cartograph_core::NodeId;
use cartograph_desktop::ask::{self, AiState, AskAnswer};
use cartograph_desktop::credential::{CredentialError, CredentialStore, Secret};
use cartograph_desktop::error::DesktopErrorKind;
use cartograph_desktop::evidence::{AnalysisId, AnalysisSession};
use cartograph_desktop::optin::{GrantedRepository, OptIns};
use cartograph_desktop::repository;
use cartograph_desktop::session;
use cartograph_pipeline::authorization::RepositoryIdentity;

// ------------------------------------------------------------------ fixtures

/// A credential store that answers the same thing to everything.
///
/// Hand-written rather than the keychain: these tests are about the ASK flow's
/// decisions, and the keychain's own behaviour is proved in `keychain.rs`.
struct FixedStore(Result<Option<Secret>, CredentialError>);

impl FixedStore {
    fn holding(key: &str) -> Self {
        Self(Ok(Some(Secret::new(key.to_owned()))))
    }
    fn empty() -> Self {
        Self(Ok(None))
    }
    fn failing(error: CredentialError) -> Self {
        Self(Err(error))
    }
}

impl CredentialStore for FixedStore {
    fn get(&self, _identity: &RepositoryIdentity) -> Result<Option<Secret>, CredentialError> {
        match &self.0 {
            Ok(Some(secret)) => Ok(Some(Secret::new(secret.expose().to_owned()))),
            Ok(None) => Ok(None),
            Err(error) => Err(*error),
        }
    }
    fn set(&self, _identity: &RepositoryIdentity, _secret: Secret) -> Result<(), CredentialError> {
        Err(CredentialError::Unavailable)
    }
    fn delete(&self, _identity: &RepositoryIdentity) -> Result<(), CredentialError> {
        Err(CredentialError::Unavailable)
    }
}

/// A store that fails the test if it is consulted at all.
///
/// The opt-in gate must stop before the credential is read: a store that is
/// never asked cannot prompt, fail or leak.
struct NeverConsulted;

impl CredentialStore for NeverConsulted {
    fn get(&self, _identity: &RepositoryIdentity) -> Result<Option<Secret>, CredentialError> {
        panic!("the credential store was consulted on a path that must not read it");
    }
    fn set(&self, _identity: &RepositoryIdentity, _secret: Secret) -> Result<(), CredentialError> {
        panic!("the credential store was written on a path that must not write it");
    }
    fn delete(&self, _identity: &RepositoryIdentity) -> Result<(), CredentialError> {
        panic!("the credential store was cleared on a path that must not clear it");
    }
}

/// A credential-shaped value built at runtime, never a literal (QG-005).
fn fake_key() -> String {
    let prefix: String = ['g', 's', 'k', '_'].iter().collect();
    format!("{prefix}{}", "0123456789abcdef".repeat(2))
}

/// A provider that must never be reached.
fn unreachable() -> GroqProvider<UnreachableTransport> {
    GroqProvider::new(UnreachableTransport)
}

/// The fixture repository, analysed.
fn analysed() -> (AnalysisSession, AnalysisId) {
    let root = fixture_root();
    let repository = repository::validate(&root).expect("the fixture is a repository");
    let id = AnalysisId::first();
    let (_, session) = session::analyze_as(&repository, id).expect("the fixture analyses");
    (session, id)
}

/// A node that has at least one outgoing relationship, so the bundle is real.
fn a_node_with_evidence(session: &AnalysisSession, analysis: AnalysisId) -> NodeId {
    let graph = session.graph();
    graph
        .nodes()
        .map(cartograph_core::Node::id)
        .find(|&id| {
            ask::answer(session, analysis, id).is_ok_and(|answer| !answer.entries.is_empty())
        })
        .expect("the fixture has an artefact with derived relationships")
}

fn fixture_root() -> PathBuf {
    // The parser fixtures the other desktop suites already analyse.
    let here = Path::new(env!("CARGO_MANIFEST_DIR"));
    here.join("../cartograph-parser/tests/fixtures")
}

/// A grant for a scratch settings file, with ASK in the requested state.
fn grant(enabled: bool) -> (GrantedRepository, tempdir::Scratch) {
    let scratch = tempdir::Scratch::new();
    let mut granted =
        GrantedRepository::establish(&scratch.repository, Some(scratch.settings.clone()))
            .expect("a grant can be established");
    if enabled {
        granted.set_ask_enabled(true).expect("saved");
    }
    (granted, scratch)
}

/// A scratch directory, removed when the test ends.
mod tempdir {
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};

    pub struct Scratch {
        pub root: PathBuf,
        pub repository: PathBuf,
        pub settings: PathBuf,
    }

    impl Scratch {
        pub fn new() -> Self {
            static NEXT: AtomicU64 = AtomicU64::new(0);
            let unique = format!(
                "cartograph-ask-wiring-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            );
            let root = std::env::temp_dir().join(unique);
            let repository = root.join("repository");
            std::fs::create_dir_all(&repository).expect("scratch directory");
            let settings = root.join("optin.json");
            Self {
                root,
                repository,
                settings,
            }
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.root);
        }
    }
}

// ------------------------------------------------------- gate 1: selection

#[test]
fn a_stale_analysis_is_refused_before_anything_else_happens() {
    let (session, analysis) = analysed();
    let node = a_node_with_evidence(&session, analysis);
    let (granted, _scratch) = grant(true);

    let error = ask::explain(
        &session,
        analysis.next(),
        node,
        "why?",
        Some(&granted),
        &NeverConsulted,
        &unreachable(),
    )
    .expect_err("a stale analysis is refused");

    assert_eq!(error.kind, DesktopErrorKind::StaleSelection);
}

#[test]
fn an_unknown_artefact_is_refused_before_anything_else_happens() {
    let (session, analysis) = analysed();
    let (granted, _scratch) = grant(true);

    let error = ask::explain(
        &session,
        analysis,
        NodeId::from_raw(u64::MAX),
        "why?",
        Some(&granted),
        &NeverConsulted,
        &unreachable(),
    )
    .expect_err("an unknown artefact is refused");

    assert_eq!(error.kind, DesktopErrorKind::UnknownNode);
}

#[test]
fn an_empty_question_is_the_callers_mistake_and_is_reported_as_one() {
    // Not degraded: returning evidence for a question that was never asked
    // would call something an answer that is not one.
    let (session, analysis) = analysed();
    let node = a_node_with_evidence(&session, analysis);
    let (granted, _scratch) = grant(true);

    for question in ["", "   ", "\n\t"] {
        let error = ask::explain(
            &session,
            analysis,
            node,
            question,
            Some(&granted),
            &NeverConsulted,
            &unreachable(),
        )
        .expect_err("an empty question is refused");

        assert_eq!(error.kind, DesktopErrorKind::InvalidQuestion);
    }
}

// ---------------------------------------------------------- gate 2: opt-in

#[test]
fn with_ask_switched_off_no_credential_is_read_and_no_request_is_made() {
    // `NeverConsulted` panics if the store is touched; `UnreachableTransport`
    // panics if the network is. Both are the assertion.
    let (session, analysis) = analysed();
    let node = a_node_with_evidence(&session, analysis);
    let (granted, _scratch) = grant(false);

    let answer = ask::explain(
        &session,
        analysis,
        node,
        "why does this exist?",
        Some(&granted),
        &NeverConsulted,
        &unreachable(),
    )
    .expect("degrades rather than failing");

    assert_eq!(answer.ai, AiState::Disabled);
    assert!(answer.explanation.is_empty());
    assert!(
        !answer.entries.is_empty(),
        "the evidence must still be there"
    );
}

#[test]
fn with_no_grant_at_all_no_credential_is_read_and_no_request_is_made() {
    // A settings file that could not be read leaves the grant unset. That must
    // read as "off", never as "on".
    let (session, analysis) = analysed();
    let node = a_node_with_evidence(&session, analysis);

    let answer = ask::explain(
        &session,
        analysis,
        node,
        "why does this exist?",
        None,
        &NeverConsulted,
        &unreachable(),
    )
    .expect("degrades rather than failing");

    assert_eq!(answer.ai, AiState::Disabled);
    assert!(!answer.entries.is_empty());
}

#[test]
fn one_repository_being_enabled_does_not_enable_another() {
    // A enabled, B disabled. B must not reach a provider.
    let (session, analysis) = analysed();
    let node = a_node_with_evidence(&session, analysis);
    let (enabled, _a) = grant(true);
    let (disabled, _b) = grant(false);

    assert_ne!(
        enabled.identity(),
        disabled.identity(),
        "two grants must not share an identity"
    );

    let refused = ask::explain(
        &session,
        analysis,
        node,
        "why?",
        Some(&disabled),
        &NeverConsulted,
        &unreachable(),
    )
    .expect("degrades");
    assert_eq!(refused.ai, AiState::Disabled);

    // And the enabled one gets past the opt-in gate, so the difference is the
    // opt-in and not something else about the fixture.
    let allowed = ask::explain(
        &session,
        analysis,
        node,
        "why?",
        Some(&enabled),
        &FixedStore::empty(),
        &unreachable(),
    )
    .expect("degrades");
    assert_eq!(allowed.ai, AiState::Unavailable);
}

// ------------------------------------------------------ gate 3: credential

#[test]
fn with_no_credential_stored_nothing_is_sent() {
    let (session, analysis) = analysed();
    let node = a_node_with_evidence(&session, analysis);
    let (granted, _scratch) = grant(true);

    let answer = ask::explain(
        &session,
        analysis,
        node,
        "why?",
        Some(&granted),
        &FixedStore::empty(),
        &unreachable(),
    )
    .expect("degrades rather than failing");

    assert_eq!(answer.ai, AiState::Unavailable);
    assert!(answer.explanation.is_empty());
    assert!(!answer.entries.is_empty());
}

#[test]
fn every_credential_failure_degrades_without_sending_anything() {
    // Missing, refused, unavailable and broken are four different problems for
    // whoever fixes them, and one outcome here: show the evidence, send
    // nothing. The store keeps the distinction; this decision does not need it.
    let (session, analysis) = analysed();
    let node = a_node_with_evidence(&session, analysis);
    let (granted, _scratch) = grant(true);

    for error in [
        CredentialError::PermissionDenied,
        CredentialError::Unavailable,
        CredentialError::Backend,
    ] {
        let answer = ask::explain(
            &session,
            analysis,
            node,
            "why?",
            Some(&granted),
            &FixedStore::failing(error),
            &unreachable(),
        )
        .expect("degrades rather than failing");

        assert_eq!(answer.ai, AiState::Unavailable, "for {error:?}");
        assert!(!answer.entries.is_empty(), "for {error:?}");
    }
}

// -------------------------------------------------------- gate 4: provider

/// A provider reply whose citations quote the bundle it was sent.
fn reply_quoting(answer: &AskAnswer) -> String {
    let first = answer.entries.first().expect("evidence to quote");
    let content = serde_json::json!({
        "items": [{
            "text": "It is used by the other module.",
            "citations": [{ "edge": first.edge, "text": first.evidence }],
        }]
    })
    .to_string();
    serde_json::json!({
        "choices": [{ "message": { "content": content } }]
    })
    .to_string()
}

#[test]
fn an_enabled_repository_with_a_credential_gets_a_validated_explanation() {
    let (session, analysis) = analysed();
    let node = a_node_with_evidence(&session, analysis);
    let (granted, _scratch) = grant(true);
    let degraded = ask::answer(&session, analysis, node).expect("evidence");

    let transport = MockTransport::replying(200, reply_quoting(&degraded));
    let provider = GroqProvider::new(transport);

    let answer = ask::explain(
        &session,
        analysis,
        node,
        "why does this exist?",
        Some(&granted),
        &FixedStore::holding(&fake_key()),
        &provider,
    )
    .expect("answers");

    assert_eq!(answer.ai, AiState::Answered);
    assert_eq!(answer.explanation.len(), 1);
    assert!(!answer.explanation[0].citations.is_empty());
    // The evidence is still there underneath the explanation.
    assert!(!answer.entries.is_empty());
}

#[test]
fn an_answer_whose_citations_do_not_hold_is_rejected_whole() {
    // The most dangerous output this feature could produce is a partially
    // valid answer, because it looks checked.
    let (session, analysis) = analysed();
    let node = a_node_with_evidence(&session, analysis);
    let (granted, _scratch) = grant(true);
    let degraded = ask::answer(&session, analysis, node).expect("evidence");
    let edge = degraded.entries.first().expect("an edge").edge;

    let content = serde_json::json!({
        "items": [{
            "text": "A confident sentence.",
            "citations": [{ "edge": edge, "text": "text that is in no evidence at all" }],
        }]
    })
    .to_string();
    let body =
        serde_json::json!({ "choices": [{ "message": { "content": content } }] }).to_string();
    let provider = GroqProvider::new(MockTransport::replying(200, body));

    let answer = ask::explain(
        &session,
        analysis,
        node,
        "why?",
        Some(&granted),
        &FixedStore::holding(&fake_key()),
        &provider,
    )
    .expect("degrades rather than failing");

    assert_eq!(answer.ai, AiState::Failed);
    assert!(
        answer.explanation.is_empty(),
        "an unvalidated claim reached the window"
    );
    assert!(!answer.entries.is_empty());
}

#[test]
fn a_provider_that_refuses_degrades_with_the_evidence_intact() {
    let (session, analysis) = analysed();
    let node = a_node_with_evidence(&session, analysis);
    let (granted, _scratch) = grant(true);
    let provider = GroqProvider::new(MockTransport::replying(401, "{\"error\":{}}"));

    let answer = ask::explain(
        &session,
        analysis,
        node,
        "why?",
        Some(&granted),
        &FixedStore::holding(&fake_key()),
        &provider,
    )
    .expect("degrades rather than failing");

    assert_eq!(answer.ai, AiState::Failed);
    assert!(answer.explanation.is_empty());
    assert!(!answer.entries.is_empty());
}

// ------------------------------------------------------------- the leakage

#[test]
fn nothing_the_window_receives_carries_the_credential_or_the_identity() {
    let (session, analysis) = analysed();
    let node = a_node_with_evidence(&session, analysis);
    let (granted, _scratch) = grant(true);
    let degraded = ask::answer(&session, analysis, node).expect("evidence");
    let key = fake_key();

    let provider = GroqProvider::new(MockTransport::replying(200, reply_quoting(&degraded)));
    let answer = ask::explain(
        &session,
        analysis,
        node,
        "why?",
        Some(&granted),
        &FixedStore::holding(&key),
        &provider,
    )
    .expect("answers");

    // What crosses to the window is exactly this JSON.
    let wire = serde_json::to_string(&answer).expect("serialisable");

    assert!(!wire.contains(&key), "the credential reached the window");
    assert!(
        !wire.contains(granted.identity().keychain_subject()),
        "the repository identity reached the window"
    );
    // And its Debug does not carry them either, for a panic or a log line.
    let debugged = format!("{answer:?}");
    assert!(!debugged.contains(&key));
    assert!(!debugged.contains(granted.identity().keychain_subject()));
}

#[test]
fn the_credential_goes_in_the_header_and_nowhere_else() {
    // It must reach the Authorization header -- that is the point -- and must
    // not appear in the body, which is the part that would be logged, cached
    // or replayed by a proxy.
    let (session, analysis) = analysed();
    let node = a_node_with_evidence(&session, analysis);
    let (granted, _scratch) = grant(true);
    let degraded = ask::answer(&session, analysis, node).expect("evidence");
    let key = fake_key();

    // A transport that captures, wrapped by the provider under test.
    let transport = MockTransport::replying(200, reply_quoting(&degraded));
    let provider = GroqProvider::new(transport);
    let _ = ask::explain(
        &session,
        analysis,
        node,
        "why?",
        Some(&granted),
        &FixedStore::holding(&key),
        &provider,
    )
    .expect("answers");

    let sent = provider.transport().only_call();
    let body = String::from_utf8(sent.body().to_vec()).expect("utf-8");

    assert!(!body.contains(&key), "the credential reached the body");
    assert!(
        !body.contains(granted.identity().keychain_subject()),
        "the repository identity reached the body"
    );
    assert!(
        sent.has_header("Authorization"),
        "the credential must go in the header"
    );
}

// -------------------------------------------------------------- opt-in state

#[test]
fn the_same_repository_recovers_the_same_identity_and_opt_in_after_a_restart() {
    let scratch = tempdir::Scratch::new();

    let subject = {
        let mut granted =
            GrantedRepository::establish(&scratch.repository, Some(scratch.settings.clone()))
                .expect("granted");
        granted.set_ask_enabled(true).expect("saved");
        granted.identity().keychain_subject().to_owned()
    };

    // A second run, reading the settings file back.
    let reopened =
        GrantedRepository::establish(&scratch.repository, Some(scratch.settings.clone()))
            .expect("granted");

    assert_eq!(reopened.identity().keychain_subject(), subject);
    assert!(reopened.ask_enabled(), "the opt-in did not survive");
}

#[test]
fn an_unreadable_settings_file_leaves_ask_off_rather_than_on() {
    // The failure direction that matters: if settings cannot be read, ASK must
    // read as off. `GrantedRepository::establish` returns an error, the shell
    // stores `None`, and gate 2 refuses.
    let (session, analysis) = analysed();
    let node = a_node_with_evidence(&session, analysis);

    let answer = ask::explain(
        &session,
        analysis,
        node,
        "why?",
        None,
        &NeverConsulted,
        &unreachable(),
    )
    .expect("degrades");

    assert_eq!(answer.ai, AiState::Disabled);
}

#[test]
fn the_degraded_entry_point_still_says_the_model_was_not_consulted() {
    // `ask::answer` is unchanged by this slice, and its contract is what the
    // panel has rendered since Slice 2.
    let (session, analysis) = analysed();
    let node = a_node_with_evidence(&session, analysis);

    let answer = ask::answer(&session, analysis, node).expect("evidence");

    assert_eq!(answer.ai, AiState::Disabled);
    assert!(answer.explanation.is_empty());
}

#[test]
fn an_opt_in_table_never_records_anything_credential_shaped() {
    // §13's second half, from the other side: the only thing this product
    // persists itself is the opt-in table.
    let scratch = tempdir::Scratch::new();
    let mut granted =
        GrantedRepository::establish(&scratch.repository, Some(scratch.settings.clone()))
            .expect("granted");
    granted.set_ask_enabled(true).expect("saved");

    let written = std::fs::read_to_string(&scratch.settings).expect("the table was written");

    assert!(!written.contains(&fake_key()));
    assert!(!written.to_lowercase().contains("authorization"));
    // It does hold the opaque token, which is not a credential and is how the
    // same repository recovers the same identity (ADR-0021 Amendment 4).
    assert!(written.contains(granted.identity().keychain_subject()));
    let _ = OptIns::default();
}
