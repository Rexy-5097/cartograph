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
        stdout.contains("milestone M00"),
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
    assert_eq!(value["milestone"], "M00");
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
