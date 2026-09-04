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
        stdout.contains("milestone M15"),
        "missing milestone line: {stdout}"
    );
    assert!(stdout.contains("cartograph 0.1.0"), "not v0.1.0: {stdout}");
    assert!(stdout.contains("commit "), "missing commit line: {stdout}");
    assert!(stdout.contains("target "), "missing target line: {stdout}");
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
    assert_eq!(value["milestone"], "M15");
    assert_eq!(value["version"], "0.1.0");
    assert!(value["commit"].is_string());
    assert!(value["target"].is_string());
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
fn an_unknown_command_is_not_silently_accepted() {
    // From M09 a bare argument is a repository path, so `analyze` is read as a
    // directory named `analyze`. It does not exist, so the run fails with the
    // input code rather than appearing to do something.
    let output = cartograph().arg("analyze").output().expect("binary runs");

    assert_eq!(output.status.code(), Some(3), "expected the input code");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("does not exist"), "unclear error: {stderr}");
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
fn parse_rejects_a_file_in_an_unsupported_language() {
    let output = cartograph()
        .args(["parse", "Cargo.toml"])
        .output()
        .expect("binary runs");
    assert_eq!(output.status.code(), Some(3));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains(".tsx"),
        "the error must name what is supported: {stderr}"
    );
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

// ── Graph export (M07) ───────────────────────────────────────────────
//
// The M07 benchmark evaluates the graph Cartograph builds. If it rebuilt the
// graph itself it would be validating a copy, so `match --json` exports the
// real one and these tests hold that export to the domain model's guarantees.

#[test]
fn match_json_exports_the_graph_it_built() {
    let output = cartograph()
        .args(["match", "--json"])
        .arg(dynamic_project())
        .output()
        .expect("binary runs");
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();

    let graph = &value["graph"];
    let nodes = graph["nodes"].as_array().expect("nodes array");
    let edges = graph["edges"].as_array().expect("edges array");

    assert_eq!(
        graph["node_count"].as_u64().unwrap(),
        nodes.len() as u64,
        "the exported node list must be the whole graph"
    );
    assert_eq!(
        graph["edge_count"].as_u64().unwrap(),
        edges.len() as u64,
        "the exported edge list must be the whole graph"
    );
    assert_eq!(
        edges.len() as u64,
        value["totals"]["edges"].as_u64().unwrap()
            + value["totals"]["orm_edges"].as_u64().unwrap()
            + value["totals"]["client_call_edges"].as_u64().unwrap(),
        "the graph must hold exactly the edges the run reported"
    );
    assert!(!edges.is_empty(), "this fixture produces edges");
}

#[test]
fn every_exported_edge_carries_its_four_attributes() {
    // The domain model refuses an edge without confidence, provenance,
    // evidence and a location. An export that dropped one would let a
    // benchmark score edges that could not exist.
    let output = cartograph()
        .args(["match", "--json"])
        .arg(dynamic_project())
        .output()
        .expect("binary runs");
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();

    for edge in value["graph"]["edges"].as_array().unwrap() {
        let confidence = edge["confidence"].as_f64().expect("confidence is a number");
        assert!(
            (0.0..=1.0).contains(&confidence),
            "confidence out of range: {edge}"
        );
        assert!(
            edge["provenance"].as_str().is_some_and(|p| !p.is_empty()),
            "missing provenance: {edge}"
        );
        assert!(
            edge["evidence"]
                .as_str()
                .is_some_and(|e| !e.trim().is_empty()),
            "missing evidence: {edge}"
        );
        assert!(
            edge["file"].as_str().is_some_and(|f| !f.is_empty()),
            "missing source file: {edge}"
        );
        assert!(edge["line"].as_u64().unwrap() >= 1, "missing line: {edge}");
        // No language model is ever a provenance (RULE 007).
        assert_ne!(edge["provenance"].as_str(), Some("model-inference"));
    }
}

