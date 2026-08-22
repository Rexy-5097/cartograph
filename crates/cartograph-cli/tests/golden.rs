//! Golden fixtures for CLI output.
//!
//! # Why byte comparison
//!
//! The assertions in `cli.rs` check that particular strings are present. That
//! catches a missing field; it does not catch a summary that quietly grows a
//! misleading line, loses its caveat, or reorders itself between runs. These
//! tests pin the whole document, so any change to what a user sees has to be
//! made deliberately and shows up in review as a diff of the output itself.
//!
//! # Normalisation
//!
//! Machine-specific values are replaced before comparison, because a fixture
//! containing this laptop's paths or a wall-clock duration would fail on every
//! other machine and teach the team to regenerate goldens without reading them
//! — which would make them worthless.
//!
//! Normalised: durations, the build commit, the target triple, and any
//! absolute path.
//!
//! # Regenerating
//!
//! ```bash
//! CARTOGRAPH_UPDATE_GOLDEN=1 cargo test -p cartograph-cli --test golden
//! ```
//!
//! Then **read the diff**. A golden test that is regenerated without being
//! read is a test that has been deleted.

use std::path::{Path, PathBuf};
use std::process::Command;

fn cartograph() -> Command {
    Command::new(env!("CARGO_BIN_EXE_cartograph"))
}

fn fixtures() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("cartograph-parser/tests/fixtures")
}

fn golden_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/golden")
}

/// Replaces everything that legitimately differs between machines and runs.
fn normalise(raw: &str) -> String {
    let mut text = raw.to_owned();

    // Absolute paths, longest first so a prefix does not mask a longer match.
    let manifest = env!("CARGO_MANIFEST_DIR");
    let workspace = Path::new(manifest).parent().and_then(Path::parent);
    if let Some(root) = workspace {
        text = text.replace(&root.to_string_lossy().to_string(), "<workspace>");
    }
    text = text.replace(manifest, "<manifest>");

    // Build metadata: correct to differ, wrong to pin.
    text = replace_after(&text, "commit ", "<commit>");
    text = replace_after(&text, "target ", "<target>");
    text = replace_json_field(&text, "\"commit\": \"", "<commit>");
    text = replace_json_field(&text, "\"target\": \"", "<target>");

    // Durations, in every form the CLI prints them.
    text = normalise_durations(&text);
    text
}

