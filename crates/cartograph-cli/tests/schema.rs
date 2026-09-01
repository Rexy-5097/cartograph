//! Conformance of `--json` output to the published schema.
//!
//! # Why the schema is executed rather than described
//!
//! `docs/architecture/cartograph-output-1.1.schema.json` is the contract a
//! future MCP server, editor plugin or CI integration will be written against.
//! A schema file that nothing checks is a wish, and it drifts from the code the
//! first time a field is renamed. These tests load that exact file and validate
//! real command output against it, so the document and the program cannot
//! disagree without a test failing.
//!
//! # Why the validator is written here
//!
//! It supports the subset the schema actually uses — `type`, `required`,
//! `enum`, `const`, `properties`, `items`, `minimum`, `maximum`, `minLength` —
//! in about a hundred lines. A general JSON Schema implementation would be a
//! new dependency in service of one file, which the project's dependency rule
//! declines (PART 22). If the schema ever needs a construct this does not
//! support, `unsupported_keywords_are_not_silently_ignored` fails rather than
//! letting the validation quietly become a no-op.

use std::path::PathBuf;
use std::process::Command;

use serde_json::Value;

fn cartograph() -> Command {
    Command::new(env!("CARGO_BIN_EXE_cartograph"))
}

fn fixtures() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("cartograph-parser/tests/fixtures")
}

fn schema() -> Value {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("docs/architecture/cartograph-output-1.1.schema.json");
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("reading {}: {e}", path.display()));
    serde_json::from_str(&text).expect("the published schema is valid JSON")
}

/// The keywords the validator below understands.
const SUPPORTED: [&str; 15] = [
    "$schema",
    "$id",
    "title",
    "description",
    "type",
    "required",
    "enum",
    "const",
    "properties",
    "items",
    "minimum",
    "maximum",
    "minLength",
    "oneOf",
    "additionalProperties",
];

/// Validates `value` against `schema`, collecting every violation.
fn validate(value: &Value, schema: &Value, path: &str, problems: &mut Vec<String>) {
    let Some(object) = schema.as_object() else {
        return;
    };

    if let Some(expected) = object.get("type").and_then(Value::as_str) {
        let ok = match expected {
            "object" => value.is_object(),
            "array" => value.is_array(),
            "string" => value.is_string(),
            "integer" => value.is_i64() || value.is_u64(),
            "number" => value.is_number(),
            "boolean" => value.is_boolean(),
            other => {
                problems.push(format!("{path}: schema uses unknown type `{other}`"));
                true
            }
        };
        if !ok {
            problems.push(format!("{path}: expected {expected}, found {value}"));
            return;
        }
    }

    if let Some(expected) = object.get("const") {
        if value != expected {
            problems.push(format!("{path}: expected const {expected}, found {value}"));
        }
    }

    if let Some(Value::Array(allowed)) = object.get("enum") {
        if !allowed.contains(value) {
            problems.push(format!("{path}: {value} is not one of {allowed:?}"));
        }
    }

    if let (Some(minimum), Some(actual)) = (
        object.get("minimum").and_then(Value::as_f64),
        value.as_f64(),
    ) {
        if actual < minimum {
            problems.push(format!("{path}: {actual} is below minimum {minimum}"));
        }
    }
    if let (Some(maximum), Some(actual)) = (
        object.get("maximum").and_then(Value::as_f64),
        value.as_f64(),
    ) {
        if actual > maximum {
            problems.push(format!("{path}: {actual} is above maximum {maximum}"));
        }
    }
    if let (Some(minimum), Some(actual)) = (
        object.get("minLength").and_then(Value::as_u64),
        value.as_str(),
    ) {
        if (actual.len() as u64) < minimum {
            problems.push(format!("{path}: string shorter than {minimum}"));
        }
    }

    if let Some(Value::Array(required)) = object.get("required") {
        for name in required.iter().filter_map(Value::as_str) {
            if value.get(name).is_none() {
                problems.push(format!("{path}: missing required field `{name}`"));
            }
        }
    }

    if let Some(Value::Object(properties)) = object.get("properties") {
        for (name, sub) in properties {
            if let Some(child) = value.get(name) {
                validate(child, sub, &format!("{path}.{name}"), problems);
            }
        }
    }

    if let (Some(items), Some(array)) = (object.get("items"), value.as_array()) {
        for (index, element) in array.iter().enumerate() {
            validate(element, items, &format!("{path}[{index}]"), problems);
        }
    }

    check_additional_properties(value, object, path, problems);
    check_one_of(value, object, path, problems);
}