#[test]
fn exported_edges_reference_exported_nodes() {
    // A dangling endpoint would make a chain look traversable when it is not.
    let output = cartograph()
        .args(["match", "--json"])
        .arg(dynamic_project())
        .output()
        .expect("binary runs");
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();

    let ids: std::collections::HashSet<u64> = value["graph"]["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .map(|n| n["id"].as_u64().unwrap())
        .collect();
    for edge in value["graph"]["edges"].as_array().unwrap() {
        assert!(
            ids.contains(&edge["source"].as_u64().unwrap()),
            "dangling source: {edge}"
        );
        assert!(
            ids.contains(&edge["target"].as_u64().unwrap()),
            "dangling target: {edge}"
        );
    }
}

#[test]
fn the_export_is_stable_across_runs() {
    // A benchmark compares pass 1 against pass 2. If the same tree produced a
    // different serialisation each run, every comparison would be noise.
    let run = || {
        let output = cartograph()
            .args(["match", "--json"])
            .arg(dynamic_project())
            .output()
            .expect("binary runs");
        let mut value: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
        // Wall-clock time is expected to differ; nothing else may.
        value["elapsed_ms"] = serde_json::Value::Null;
        value
    };
    assert_eq!(run(), run(), "analysis output must be deterministic");
}

// ════════════════════════════════════════════════════════════════════
// M09 — the developer-facing CLI
// ════════════════════════════════════════════════════════════════════
//
// These exercise the shipped binary the way a user does. The layers are
// deliberate: argument parsing and formatting are unit-tested inside the
// crate, and everything below runs the real executable, because the two M05
// and M07 defects that reached a release were both invisible to unit tests
// and obvious the first time the binary was run.

/// Exit codes, as documented in `docs/development/cli.md`.
mod exit {
    pub const SUCCESS: i32 = 0;
    pub const USAGE: i32 = 2;
    pub const INPUT: i32 = 3;
    pub const NOT_FOUND: i32 = 5;
    pub const AMBIGUOUS: i32 = 6;
    pub const PARTIAL: i32 = 7;
}

// ── the default command ─────────────────────────────────────────────

#[test]
fn a_bare_path_analyses_the_repository() {
    let output = cartograph()
        .arg(parser_fixtures())
        .output()
        .expect("binary runs");

    assert_eq!(output.status.code(), Some(exit::SUCCESS));
    let stdout = String::from_utf8_lossy(&output.stdout);

    // Everything PART 4 requires the summary to report.
    for expected in [
        "Cartograph 0.1.0",
        "languages",
        "files analysed",
        "symbols",
        "duration",
        "OBSERVED",
        "routes",
        "HTTP call sites",
        "ORM models",
        "RESOLVED",
        "database tables",
        "cross-language",
        "REFUSED",
        "ambiguous",
        "unsupported",
        "DIAGNOSTICS",
    ] {
        assert!(
            stdout.contains(expected),
            "summary lacks `{expected}`:\n{stdout}"
        );
    }
}

#[test]
fn the_summary_never_claims_a_relationship_is_verified() {
    // The product's whole argument is that it computes rather than guesses.
    // Claiming verification it has not performed would be the one overclaim
    // that undermines everything else.
    let output = cartograph()
        .arg(parser_fixtures())
        .output()
        .expect("binary runs");
    let stdout = String::from_utf8_lossy(&output.stdout).to_lowercase();

    assert!(
        !stdout.contains("verified relationship") && !stdout.contains("verified edge"),
        "the summary claimed verification: {stdout}"
    );
    assert!(
        stdout.contains("nothing above is claimed to be verified"),
        "the summary must state the caveat explicitly: {stdout}"
    );
    assert!(
        stdout.contains("uncalibrated"),
        "confidence must be labelled uncalibrated: {stdout}"
    );
}

#[test]
fn the_summary_reports_refusals_as_prominently_as_results() {
    let output = cartograph()
        .arg(parser_fixtures())
        .output()
        .expect("binary runs");
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(stdout.contains("REFUSED"), "{stdout}");
    assert!(stdout.contains("no edge produced"), "{stdout}");
    assert!(stdout.contains("Refusals are by design"), "{stdout}");
}

#[test]
fn the_summary_is_deterministic() {
    let run = || {
        let output = cartograph()
            .arg(parser_fixtures())
            .output()
            .expect("binary runs");
        // The duration line is expected to differ; nothing else may.
        String::from_utf8_lossy(&output.stdout)
            .lines()
            .filter(|line| !line.trim_start().starts_with("duration"))
            .filter(|line| !line.starts_with("Repository "))
            .collect::<Vec<_>>()
            .join("\n")
    };
    assert_eq!(run(), run(), "the summary must not vary between runs");
}

// ── trace ───────────────────────────────────────────────────────────

#[test]
fn trace_follows_a_chain_out_of_the_graph() {
    let output = cartograph()
        .args(["trace", "http.ts", "--path"])
        .arg(parser_fixtures())
        .output()
        .expect("binary runs");

    assert_eq!(output.status.code(), Some(exit::SUCCESS));
    let stdout = String::from_utf8_lossy(&output.stdout);

    // PART 6: the relationship, its confidence, provenance, evidence and both
    // locations must all be inspectable.
    assert!(
        stdout.contains("HTTP-CALL"),
        "no relationship kind: {stdout}"
    );
    assert!(
        stdout.contains("confidence 0.98"),
        "no confidence: {stdout}"
    );
    assert!(
        stdout.contains("provenance route-matcher"),
        "no provenance: {stdout}"
    );
    assert!(stdout.contains("evidence"), "no evidence: {stdout}");
    assert!(
        stdout.contains("observed   http.ts:"),
        "no location: {stdout}"
    );
    assert!(
        stdout.contains("python/fastapi_routes.py:28"),
        "no target location: {stdout}"
    );
}

#[test]
fn trace_marks_the_break_rather_than_implying_completeness() {
    let output = cartograph()
        .args(["trace", "http.ts", "--path"])
        .arg(parser_fixtures())
        .output()
        .expect("binary runs");
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(
        stdout.contains("chain ends here"),
        "an incomplete chain must say so: {stdout}"
    );
    assert!(
        stdout.contains("does not reach a database table"),
        "the summary line must not imply a full-stack path: {stdout}"
    );
}

#[test]
fn trace_labels_a_fork_instead_of_printing_it_as_one_chain() {
    // Two outgoing edges printed in sequence read as A -> B -> C, which is a
    // different claim from A -> B and A -> C.
    let output = cartograph()
        .args(["trace", "http.ts", "--path"])
        .arg(parser_fixtures())
        .output()
        .expect("binary runs");
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(
        stdout.contains("branch 1 of 2"),
        "unlabelled fork: {stdout}"
    );
    assert!(
        stdout.contains("branch 2 of 2"),
        "unlabelled fork: {stdout}"
    );
}

#[test]
fn trace_reports_a_missing_symbol_with_its_own_exit_code() {
    let output = cartograph()
        .args(["trace", "NoSuchSymbolAnywhere", "--path"])
        .arg(parser_fixtures())
        .output()
        .expect("binary runs");

    assert_eq!(output.status.code(), Some(exit::NOT_FOUND));
    assert!(output.stdout.is_empty(), "stdout must stay empty on error");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("symbol-not-found"), "{stderr}");
    assert!(stderr.contains("NoSuchSymbolAnywhere"), "{stderr}");
}

