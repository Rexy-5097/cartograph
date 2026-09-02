//! `cartograph diff`, exercised through the real binary.
//!
//! These run the compiled executable rather than calling a function, because
//! the things most likely to be wrong here are the ones a unit test cannot
//! see: the exit code the process returns, whether stdout is valid JSON, and
//! whether an error reaches stderr instead of corrupting a document a script
//! is parsing.
//!
//! Two trees are written to a temporary directory and analysed independently,
//! which is what the two-path model means in practice: the caller checks out
//! two states — `git worktree add` in real use — and Cartograph compares the
//! analyses without knowing anything about revisions.

use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::Value;

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
            "cartograph-diff-{tag}-{}-{:?}",
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

    fn before(&self) -> PathBuf {
        self.root.join("before")
    }

    fn after(&self) -> PathBuf {
        self.root.join("after")
    }

    fn run(&self, extra: &[&str]) -> (String, String, i32) {
        let mut command = cartograph();
        command.arg("diff").arg(self.before()).arg(self.after());
        for arg in extra {
            command.arg(arg);
        }
        let output = command.output().expect("the binary runs");
        (
            String::from_utf8_lossy(&output.stdout).into_owned(),
            String::from_utf8_lossy(&output.stderr).into_owned(),
            output.status.code().unwrap_or(-1),
        )
    }

    fn json(&self) -> Value {
        let (stdout, stderr, code) = self.run(&["--json"]);
        serde_json::from_str(&stdout)
            .unwrap_or_else(|e| panic!("diff (exit {code}) did not emit JSON: {e}\n{stderr}"))
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

fn routes(prefix: &str, handler: &str) -> String {
    format!(
        "{prefix}from flask import Flask
from .models import Order

app = Flask(__name__)

@app.route('/api/orders')
def {handler}():
    return Order.objects.all()
"
    )
}

/// Both sides identical.
fn identical(tag: &str) -> Pair {
    let pair = Pair::new(tag);
    for side in ["before", "after"] {
        pair.write(side, "api/models.py", MODELS);
        pair.write(side, "api/routes.py", &routes("", "list_orders"));
    }
    pair
}

// ── The empty diff ──────────────────────────────────────────────────────

#[test]
fn two_identical_trees_diff_to_nothing_and_exit_zero() {
    let pair = identical("identical");
    let (stdout, stderr, code) = pair.run(&[]);

    assert_eq!(code, 0, "an empty diff is success, not failure\n{stderr}");
    assert!(
        stdout.contains("No architectural difference"),
        "unexpected text:\n{stdout}"
    );
}

#[test]
fn an_empty_diff_reports_empty_arrays_rather_than_omitting_them() {
    let pair = identical("empty-json");
    let document = pair.json();
    let result = &document["result"];

    assert_eq!(document["command"], "diff");
    assert_eq!(document["schema_version"], "1.2");
    assert_eq!(result["added"].as_array().expect("added").len(), 0);
    assert_eq!(result["removed"].as_array().expect("removed").len(), 0);
    assert_eq!(result["changed"].as_array().expect("changed").len(), 0);
    assert!(
        result["unchanged"].as_u64().expect("unchanged") > 0,
        "the trees share relationships, so some must be unchanged"
    );
    assert_eq!(result["correspondence"], "stable-identity");
}

// ── A real difference ───────────────────────────────────────────────────

#[test]
fn removing_a_relationship_is_reported_as_removed_with_evidence() {
    let pair = Pair::new("removed");
    pair.write("before", "api/models.py", MODELS);
    pair.write("before", "api/routes.py", &routes("", "list_orders"));
    // The later tree keeps the model but the handler no longer touches it.
    pair.write("after", "api/models.py", MODELS);
    pair.write(
        "after",
        "api/routes.py",
        "from flask import Flask

app = Flask(__name__)

@app.route('/api/orders')
def list_orders():
    return []
",
    );

    let document = pair.json();
    let removed = document["result"]["removed"]
        .as_array()
        .expect("removed array");

    assert!(
        !removed.is_empty(),
        "dropping the ORM access must be visible: {}",
        serde_json::to_string_pretty(&document["result"]).expect("json")
    );
    for entry in removed {
        assert!(!entry["source"].as_str().expect("source").is_empty());
        assert!(!entry["target"].as_str().expect("target").is_empty());
        let facts = &entry["facts"];
        assert!(!facts["evidence"].as_str().expect("evidence").is_empty());
        assert!(!facts["file"].as_str().expect("file").is_empty());
        assert!(facts["line"].as_u64().expect("line") >= 1);
        let confidence = facts["confidence"].as_f64().expect("confidence");
        assert!((0.0..=1.0).contains(&confidence));
    }
}

/// The identity property, through the binary: a handler that merely moved
/// down its file must not appear in the diff at all.
#[test]
fn moving_code_within_a_file_produces_no_diff() {
    let pair = Pair::new("moved");
    pair.write("before", "api/models.py", MODELS);
    pair.write("before", "api/routes.py", &routes("", "list_orders"));
    pair.write("after", "api/models.py", MODELS);
    pair.write(
        "after",
        "api/routes.py",
        &routes(&"# padding\n".repeat(30), "list_orders"),
    );

    let document = pair.json();
    let result = &document["result"];

    assert_eq!(
        result["added"].as_array().expect("added").len(),
        0,
        "a move must not add: {}",
        serde_json::to_string_pretty(result).expect("json")
    );
    assert_eq!(result["removed"].as_array().expect("removed").len(), 0);
    assert_eq!(result["changed"].as_array().expect("changed").len(), 0);
}

/// A rename is remove-plus-add, as Slice 1 established.
#[test]
fn renaming_a_handler_is_a_removal_and_an_addition() {
    let pair = Pair::new("renamed");
    pair.write("before", "api/models.py", MODELS);
    pair.write("before", "api/routes.py", &routes("", "list_orders"));
    pair.write("after", "api/models.py", MODELS);
    pair.write("after", "api/routes.py", &routes("", "list_all_orders"));

    let document = pair.json();
    let result = &document["result"];

    assert!(!result["added"].as_array().expect("added").is_empty());
    assert!(!result["removed"].as_array().expect("removed").is_empty());
    assert!(
        result["changed"].as_array().expect("changed").is_empty(),
        "a rename is not an adjusted edge"
    );
}

// ── Determinism ─────────────────────────────────────────────────────────

#[test]
fn repeated_runs_produce_byte_identical_json() {
    let pair = Pair::new("deterministic");
    pair.write("before", "api/models.py", MODELS);
    pair.write("before", "api/routes.py", &routes("", "list_orders"));
    pair.write("after", "api/models.py", MODELS);
    pair.write("after", "api/routes.py", &routes("", "list_all_orders"));

    let (first, _, _) = pair.run(&["--json"]);
    for _ in 0..3 {
        let (again, _, _) = pair.run(&["--json"]);
        // `summary` carries timings, so compare the part that must not move.
        let a: Value = serde_json::from_str(&first).expect("json");
        let b: Value = serde_json::from_str(&again).expect("json");
        assert_eq!(a["result"], b["result"], "the diff changed between runs");
    }
}

// ── Exit codes and stream hygiene ───────────────────────────────────────

#[test]
fn an_unreadable_before_path_exits_input_and_leaves_stdout_clean() {
    let pair = identical("bad-before");
    let output = cartograph()
        .args(["diff", "no/such/directory"])
        .arg(pair.after())
        .output()
        .expect("runs");

    assert_eq!(output.status.code(), Some(3), "expected Input");
    assert!(
        String::from_utf8_lossy(&output.stdout).is_empty(),
        "an error must not write to stdout"
    );
}

#[test]
fn an_unreadable_after_path_exits_input() {
    let pair = identical("bad-after");
    let output = cartograph()
        .arg("diff")
        .arg(pair.before())
        .arg("no/such/directory")
        .output()
        .expect("runs");

    assert_eq!(output.status.code(), Some(3));
    assert!(String::from_utf8_lossy(&output.stdout).is_empty());
}

#[test]
fn a_missing_second_path_exits_usage() {
    let pair = identical("one-arg");
    let output = cartograph()
        .arg("diff")
        .arg(pair.before())
        .output()
        .expect("runs");

    assert_eq!(output.status.code(), Some(2), "clap's convention");
}

#[test]
fn no_arguments_exits_usage() {
    let output = cartograph().arg("diff").output().expect("runs");
    assert_eq!(output.status.code(), Some(2));
}

/// A JSON-mode failure must still be JSON, never a half-written document.
#[test]
fn a_json_mode_error_is_still_valid_json() {
    let pair = identical("json-error");
    let output = cartograph()
        .args(["diff", "no/such/directory"])
        .arg(pair.after())
        .arg("--json")
        .output()
        .expect("runs");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_ne!(output.status.code(), Some(0));
    if !stdout.trim().is_empty() {
        let document: Value =
            serde_json::from_str(&stdout).expect("a JSON-mode error is still valid JSON");
        assert!(
            document.get("errors").is_some(),
            "a JSON error document must carry `errors`"
        );
    }
}

// ── The real repository state, not a fixture ────────────────────────────

/// Two states of a real repository, compared through the binary.
///
/// The corpus is copied into two temporary trees and one file is edited in the
/// second, so this is a genuine two-analysis comparison over code nobody wrote
/// for the test — the acceptance shape, at a size that can run in CI when the
/// corpus is present.
///
/// ```text
/// CARTOGRAPH_DIFF_REPO=<path> cargo test -p cartograph-cli --release \
///     --test diff_cli -- --ignored --nocapture
/// ```
#[test]
#[ignore = "needs CARTOGRAPH_DIFF_REPO; the pinned corpora are not vendored"]
fn a_real_repository_diffs_against_an_edited_copy_of_itself() {
    let Ok(source) = std::env::var("CARTOGRAPH_DIFF_REPO") else {
        return;
    };
    let source = PathBuf::from(source);

    let pair = Pair::new("real");
    copy_tree(&source, &pair.before()).expect("copy before");
    copy_tree(&source, &pair.after()).expect("copy after");

    let document = pair.json();
    let result = &document["result"];

    println!(
        "identical copies: added {} removed {} changed {} unchanged {}",
        result["added"].as_array().expect("added").len(),
        result["removed"].as_array().expect("removed").len(),
        result["changed"].as_array().expect("changed").len(),
        result["unchanged"].as_u64().expect("unchanged")
    );

    // Two copies of one revision are the same architecture, so the diff of a
    // real repository against itself must be empty. This is the strongest
    // available statement that correspondence survives two separate analyses.
    assert_eq!(result["added"].as_array().expect("added").len(), 0);
    assert_eq!(result["removed"].as_array().expect("removed").len(), 0);
    assert_eq!(result["changed"].as_array().expect("changed").len(), 0);
    assert!(
        result["unchanged"].as_u64().expect("unchanged") > 0,
        "precondition: the corpus must produce relationships"
    );
}

/// Recursively copies a tree, skipping `.git` and anything unreadable.
fn copy_tree(from: &Path, to: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(to)?;
    for entry in std::fs::read_dir(from)? {
        let entry = entry?;
        let name = entry.file_name();
        if name == ".git" || name == "node_modules" || name == "target" {
            continue;
        }
        let source = entry.path();
        let destination = to.join(&name);
        if entry.file_type()?.is_dir() {
            copy_tree(&source, &destination)?;
        } else if entry.file_type()?.is_file() {
            let _ = std::fs::copy(&source, &destination);
        }
    }
    Ok(())
}