/// `additionalProperties: false` — every field present must be declared.
///
/// Without this, a payload could grow a field the contract never promised and
/// still "conform".
fn check_additional_properties(
    value: &Value,
    schema: &serde_json::Map<String, Value>,
    path: &str,
    problems: &mut Vec<String>,
) {
    if schema.get("additionalProperties") != Some(&Value::Bool(false)) {
        return;
    }
    if let (Some(Value::Object(declared)), Some(actual)) =
        (schema.get("properties"), value.as_object())
    {
        for name in actual.keys() {
            if !declared.contains_key(name) {
                problems.push(format!("{path}: undeclared field `{name}`"));
            }
        }
    }
}

/// `oneOf` — exactly one branch must accept the value.
///
/// Branches are tried against a scratch list so a failing alternative does not
/// report itself as a problem with the document; only the count matters.
fn check_one_of(
    value: &Value,
    schema: &serde_json::Map<String, Value>,
    path: &str,
    problems: &mut Vec<String>,
) {
    let Some(Value::Array(branches)) = schema.get("oneOf") else {
        return;
    };
    let accepted = branches
        .iter()
        .filter(|branch| {
            let mut scratch = Vec::new();
            validate(value, branch, path, &mut scratch);
            scratch.is_empty()
        })
        .count();
    if accepted != 1 {
        problems.push(format!(
            "{path}: matched {accepted} of {} `oneOf` branches, expected exactly 1",
            branches.len()
        ));
    }
}

fn assert_conforms(label: &str, document: &Value) {
    let mut problems = Vec::new();
    validate(document, &schema(), label, &mut problems);
    assert!(
        problems.is_empty(),
        "{label} does not conform to the published schema:\n  {}",
        problems.join("\n  ")
    );
}

fn json_of(args: &[&str]) -> Value {
    let mut command = cartograph();
    for arg in args {
        if *arg == "<fixtures>" {
            command.arg(fixtures());
        } else {
            command.arg(arg);
        }
    }
    let output = command.output().expect("binary runs");
    serde_json::from_slice(&output.stdout).unwrap_or_else(|e| {
        panic!(
            "{args:?} did not emit valid JSON: {e}\n{}",
            String::from_utf8_lossy(&output.stdout)
        )
    })
}

// ── the validator itself ────────────────────────────────────────────

#[test]
fn unsupported_keywords_are_not_silently_ignored() {
    // If the schema grows a construct this validator cannot check, every
    // conformance test below would keep passing while checking less. Failing
    // here is how that gets noticed.
    fn walk(node: &Value, path: &str, unsupported: &mut Vec<String>) {
        let Some(object) = node.as_object() else {
            return;
        };
        for (key, child) in object {
            if key == "properties" {
                if let Some(properties) = child.as_object() {
                    for (name, sub) in properties {
                        walk(sub, &format!("{path}.{name}"), unsupported);
                    }
                }
                continue;
            }
            if key == "items" {
                walk(child, &format!("{path}[]"), unsupported);
                continue;
            }
            if key == "oneOf" {
                if let Some(branches) = child.as_array() {
                    for (index, branch) in branches.iter().enumerate() {
                        walk(branch, &format!("{path}.oneOf[{index}]"), unsupported);
                    }
                }
                continue;
            }
            if !SUPPORTED.contains(&key.as_str()) {
                unsupported.push(format!("{path}.{key}"));
            }
        }
    }

    let mut unsupported = Vec::new();
    walk(&schema(), "schema", &mut unsupported);
    assert!(
        unsupported.is_empty(),
        "the schema uses keywords the conformance validator does not check, so \
         conformance is weaker than it appears:\n  {}",
        unsupported.join("\n  ")
    );
}

