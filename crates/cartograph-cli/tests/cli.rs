//! Integration tests that run the real binary.
//!
//! `CARGO_BIN_EXE_cartograph` is set by Cargo for integration tests, so these
//! exercise the shipped executable without an assertion-framework dependency.

use std::process::Command;

fn cartograph() -> Command {
    Command::new(env!("CARGO_BIN_EXE_cartograph"))
}

#[test]
fn version_prints_the_binary_and_specification_versions() {
    let output = cartograph().arg("version").output().expect("binary runs");

    assert!(output.status.success(), "exit status: {}", output.status);
    let stdout = String::from_utf8(output.stdout).expect("valid UTF-8");
    assert!(
        stdout.contains("cartograph "),
        "missing version line: {stdout}"
    );
    assert!(
        stdout.contains("specification V3"),
        "missing spec line: {stdout}"
    );
    assert!(
        stdout.contains("milestone M01"),
        "missing milestone line: {stdout}"
    );
}

#[test]
fn version_json_is_parseable() {
    let output = cartograph()
        .args(["version", "--json"])
        .output()
        .expect("binary runs");

    assert!(output.status.success());
    let value: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("stdout is valid JSON");

    assert_eq!(value["spec_version"], "V3");
    assert_eq!(value["milestone"], "M01");
    assert!(value["version"].is_string());
}

#[test]
fn json_output_goes_to_stdout_and_logs_go_to_stderr() {
    // Anything piping `--json` into jq needs stdout to hold nothing but JSON.
    let output = cartograph()
        .args(["version", "--json", "-vv"])
        .output()
        .expect("binary runs");

    assert!(
        serde_json::from_slice::<serde_json::Value>(&output.stdout).is_ok(),
        "log output contaminated stdout: {}",
        String::from_utf8_lossy(&output.stdout)
    );
}

#[test]
fn an_unknown_command_fails_rather_than_guessing() {
    let output = cartograph().arg("analyze").output().expect("binary runs");

    assert!(
        !output.status.success(),
        "`analyze` is an M09 deliverable and must not appear to work at M00"
    );
}

#[test]
fn no_subcommand_is_an_error_with_usage() {
    let output = cartograph().output().expect("binary runs");

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("Usage"), "no usage shown: {stderr}");
}

// ── cartograph parse (M01) ──────────────────────────────────────────

use std::path::PathBuf;

fn python_fixtures() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("cartograph-parser/tests/fixtures/python")
}

fn parser_fixtures() -> PathBuf {
    // The parser crate's fixture corpus doubles as the CLI's test corpus.
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("cartograph-parser/tests/fixtures")
}

#[test]
fn parse_walks_a_tree_and_summarises_facts() {
    let output = cartograph()
        .arg("parse")
        .arg(parser_fixtures())
        .output()
        .expect("binary runs");

    assert!(output.status.success(), "exit: {}", output.status);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Files"), "no summary: {stdout}");
    assert!(
        stdout.contains("component.tsx"),
        "TSX file missed: {stdout}"
    );
    assert!(
        stdout.contains("observations, not resolved edges"),
        "the summary must label HTTP counts as observations: {stdout}"
    );
}

#[test]
fn parse_json_is_pure_and_carries_the_fact_model() {
    let output = cartograph()
        .args(["parse", "--json"])
        .arg(parser_fixtures())
        .output()
        .expect("binary runs");

    assert!(output.status.success());
    let value: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("stdout is pure JSON");
    assert!(value["totals"]["files"].as_u64().unwrap() >= 8);
    assert!(value["totals"]["http_observations"].as_u64().unwrap() >= 5);
    // Failed files are included, not dropped.
    assert!(value["totals"]["failed"].as_u64().unwrap() >= 1);
    // Fact paths are repository-relative.
    for file in value["files"].as_array().unwrap() {
        let path = file["path"].as_str().unwrap();
        assert!(!path.starts_with('/'), "absolute path leaked: {path}");
    }
}

#[test]
fn parse_single_file_works() {
    let output = cartograph()
        .arg("parse")
        .arg(parser_fixtures().join("component.tsx"))
        .output()
        .expect("binary runs");
    assert!(output.status.success());
}