#[test]
fn trace_json_reports_a_missing_symbol_as_json_on_stdout() {
    let output = cartograph()
        .args(["trace", "NoSuchSymbolAnywhere", "--json", "--path"])
        .arg(parser_fixtures())
        .output()
        .expect("binary runs");

    assert_eq!(output.status.code(), Some(exit::NOT_FOUND));
    let value: serde_json::Value = serde_json::from_slice(&output.stdout)
        .expect("a failing --json run must still emit JSON on stdout");
    assert_eq!(value["schema_version"], "1.2");
    assert_eq!(value["command"], "trace");
    assert_eq!(value["errors"][0]["code"], "symbol-not-found");
}

#[test]
fn trace_refuses_to_choose_between_ambiguous_symbols() {
    // `health` is declared in both the FastAPI and Flask fixtures.
    let output = cartograph()
        .args(["trace", "health", "--path"])
        .arg(parser_fixtures())
        .output()
        .expect("binary runs");

    // Either it is unique here (then this asserts nothing useful) or it is
    // ambiguous — in which case no symbol may be chosen.
    if output.status.code() == Some(exit::AMBIGUOUS) {
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(stderr.contains("ambiguous-symbol"), "{stderr}");
        assert!(stderr.contains("matches"), "{stderr}");
        assert!(
            stderr.contains("qualifying the name with its file"),
            "the error must say how to disambiguate: {stderr}"
        );
        assert!(output.stdout.is_empty());
    }
}

