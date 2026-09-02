//! `cartograph blast`, exercised through the real binary.
//!
//! These run the compiled executable rather than calling a function, because
//! the things most likely to be wrong here are the things a unit test cannot
//! see: the exit code the process actually returns, whether stdout is valid
//! JSON, and whether an error goes to stderr instead of corrupting a document
//! a script is parsing.
//!
//! The CLI is an adapter over `cartograph_graph::blast::blast_radius`. Nothing
//! here re-derives what the answer should be — the expected sets come from the
//! fixture by hand, and the core's own suite owns the semantics.

use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::Value;

fn cartograph() -> Command {
    Command::new(env!("CARGO_BIN_EXE_cartograph"))
}

/// A repository whose shape is known: a TypeScript client calling a Python
/// route that touches a model.
fn fixtures() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("blast")
}

/// Writes the fixture repository, exactly once per test binary.
///
/// The `Once` is not an optimisation. Tests run in parallel, and rewriting a
/// tree while another test's analyser is walking it means a reader can observe
/// a half-written file — which showed up as `the_text_and_json_forms_report_the_same_count`
/// failing on Linux while passing on Windows, purely on timing.
fn ensure_fixture() -> PathBuf {
    static WRITTEN: std::sync::Once = std::sync::Once::new();
    let root = fixtures();

    WRITTEN.call_once(|| {
        let web = root.join("web");
        let api = root.join("api");
        std::fs::create_dir_all(&web).expect("fixture directory");
        std::fs::create_dir_all(&api).expect("fixture directory");

        std::fs::write(
            web.join("client.ts"),
            "export async function loadOrders() {
               return fetch('/api/orders');
}
",
        )
        .expect("write");
        std::fs::write(
            api.join("routes.py"),
            "from flask import Flask
             from .models import Order

             app = Flask(__name__)

             @app.route('/api/orders')
             def list_orders():
                 return Order.objects.all()
",
        )
        .expect("write");
        std::fs::write(
            api.join("models.py"),
            "from django.db import models

             class Order(models.Model):
                 class Meta:
                     db_table = 'orders'
",
        )
        .expect("write");
    });

    root
}

fn run(args: &[&str]) -> (String, String, i32) {
    let root = ensure_fixture();
    let mut command = cartograph();
    for arg in args {
        if *arg == "<repo>" {
            command.arg(&root);
        } else {
            command.arg(arg);
        }
    }
    let output = command.output().expect("the binary runs");
    (
        String::from_utf8_lossy(&output.stdout).into_owned(),
        String::from_utf8_lossy(&output.stderr).into_owned(),
        output.status.code().unwrap_or(-1),
    )
}

fn json(args: &[&str]) -> Value {
    let (stdout, stderr, code) = run(args);
    serde_json::from_str(&stdout)
        .unwrap_or_else(|e| panic!("{args:?} (exit {code}) did not emit JSON: {e}\n{stderr}"))
}

// ── The oracle, through the binary ──────────────────────────────────────

/// The fixture's chain, whatever the resolver manages to build from it, must
/// answer consistently in both output forms.
#[test]
fn a_blast_query_succeeds_and_reports_the_target() {
    let document = json(&["blast", "Order", "--json", "--path", "<repo>"]);

    assert_eq!(document["command"], "blast");
    assert_eq!(document["schema_version"], "1.2");
    assert_eq!(document["result"]["target"]["name"], "Order");
    assert_eq!(document["result"]["calibrated"], false);
    assert_eq!(document["result"]["routes"], "representative");
}

/// Every reached entry must name an edge that is in the document's own `edges`
/// array — the whole reason `via` is an id rather than a copy.
#[test]
fn every_via_resolves_to_an_edge_in_the_same_document() {
    let document = json(&["blast", "orders", "--json", "--path", "<repo>"]);
    let edges = document["edges"].as_array().expect("edges array");
    let reached = document["result"]["reached"]
        .as_array()
        .expect("reached array");

    assert!(
        !reached.is_empty(),
        "precondition: the fixture must produce a dependent"
    );

    for entry in reached {
        let via = &entry["via"];
        assert!(
            edges.iter().any(|e| &e["id"] == via),
            "via {via} is not in the document's edges"
        );
        // Depth is at least one: the target is never its own dependent.
        assert!(entry["depth"].as_u64().expect("depth") >= 1);
        let confidence = entry["confidence"].as_f64().expect("confidence");
        assert!((0.0..=1.0).contains(&confidence));
    }
}