/// Replaces the remainder of a line that begins with `prefix`.
fn replace_after(text: &str, prefix: &str, with: &str) -> String {
    text.lines()
        .map(|line| {
            if line.starts_with(prefix) {
                format!("{prefix}{with}")
            } else {
                line.to_owned()
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
        + if text.ends_with('\n') { "\n" } else { "" }
}

/// Replaces a JSON string field's value.
fn replace_json_field(text: &str, opening: &str, with: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(index) = rest.find(opening) {
        let (before, after) = rest.split_at(index + opening.len());
        out.push_str(before);
        out.push_str(with);
        let Some(end) = after.find('"') else {
            rest = after;
            break;
        };
        rest = &after[end..];
    }
    out.push_str(rest);
    out
}

/// Replaces every duration and millisecond count with a fixed token.
fn normalise_durations(text: &str) -> String {
    let mut out = String::with_capacity(text.len());

    for (index, line) in text.lines().enumerate() {
        if index > 0 {
            out.push('\n');
        }
        let trimmed = line.trim_start();

        if let Some(rest) = trimmed.strip_prefix("duration") {
            // `  duration        11.78ms`
            let indent = &line[..line.len() - trimmed.len()];
            let spaces = rest.len() - rest.trim_start().len();
            out.push_str(indent);
            out.push_str("duration");
            out.push_str(&" ".repeat(spaces));
            out.push_str("<duration>");
            continue;
        }
        if trimmed.starts_with("Completed in") {
            out.push_str("Completed in       <duration>");
            continue;
        }
        if let Some(head) = trimmed.split_once(", analysed in ") {
            // `Repository fixtures — 6 nodes, 6 edges, analysed in 3.28ms.`
            out.push_str(head.0);
            out.push_str(", analysed in <duration>.");
            continue;
        }
        if let Some(head) = line.split_once("\"duration_ms\":") {
            out.push_str(head.0);
            out.push_str("\"duration_ms\": \"<duration>\",");
            continue;
        }
        if let Some(head) = line.split_once("\"elapsed_ms\":") {
            out.push_str(head.0);
            out.push_str("\"elapsed_ms\": \"<duration>\",");
            continue;
        }
        out.push_str(line);
    }
    if text.ends_with('\n') {
        out.push('\n');
    }
    out
}

/// Compares `actual` against the named fixture, or writes it when updating.
fn assert_golden(name: &str, actual: &str) {
    let path = golden_dir().join(name);
    let actual = normalise(actual);

    if std::env::var_os("CARTOGRAPH_UPDATE_GOLDEN").is_some() {
        std::fs::create_dir_all(golden_dir()).expect("golden directory");
        std::fs::write(&path, &actual).expect("writing golden");
        return;
    }

    let expected = std::fs::read_to_string(&path).unwrap_or_else(|_| {
        panic!(
            "missing golden fixture {}\n\
             regenerate with CARTOGRAPH_UPDATE_GOLDEN=1 cargo test -p cartograph-cli --test golden",
            path.display()
        )
    });

    assert_eq!(
        expected.trim_end(),
        actual.trim_end(),
        "\nCLI output changed. If the change is intended, regenerate with:\n  \
         CARTOGRAPH_UPDATE_GOLDEN=1 cargo test -p cartograph-cli --test golden\n\
         and read the diff before committing.\nFixture: {}\n",
        path.display()
    );
}

fn stdout_of(args: &[&str]) -> String {
    let mut command = cartograph();
    for arg in args {
        if *arg == "<fixtures>" {
            command.arg(fixtures());
        } else {
            command.arg(arg);
        }
    }
    let output = command.output().expect("binary runs");
    String::from_utf8_lossy(&output.stdout).into_owned()
}

fn stderr_of(args: &[&str]) -> String {
    let mut command = cartograph();
    for arg in args {
        if *arg == "<fixtures>" {
            command.arg(fixtures());
        } else {
            command.arg(arg);
        }
    }
    let output = command.output().expect("binary runs");
    String::from_utf8_lossy(&output.stderr).into_owned()
}

// ── the fixtures ────────────────────────────────────────────────────

#[test]
fn golden_version() {
    assert_golden("version.txt", &stdout_of(&["version"]));
}

#[test]
fn golden_version_json() {
    assert_golden("version.json", &stdout_of(&["version", "--json"]));
}

#[test]
fn golden_summary() {
    assert_golden("summary.txt", &stdout_of(&["<fixtures>"]));
}

#[test]
fn golden_summary_json() {
    assert_golden("summary.json", &stdout_of(&["--json", "<fixtures>"]));
}

#[test]
fn golden_trace_incomplete_chain() {
    // This chain stops before any database table, which is the case the
    // output must not overstate.
    assert_golden(
        "trace-incomplete.txt",
        &stdout_of(&["trace", "http.ts", "--path", "<fixtures>"]),
    );
}

#[test]
fn golden_trace_json() {
    assert_golden(
        "trace.json",
        &stdout_of(&["trace", "http.ts", "--json", "--path", "<fixtures>"]),
    );
}

#[test]
fn golden_error_missing_path() {
    assert_golden(
        "error-missing-path.txt",
        &stderr_of(&["definitely/not/a/repository"]),
    );
}

#[test]
fn golden_error_unknown_symbol() {
    assert_golden(
        "error-unknown-symbol.txt",
        &stderr_of(&["trace", "NoSuchSymbolAnywhere", "--path", "<fixtures>"]),
    );
}

#[test]
fn golden_error_json() {
    assert_golden(
        "error.json",
        &stdout_of(&[
            "trace",
            "NoSuchSymbolAnywhere",
            "--json",
            "--path",
            "<fixtures>",
        ]),
    );
}

// ── the normaliser itself ───────────────────────────────────────────

#[test]
fn normalisation_removes_every_machine_specific_value() {
    let raw = stdout_of(&["--json", "<fixtures>"]);
    let normalised = normalise(&raw);

    assert!(
        !normalised.contains("/Volumes/"),
        "an absolute path survived normalisation"
    );
    assert!(
        !normalised.contains(env!("CARGO_MANIFEST_DIR")),
        "the manifest directory survived normalisation"
    );
    assert!(
        normalised.contains("<duration>"),
        "the duration was not normalised: {normalised}"
    );
}

#[test]
fn normalisation_is_idempotent() {
    // A normaliser that changed its own output would make every fixture
    // depend on how many times it had been applied.
    let raw = stdout_of(&["version"]);
    let once = normalise(&raw);
    assert_eq!(once, normalise(&once));
}