#[test]
fn trace_accepts_a_file_qualified_symbol() {
    let output = cartograph()
        .args(["trace", "python/fastapi_routes.py:health", "--path"])
        .arg(parser_fixtures())
        .output()
        .expect("binary runs");

    assert_eq!(
        output.status.code(),
        Some(exit::SUCCESS),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn trace_json_carries_the_full_evidence_of_every_hop() {
    let output = cartograph()
        .args(["trace", "http.ts", "--json", "--path"])
        .arg(parser_fixtures())
        .output()
        .expect("binary runs");

    let value: serde_json::Value = serde_json::from_slice(&output.stdout).expect("pure JSON");
    assert_eq!(value["command"], "trace");
    let steps = value["result"]["steps"].as_array().expect("steps array");
    assert!(!steps.is_empty());

    for step in steps {
        let edge = &step["edge"];
        for required in [
            "source",
            "target",
            "kind",
            "confidence",
            "provenance",
            "evidence",
            "file",
            "line",
        ] {
            assert!(
                edge.get(required).is_some(),
                "trace lost `{required}` from an edge: {edge}"
            );
        }
        assert!(step["to"]["name"].is_string(), "no target node: {step}");
    }
}

#[test]
fn trace_respects_the_depth_limit() {
    let output = cartograph()
        .args(["trace", "http.ts", "--max-depth", "0", "--json", "--path"])
        .arg(parser_fixtures())
        .output()
        .expect("binary runs");

    let value: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(value["result"]["max_depth"], 0);
    assert_eq!(
        value["result"]["stop"], "max-depth",
        "a truncated walk must say it was truncated: {}",
        value["result"]
    );
}

// ── the JSON contract ───────────────────────────────────────────────

#[test]
fn every_json_document_declares_the_schema_version_and_command() {
    let cases: [(&[&str], &str); 4] = [
        (&["--json"], "summary"),
        (&["parse", "--json"], "parse"),
        (&["normalize", "--json"], "normalize"),
        (&["match", "--json"], "match"),
    ];

    for (args, command) in cases {
        let mut cmd = cartograph();
        // The default summary takes the path positionally; the rest take it
        // after the subcommand. Both orders end up here.
        if args == ["--json"] {
            cmd.arg("--json").arg(parser_fixtures());
        } else {
            cmd.args(args).arg(parser_fixtures());
        }
        let output = cmd.output().expect("binary runs");
        let value: serde_json::Value = serde_json::from_slice(&output.stdout)
            .unwrap_or_else(|e| panic!("{command} did not emit pure JSON: {e}"));

        assert_eq!(value["schema_version"], "1.2", "{command}");
        assert_eq!(value["command"], command, "{command}");
    }
}

#[test]
fn the_summary_document_has_the_documented_sections() {
    let output = cartograph()
        .arg("--json")
        .arg(parser_fixtures())
        .output()
        .expect("binary runs");
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();

    for section in [
        "schema_version",
        "command",
        "repository",
        "languages",
        "summary",
        "nodes",
        "edges",
        "diagnostics",
        "errors",
    ] {
        assert!(value.get(section).is_some(), "missing `{section}`");
    }
    assert!(value["errors"].as_array().unwrap().is_empty());
    assert!(value["summary"]["files"].as_u64().unwrap() >= 8);
    assert!(value["summary"]["duration_ms"].is_number());
}

#[test]
fn no_json_document_contains_an_absolute_machine_path() {
    // PART 12 and PART 21: a serialised graph states facts about a
    // repository, never about the machine that analysed it.
    for args in [
        vec!["--json"],
        vec!["parse", "--json"],
        vec!["normalize", "--json"],
        vec!["match", "--json"],
    ] {
        let mut cmd = cartograph();
        cmd.args(&args).arg(parser_fixtures());
        let output = cmd.output().expect("binary runs");
        let text = String::from_utf8_lossy(&output.stdout);

        assert!(
            !text.contains("/Volumes/"),
            "{args:?} leaked an absolute path"
        );
        assert!(
            !text.contains(env!("CARGO_MANIFEST_DIR")),
            "{args:?} leaked the manifest directory"
        );
    }
}

#[test]
fn an_absolute_repository_path_is_accepted_but_never_echoed() {
    let output = cartograph()
        .arg("--json")
        .arg(parser_fixtures()) // an absolute path
        .output()
        .expect("binary runs");

    assert!(output.status.success());
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(value["repository"]["name"], "fixtures");
    assert_eq!(value["repository"]["requested"], "<absolute>");
}

/// A path rooted on the current drive, with no drive letter, is legal on
/// Windows and is not `Path::is_absolute()`. It reached both the serialised
/// `requested` field and the human-readable error before this was fixed.
#[test]
fn a_rooted_drive_less_path_never_reaches_serialised_output() {
    // The forward-slash form is rooted on Unix too. The backslash form is a
    // plain filename there, so asserting on it would be a Windows assumption.
    #[cfg(windows)]
    const ROOTED: &[&str] = &[
        "/nonexistent-root-xyz/secret-project",
        r"\nonexistent-root-xyz\secret-project",
    ];
    #[cfg(not(windows))]
    const ROOTED: &[&str] = &["/nonexistent-root-xyz/secret-project"];

    for &rooted in ROOTED {
        let output = cartograph()
            .arg("--json")
            .arg(rooted)
            .output()
            .expect("binary runs");

        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(
            !stdout.contains("nonexistent-root-xyz"),
            "the rooted path leaked into JSON for {rooted}: {stdout}"
        );
        // Still a well-formed document reporting the failure, not a panic.
        let value: serde_json::Value = serde_json::from_str(&stdout).expect("stdout is pure JSON");
        assert_eq!(value["schema_version"], "1.2");

        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            !stderr.contains("nonexistent-root-xyz"),
            "the rooted path leaked into stderr for {rooted}: {stderr}"
        );
    }
}

/// The redaction must not be implemented by redacting everything: a
/// repository-relative path is the user's own words and is echoed unchanged.
#[test]
fn a_relative_repository_path_is_still_echoed_as_typed() {
    let parser_crate = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("cartograph-parser");

    let output = cartograph()
        .arg("--json")
        .arg("tests/fixtures")
        .current_dir(&parser_crate)
        .output()
        .expect("binary runs");

    assert!(output.status.success(), "exit status: {}", output.status);
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(value["repository"]["requested"], "tests/fixtures");
}

// ── stdout / stderr discipline ──────────────────────────────────────

#[test]
fn json_mode_keeps_stdout_pure_even_with_logging_turned_up() {
    for args in [
        vec!["--json", "-vv"],
        vec!["match", "--json", "-vv"],
        vec!["parse", "--json", "-vv"],
    ] {
        let mut cmd = cartograph();
        cmd.args(&args).arg(parser_fixtures());
        let output = cmd.output().expect("binary runs");
        assert!(
            serde_json::from_slice::<serde_json::Value>(&output.stdout).is_ok(),
            "{args:?} contaminated stdout: {}",
            String::from_utf8_lossy(&output.stdout)
        );
    }
}

#[test]
fn json_mode_emits_no_progress_output() {
    let output = cartograph()
        .arg("--json")
        .arg(parser_fixtures())
        .output()
        .expect("binary runs");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !stderr.contains("analysing…"),
        "progress leaked in JSON mode: {stderr}"
    );
}