#[test]
fn parse_rejects_a_non_typescript_file() {
    let output = cartograph()
        .args(["parse", "Cargo.toml"])
        .output()
        .expect("binary runs");
    assert!(!output.status.success());
}

// ── Python support (M02) ────────────────────────────────────────────

#[test]
fn parse_recognises_python_without_changing_the_invocation() {
    let output = cartograph()
        .arg("parse")
        .arg(python_fixtures())
        .output()
        .expect("binary runs");

    assert!(output.status.success(), "exit: {}", output.status);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("fastapi_routes.py"),
        "python file missed: {stdout}"
    );
    assert!(
        stdout.contains("Routes observed"),
        "route observations must be reported: {stdout}"
    );
    assert!(
        stdout.contains("not verified endpoints"),
        "routes must be labelled as observations, not endpoints: {stdout}"
    );
}

#[test]
fn parse_json_exposes_python_route_observations() {
    let output = cartograph()
        .args(["parse", "--json"])
        .arg(python_fixtures())
        .output()
        .expect("binary runs");

    let value: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("stdout is pure JSON");
    assert!(value["totals"]["route_observations"].as_u64().unwrap() >= 10);
    assert!(value["totals"]["http_observations"].as_u64().unwrap() >= 8);

    let has_route = value["files"].as_array().unwrap().iter().any(|f| {
        f.get("routes")
            .is_some_and(|r| !r.as_array().unwrap().is_empty())
    });
    assert!(has_route, "route observations absent from JSON");
}

#[test]
fn parse_walks_a_mixed_typescript_and_python_tree_in_one_pass() {
    // The fixtures directory holds .ts, .tsx and python/ side by side.
    let output = cartograph()
        .args(["parse", "--json"])
        .arg(parser_fixtures())
        .output()
        .expect("binary runs");

    let value: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let languages: std::collections::HashSet<&str> = value["files"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|f| f["language"].as_str())
        .collect();

    assert!(languages.contains("typescript"), "{languages:?}");
    assert!(languages.contains("tsx"), "{languages:?}");
    assert!(languages.contains("python"), "{languages:?}");
}

// ── Dynamic URL resolution reaches the CLI (M05) ────────────────────
//
// A regression test for a real defect: the resolver was wired into `match`,
// compiled, and every resolver unit test passed — while the command itself
// still ran the M04-only path, because one edit silently failed to apply.
// Only running the binary against a project caught it, so the guard belongs
// here rather than in the resolver's own tests.

fn dynamic_project() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("cartograph-resolver/tests/fixtures/dynamic-project")
}

#[test]
fn match_resolves_an_imported_constant_end_to_end() {
    let output = cartograph()
        .args(["match", "--json"])
        .arg(dynamic_project())
        .output()
        .expect("binary runs");
    assert!(output.status.success(), "exit: {}", output.status);

    let value: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("stdout is pure JSON");

    assert!(
        value["totals"]["dynamic_resolved"].as_u64().unwrap() >= 1,
        "the CLI must actually run the evaluator: {}",
        value["totals"]
    );
    assert!(
        value["totals"]["edges"].as_u64().unwrap() >= 1,
        "a resolved URL must reach an edge: {}",
        value["totals"]
    );

    let routes: Vec<&str> = value["matches"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|m| m["client_route"].as_str())
        .collect();
    assert!(
        routes.iter().any(|r| r.contains("/api/v1/orders")),
        "the imported constant must be substituted: {routes:?}"
    );
}

#[test]
fn match_still_refuses_an_environment_backed_url() {
    let output = cartograph()
        .args(["match", "--json"])
        .arg(dynamic_project())
        .output()
        .expect("binary runs");
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();

    let refused = value["matches"]
        .as_array()
        .unwrap()
        .iter()
        .any(|m| m["unsupported_reason"].as_str() == Some("ClientPrefixUnresolved"));
    assert!(
        refused,
        "an environment-backed prefix must still be refused after M05"
    );
}