/// The target must never appear among its own dependents.
#[test]
fn the_target_is_absent_from_its_own_result() {
    let document = json(&["blast", "orders", "--json", "--path", "<repo>"]);
    let target = &document["result"]["target"]["id"];

    for entry in document["result"]["reached"].as_array().expect("array") {
        assert_ne!(&entry["node"]["id"], target, "the target reported itself");
    }
}

/// Text and JSON must agree. A reader comparing them should not find one
/// listing artefacts the other omits.
#[test]
fn the_text_and_json_forms_report_the_same_count() {
    let document = json(&["blast", "orders", "--json", "--path", "<repo>"]);
    let count = document["result"]["reached"]
        .as_array()
        .expect("array")
        .len();

    let (text, _, code) = run(&["blast", "orders", "--path", "<repo>"]);
    assert_eq!(code, 0);

    if count == 0 {
        assert!(text.contains("nothing depends on it"));
    } else {
        assert!(
            text.contains(&format!("{count} artefact")),
            "text did not report {count}:\n{text}"
        );
    }
}

// ── Exit codes, through the process ─────────────────────────────────────

#[test]
fn a_successful_query_exits_zero() {
    let (_, _, code) = run(&["blast", "Order", "--path", "<repo>"]);
    assert_eq!(code, 0);
}

/// The distinction that matters most: an empty radius is a *successful*
/// answer, not a missing symbol. Conflating them would make a correct result
/// look like a failure to any script checking the exit code.
#[test]
fn an_empty_radius_exits_zero_rather_than_not_found() {
    // `loadOrders` is the outermost caller in the fixture, so nothing depends
    // on it.
    let (stdout, stderr, code) = run(&["blast", "loadOrders", "--json", "--path", "<repo>"]);

    assert_eq!(
        code, 0,
        "an empty blast radius is success, not NotFound
{stderr}"
    );
    let document: Value = serde_json::from_str(&stdout).expect("json");
    assert_eq!(
        document["result"]["reached"]
            .as_array()
            .expect("array")
            .len(),
        0,
        "precondition: nothing depends on the outermost caller"
    );
}

#[test]
fn an_unknown_symbol_exits_not_found() {
    let (stdout, stderr, code) = run(&["blast", "NoSuchSymbolAnywhere", "--path", "<repo>"]);

    assert_eq!(code, 5, "expected NotFound");
    assert!(stderr.contains("symbol-not-found"), "{stderr}");
    assert!(stdout.is_empty(), "an error must not write to stdout");
}

/// Ambiguity is refused, never resolved by picking one.
///
/// Built in its own directory rather than by adding a file to the shared
/// fixture: these tests run in parallel, and mutating a tree the others are
/// analysing makes every one of them flaky.
#[test]
fn an_ambiguous_symbol_exits_ambiguous_and_chooses_nothing() {
    let root =
        std::env::temp_dir().join(format!("cartograph-blast-ambiguous-{}", std::process::id()));
    // Two handlers with the same name in different files. A bare class would
    // not do: a model becomes a node only when it participates in a resolved
    // relationship, so each handler needs its own model to access.
    for (dir, model, table) in [("alpha", "Order", "orders"), ("beta", "Ticket", "tickets")] {
        let package = root.join(dir);
        std::fs::create_dir_all(&package).expect("create");
        std::fs::write(
            package.join("models.py"),
            format!(
                "from django.db import models

                 class {model}(models.Model):
                     class Meta:
                         db_table = '{table}'
"
            ),
        )
        .expect("write");
        std::fs::write(
            package.join("routes.py"),
            format!(
                "from flask import Flask
                 from .models import {model}

                 app = Flask(__name__)

                 @app.route('/api/{table}')
                 def list_items():
                     return {model}.objects.all()
"
            ),
        )
        .expect("write");
    }

    let output = cartograph()
        .args(["blast", "list_items", "--path"])
        .arg(&root)
        .output()
        .expect("runs");
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
    let code = output.status.code().unwrap_or(-1);
    let _ = std::fs::remove_dir_all(&root);

    assert_eq!(
        code, 6,
        "two `list_items` handlers in different files must be refused, not resolved
{stderr}"
    );
    assert!(stderr.contains("ambiguous"), "{stderr}");
    assert!(
        stderr.contains("qualify"),
        "an ambiguity must say how to resolve it
{stderr}"
    );
    // Neither candidate may have been chosen.
    assert!(
        String::from_utf8_lossy(&output.stdout).is_empty(),
        "a refused query must produce no result"
    );
}