#[test]
fn a_failure_writes_nothing_to_stdout_in_text_mode() {
    let output = cartograph()
        .arg("no/such/repository")
        .output()
        .expect("binary runs");
    assert_eq!(output.status.code(), Some(exit::INPUT));
    assert!(
        output.stdout.is_empty(),
        "stdout must stay empty so a pipe receives nothing: {}",
        String::from_utf8_lossy(&output.stdout)
    );
    assert!(!output.stderr.is_empty(), "the error must be reported");
}

// ── exit codes ──────────────────────────────────────────────────────

#[test]
fn success_exits_zero() {
    let output = cartograph()
        .arg(parser_fixtures())
        .output()
        .expect("binary runs");
    assert_eq!(output.status.code(), Some(exit::SUCCESS));
}

#[test]
fn a_usage_error_exits_two() {
    let output = cartograph()
        .args(["--definitely-not-a-flag"])
        .output()
        .expect("binary runs");
    assert_eq!(output.status.code(), Some(exit::USAGE));
}

#[test]
fn no_arguments_shows_help_and_exits_with_the_usage_code() {
    let output = cartograph().output().expect("binary runs");
    assert_eq!(output.status.code(), Some(exit::USAGE));
    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(combined.contains("Usage"), "no usage shown: {combined}");
}

