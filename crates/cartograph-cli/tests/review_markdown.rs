//! `cartograph diff --markdown` — the architecture review M14's Action posts.
//!
//! These run the compiled binary, because what matters is what a pull request
//! comment would actually contain: the bytes on stdout, the exit code, and
//! that nothing which must not appear in a public comment does.
//!
//! The review is *content*, not delivery. Slice 1 produces it; the Action that
//! checks out two trees and posts it is Slice 2, and nothing here pretends to
//! test posting.

use std::path::PathBuf;
use std::process::Command;

fn cartograph() -> Command {
    Command::new(env!("CARGO_BIN_EXE_cartograph"))
}

/// A pair of throwaway trees, removed when the test finishes.
struct Pair {
    root: PathBuf,
}

impl Pair {
    fn new(tag: &str) -> Self {
        let root = std::env::temp_dir().join(format!(
            "cartograph-review-{tag}-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        let _ = std::fs::remove_dir_all(&root);
        Self { root }
    }

    fn write(&self, side: &str, relative: &str, contents: &str) {
        let path = self.root.join(side).join(relative);
        std::fs::create_dir_all(path.parent().expect("parent")).expect("create");
        std::fs::write(path, contents).expect("write");
    }

    fn review(&self) -> (String, String, i32) {
        let output = cartograph()
            .arg("diff")
            .arg(self.root.join("before"))
            .arg(self.root.join("after"))
            .arg("--markdown")
            .output()
            .expect("the binary runs");
        (
            String::from_utf8_lossy(&output.stdout).into_owned(),
            String::from_utf8_lossy(&output.stderr).into_owned(),
            output.status.code().unwrap_or(-1),
        )
    }
}

impl Drop for Pair {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

const MODELS: &str = "from django.db import models

class Order(models.Model):
    class Meta:
        db_table = 'orders'
";

fn routes(handler: &str, touches_model: bool) -> String {
    let body = if touches_model {
        "    return Order.objects.all()"
    } else {
        "    return []"
    };
    format!(
        "from flask import Flask
from .models import Order

app = Flask(__name__)

@app.route('/api/orders')
def {handler}():
{body}
"
    )
}

/// Two trees that differ: the later one drops the ORM access.
fn differing(tag: &str) -> Pair {
    let pair = Pair::new(tag);
    pair.write("before", "api/models.py", MODELS);
    pair.write("before", "api/routes.py", &routes("list_orders", true));
    pair.write("after", "api/models.py", MODELS);
    pair.write("after", "api/routes.py", &routes("list_orders", false));
    pair
}

/// Two identical trees.
fn identical(tag: &str) -> Pair {
    let pair = Pair::new(tag);
    for side in ["before", "after"] {
        pair.write(side, "api/models.py", MODELS);
        pair.write(side, "api/routes.py", &routes("list_orders", true));
    }
    pair
}

// ── Shape ───────────────────────────────────────────────────────────────

#[test]
fn a_review_is_markdown_with_a_heading_and_exits_zero() {
    let pair = differing("shape");
    let (stdout, stderr, code) = pair.review();

    assert_eq!(code, 0, "a review is not a failure\n{stderr}");
    assert!(
        stdout.starts_with("## Cartograph — architecture review"),
        "a PR comment must announce itself:\n{stdout}"
    );
}

#[test]
fn an_unchanged_pair_says_so_plainly_rather_than_emitting_an_empty_comment() {
    let pair = identical("unchanged");
    let (stdout, _, code) = pair.review();

    assert_eq!(code, 0);
    assert!(
        stdout.contains("No architectural change"),
        "silence would read as a broken action:\n{stdout}"
    );
    // Nothing to list, so no tables.
    assert!(!stdout.contains("| From |"), "unexpected table:\n{stdout}");
}

#[test]
fn a_changed_pair_lists_what_moved_with_counts() {
    let pair = differing("counts");
    let (stdout, _, _) = pair.review();

    assert!(
        stdout.contains("removed"),
        "the dropped relationship must be visible:\n{stdout}"
    );
    assert!(
        stdout.contains("Removed relationships"),
        "a section heading is what makes a comment scannable:\n{stdout}"
    );
    assert!(
        stdout.contains("| From | To | Kind |"),
        "no table:\n{stdout}"
    );
}

// ── What a review must never claim ──────────────────────────────────────

/// Cartograph reports evidence and leaves judgement to the reader. A review
/// that graded a pull request would be asserting what the analysis cannot
/// support (RULE 006, RULE 009).
#[test]
fn a_review_passes_no_verdict() {
    let pair = differing("verdict");
    let (stdout, _, _) = pair.review();
    let lower = stdout.to_lowercase();

    for forbidden in [
        "approve",
        "reject",
        "lgtm",
        "risky",
        "unsafe",
        "score",
        "grade",
        "pass/fail",
        "blocker",
    ] {
        assert!(
            !lower.contains(forbidden),
            "a review must not say {forbidden:?}:\n{stdout}"
        );
    }
}

/// The caveat travels with the number, wherever the number appears.
#[test]
fn confidence_is_qualified_as_an_uncalibrated_prior() {
    let pair = differing("caveat");
    let (stdout, _, _) = pair.review();

    assert!(
        stdout.contains("uncalibrated prior"),
        "a bare confidence number invites thresholding:\n{stdout}"
    );
    assert!(stdout.contains("not a probability"));
}

/// A public comment must not leak the machine it ran on, and must not quote
/// source (RULE 015).
#[test]
fn a_review_carries_no_absolute_path() {
    let pair = differing("paths");
    let (stdout, _, _) = pair.review();

    assert!(!stdout.contains("C:\\"), "windows path leaked:\n{stdout}");
    assert!(
        !stdout.contains("/home/") && !stdout.contains("/Users/"),
        "home directory leaked:\n{stdout}"
    );
    // The temporary root the trees live under must not appear either.
    let root = pair.root.to_string_lossy().replace('\\', "/");
    assert!(
        !stdout.replace('\\', "/").contains(root.as_str()),
        "the analysed path leaked into the comment:\n{stdout}"
    );
}

// ── Determinism ─────────────────────────────────────────────────────────

/// A comment that reshuffled between runs would make every re-run look like a
/// change to anyone reading the pull request.
#[test]
fn repeated_runs_are_byte_identical() {
    let pair = differing("deterministic");
    let (first, _, _) = pair.review();

    for _ in 0..3 {
        let (again, _, _) = pair.review();
        assert_eq!(first, again, "the review changed between runs");
    }
}

// ── Table safety ────────────────────────────────────────────────────────

/// A `|` in an artefact name would split the row and misalign every column
/// after it. Python allows it in a string, so the analyser can meet one.
#[test]
fn a_pipe_in_a_name_cannot_break_the_table() {
    let pair = Pair::new("pipe");
    let models = "from django.db import models

class Order(models.Model):
    class Meta:
        db_table = 'or|ders'
";
    pair.write("before", "api/models.py", models);
    pair.write("before", "api/routes.py", &routes("list_orders", true));
    pair.write("after", "api/models.py", models);
    pair.write("after", "api/routes.py", &routes("list_orders", false));

    let (stdout, _, code) = pair.review();
    assert_eq!(code, 0);

    // Every table row must keep its column count. A raw `|` inside a cell
    // would add one.
    for line in stdout.lines() {
        if !line.starts_with("| ") || line.starts_with("|---") {
            continue;
        }
        let unescaped = line.matches('|').count() - line.matches("\\|").count();
        assert!(
            unescaped == 6 || unescaped == 7,
            "row has {unescaped} unescaped pipes, so the table is misaligned:\n{line}"
        );
    }
}

// ── Flag interaction ────────────────────────────────────────────────────

/// Two output forms at once is a usage error, caught by the parser rather than
/// by one silently winning.
#[test]
fn markdown_and_json_together_are_refused() {
    let pair = identical("conflict");
    let output = cartograph()
        .arg("diff")
        .arg(pair.root.join("before"))
        .arg(pair.root.join("after"))
        .arg("--markdown")
        .arg("--json")
        .output()
        .expect("runs");

    assert_eq!(output.status.code(), Some(2), "expected Usage");
    assert!(
        String::from_utf8_lossy(&output.stdout).is_empty(),
        "a refused invocation must not write to stdout"
    );
}

/// The existing forms are untouched by the addition.
#[test]
fn the_other_forms_still_work() {
    let pair = differing("forms");

    let text = cartograph()
        .arg("diff")
        .arg(pair.root.join("before"))
        .arg(pair.root.join("after"))
        .output()
        .expect("runs");
    assert_eq!(text.status.code(), Some(0));
    assert!(!String::from_utf8_lossy(&text.stdout).contains("## Cartograph"));

    let json = cartograph()
        .arg("diff")
        .arg(pair.root.join("before"))
        .arg(pair.root.join("after"))
        .arg("--json")
        .output()
        .expect("runs");
    assert_eq!(json.status.code(), Some(0));
    let document: serde_json::Value =
        serde_json::from_slice(&json.stdout).expect("json is still json");
    assert_eq!(document["command"], "diff");
    assert_eq!(document["schema_version"], "1.2");
}

/// An unreadable path is still an input error, and the review must not be
/// half-written to stdout when it fails.
#[test]
fn an_unreadable_path_exits_input_with_clean_stdout() {
    let pair = identical("bad-path");
    let output = cartograph()
        .args(["diff", "no/such/directory"])
        .arg(pair.root.join("after"))
        .arg("--markdown")
        .output()
        .expect("runs");

    assert_eq!(output.status.code(), Some(3), "expected Input");
    assert!(String::from_utf8_lossy(&output.stdout).is_empty());
}