#[test]
fn the_validator_rejects_a_document_that_violates_the_schema() {
    // A validator that never fails proves nothing.
    let mut document = json_of(&["--json", "<fixtures>"]);
    document["schema_version"] = Value::String("99.0".to_owned());

    let mut problems = Vec::new();
    validate(&document, &schema(), "tampered", &mut problems);
    assert!(!problems.is_empty(), "a wrong schema_version was accepted");

    let mut document = json_of(&["--json", "<fixtures>"]);
    document["edges"][0]["confidence"] = Value::from(1.5);
    let mut problems = Vec::new();
    validate(&document, &schema(), "tampered", &mut problems);
    assert!(!problems.is_empty(), "a confidence above 1.0 was accepted");

    let mut document = json_of(&["--json", "<fixtures>"]);
    document["edges"][0]
        .as_object_mut()
        .unwrap()
        .remove("evidence");
    let mut problems = Vec::new();
    validate(&document, &schema(), "tampered", &mut problems);
    assert!(
        !problems.is_empty(),
        "an edge without evidence was accepted"
    );
}

// ── conformance ─────────────────────────────────────────────────────

#[test]
fn the_summary_document_conforms() {
    assert_conforms("summary", &json_of(&["--json", "<fixtures>"]));
}

#[test]
fn the_trace_document_conforms() {
    assert_conforms(
        "trace",
        &json_of(&["trace", "http.ts", "--json", "--path", "<fixtures>"]),
    );
}

/// The 1.1 addition. Also proves the discriminated `result` works: a blast
/// document matches exactly one `oneOf` branch, and the validator now checks
/// that rather than accepting anything shaped roughly right.
#[test]
fn the_blast_document_conforms() {
    assert_conforms(
        "blast",
        &json_of(&["blast", "http.ts", "--json", "--path", "<fixtures>"]),
    );
}

/// The compatibility promise of 1.1: the `trace` branch is the 1.0 shape, so a
/// trace document must still match exactly one branch and never the blast one.
#[test]
fn a_trace_document_does_not_match_the_blast_branch() {
    let document = json_of(&["trace", "http.ts", "--json", "--path", "<fixtures>"]);
    let result = document.get("result").expect("trace emits a result");

    let schema = schema();
    let branches = schema["properties"]["result"]["oneOf"]
        .as_array()
        .expect("result is a oneOf");
    let accepted: Vec<usize> = branches
        .iter()
        .enumerate()
        .filter(|(_, branch)| {
            let mut problems = Vec::new();
            validate(result, branch, "result", &mut problems);
            problems.is_empty()
        })
        .map(|(index, _)| index)
        .collect();

    assert_eq!(
        accepted,
        vec![0],
        "a trace result must match the trace branch and only that one"
    );
}

#[test]
fn the_error_document_conforms() {
    assert_conforms(
        "error",
        &json_of(&["trace", "NoSuchSymbol", "--json", "--path", "<fixtures>"]),
    );
}

#[test]
fn the_per_stage_documents_carry_the_envelope() {
    for command in ["parse", "normalize", "match"] {
        let document = json_of(&[command, "--json", "<fixtures>"]);
        assert_conforms(command, &document);
        assert_eq!(document["schema_version"], "1.1", "{command}");
        assert_eq!(document["command"], command);
    }
}

#[test]
fn a_real_repository_conforms_too() {
    // The fixture corpus is small and hand-made. A schema that only holds for
    // it would be worth very little, so this runs over the resolver's own
    // multi-file project fixture as well.
    let project = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("cartograph-resolver/tests/fixtures/dynamic-project");
    let output = cartograph()
        .arg("--json")
        .arg(&project)
        .output()
        .expect("binary runs");
    let document: Value = serde_json::from_slice(&output.stdout).expect("valid JSON");
    assert_conforms("dynamic-project", &document);
    assert!(!document["edges"].as_array().unwrap().is_empty());
}

#[test]
fn every_serialised_edge_carries_the_case_for_believing_it() {
    // The project's founding rule, checked at the serialisation boundary.
    for args in [
        vec!["--json", "<fixtures>"],
        vec!["trace", "http.ts", "--json", "--path", "<fixtures>"],
    ] {
        let document = json_of(&args);
        let edges = document["edges"].as_array().expect("edges");
        assert!(!edges.is_empty());
        for edge in edges {
            for required in ["confidence", "provenance", "evidence", "file", "line"] {
                assert!(
                    edge.get(required).is_some(),
                    "{args:?}: an edge was serialised without `{required}`: {edge}"
                );
            }
            assert_ne!(
                edge["provenance"], "model-inference",
                "no language model may produce graph structure (RULE 007)"
            );
        }
    }
}