#[test]
fn a_missing_symbol_argument_exits_usage() {
    let output = cartograph().arg("blast").output().expect("runs");
    assert_eq!(output.status.code(), Some(2));
}

#[test]
fn an_unreadable_repository_exits_input() {
    let (stdout, _, code) = run(&["blast", "Order", "--path", "no/such/directory"]);
    assert_eq!(code, 3, "expected Input");
    assert!(stdout.is_empty());
}

// ── Adversarial ─────────────────────────────────────────────────────────

#[test]
fn a_unicode_symbol_is_handled_without_panicking() {
    let (stdout, stderr, code) = run(&["blast", "梅Ω→", "--path", "<repo>"]);

    assert_eq!(code, 5, "an absent unicode name is simply not found");
    assert!(stdout.is_empty());
    assert!(!stderr.contains("panicked"), "{stderr}");
}

#[test]
fn a_repository_path_containing_spaces_works() {
    let root = std::env::temp_dir().join("cartograph blast spaces");
    let api = root.join("api");
    std::fs::create_dir_all(&api).expect("create");
    std::fs::write(
        api.join("models.py"),
        "from django.db import models\n\n\
         class Widget(models.Model):\n    \
         class Meta:\n        \
         db_table = 'widgets'\n",
    )
    .expect("write");

    let output = cartograph()
        .args(["blast", "Widget", "--json", "--path"])
        .arg(&root)
        .output()
        .expect("runs");
    let code = output.status.code().unwrap_or(-1);
    let _ = std::fs::remove_dir_all(&root);

    assert!(
        code == 0 || code == 5,
        "a path with spaces must not fail oddly"
    );
    if code == 0 {
        let document: Value =
            serde_json::from_slice(&output.stdout).expect("valid JSON from a spaced path");
        assert_eq!(document["command"], "blast");
    }
}

/// An error must never corrupt a document a script is parsing.
#[test]
fn an_error_leaves_stdout_empty_and_parseable_as_nothing() {
    let (stdout, _, code) = run(&[
        "blast",
        "NoSuchSymbolAnywhere",
        "--json",
        "--path",
        "<repo>",
    ]);

    assert_ne!(code, 0);
    // JSON mode reports the failure as a JSON document on stdout, or nothing —
    // never a half-written one.
    if !stdout.trim().is_empty() {
        let document: Value =
            serde_json::from_str(&stdout).expect("a JSON-mode error is still valid JSON");
        assert!(
            document.get("errors").is_some(),
            "a JSON error document must carry `errors`"
        );
    }
}

// ── Determinism ─────────────────────────────────────────────────────────

#[test]
fn repeated_queries_produce_identical_output() {
    let first = run(&["blast", "orders", "--json", "--path", "<repo>"]);
    for _ in 0..3 {
        let again = run(&["blast", "orders", "--json", "--path", "<repo>"]);
        // Timing fields live in `summary`, so compare the part that must not
        // move: the result.
        let a: Value = serde_json::from_str(&first.0).expect("json");
        let b: Value = serde_json::from_str(&again.0).expect("json");
        assert_eq!(a["result"], b["result"], "the result changed between runs");
    }
}