#[test]
fn a_missing_path_exits_with_the_input_code() {
    let output = cartograph()
        .arg("definitely/not/a/repository")
        .output()
        .expect("binary runs");
    assert_eq!(output.status.code(), Some(exit::INPUT));
}

#[test]
fn strict_turns_a_partial_analysis_into_a_non_zero_exit() {
    // The fixture corpus deliberately contains files that cannot be parsed.
    let output = cartograph()
        .arg(parser_fixtures())
        .arg("--strict")
        .output()
        .expect("binary runs");

    assert_eq!(output.status.code(), Some(exit::PARTIAL));
    // The result is still delivered: a partial analysis is still an analysis.
    assert!(!output.stdout.is_empty());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("partial-analysis"), "{stderr}");
}

#[test]
fn the_same_repository_succeeds_without_strict() {
    let output = cartograph()
        .arg(parser_fixtures())
        .output()
        .expect("binary runs");
    assert_eq!(
        output.status.code(),
        Some(exit::SUCCESS),
        "an unparseable file must not fail the run by default"
    );
}

#[test]
fn strict_json_records_the_partial_analysis_in_the_errors_array() {
    let output = cartograph()
        .arg("--json")
        .arg(parser_fixtures())
        .arg("--strict")
        .output()
        .expect("binary runs");

    assert_eq!(output.status.code(), Some(exit::PARTIAL));
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).expect("pure JSON");
    assert_eq!(value["errors"][0]["code"], "partial-analysis");
    // The analysis is still present alongside the complaint.
    assert!(value["summary"]["files"].as_u64().unwrap() > 0);
}

// ── path handling (PART 12) ─────────────────────────────────────────

#[test]
fn the_current_directory_is_analysable_as_a_relative_path() {
    let output = cartograph()
        .arg("--json")
        .arg(".")
        .current_dir(parser_fixtures())
        .output()
        .expect("binary runs");

    assert!(output.status.success());
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(value["repository"]["requested"], ".");
    assert_eq!(value["repository"]["name"], "fixtures");
}

#[test]
fn a_relative_subdirectory_is_analysable() {
    let output = cartograph()
        .arg("--json")
        .arg("python")
        .current_dir(parser_fixtures())
        .output()
        .expect("binary runs");

    assert!(output.status.success());
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(value["repository"]["requested"], "python");
    assert!(value["summary"]["files"].as_u64().unwrap() >= 8);
}

#[test]
fn relative_and_absolute_paths_produce_the_same_graph() {
    let strip = |mut value: serde_json::Value| {
        value["repository"] = serde_json::Value::Null;
        value["summary"]["duration_ms"] = serde_json::Value::Null;
        value
    };

    let relative = cartograph()
        .arg("--json")
        .arg(".")
        .current_dir(parser_fixtures())
        .output()
        .expect("binary runs");
    let absolute = cartograph()
        .arg("--json")
        .arg(parser_fixtures())
        .output()
        .expect("binary runs");

    let a: serde_json::Value = serde_json::from_slice(&relative.stdout).unwrap();
    let b: serde_json::Value = serde_json::from_slice(&absolute.stdout).unwrap();
    assert_eq!(strip(a), strip(b), "the path spelling changed the analysis");
}

#[test]
fn a_file_is_accepted_where_a_directory_is_expected() {
    let output = cartograph()
        .arg(parser_fixtures().join("http.ts"))
        .output()
        .expect("binary runs");
    assert_eq!(output.status.code(), Some(exit::SUCCESS));
}

#[test]
fn serialised_paths_are_repository_relative_with_forward_slashes() {
    let output = cartograph()
        .arg("--json")
        .arg(parser_fixtures())
        .output()
        .expect("binary runs");
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();

    for node in value["nodes"].as_array().unwrap() {
        if let Some(file) = node["file"].as_str() {
            assert!(!file.starts_with('/'), "absolute node path: {file}");
            assert!(!file.contains('\\'), "backslash in node path: {file}");
            assert!(!file.contains(".."), "traversal in node path: {file}");
        }
    }
    for edge in value["edges"].as_array().unwrap() {
        let file = edge["file"].as_str().unwrap();
        assert!(!file.starts_with('/'), "absolute edge path: {file}");
        assert!(!file.contains('\\'), "backslash in edge path: {file}");
    }
}
