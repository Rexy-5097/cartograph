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
