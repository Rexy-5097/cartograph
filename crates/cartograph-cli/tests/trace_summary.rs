//! T-001: the sentence that closes a trace must be true of the path shown.
//!
//! `trace` used to print "This path crosses the HTTP boundary and reaches a
//! database table." whenever any hop reached a table, without checking that an
//! HTTP-call edge was on the path. These tests run the real binary on small
//! projects and read the closing sentence.

use std::path::{Path, PathBuf};
use std::process::Command;

const CROSSES: &str = "crosses the HTTP boundary and reaches a database table";
const TABLE_ONLY: &str = "reaches a database table without crossing the HTTP boundary";
const NO_TABLE: &str = "does not reach a database table";

/// A project with one complete chain and one request whose handler touches
/// no model.
fn project(name: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!(
        "cartograph-trace-summary-{name}-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&root);
    write(
        &root,
        "web/checkout.ts",
        "export async function placeOrder(order) {
  return fetch('/api/orders', { method: 'POST' });
}

export async function ping() {
  return fetch('/api/health');
}

export async function both() {
  await fetch('/api/health');
  return fetch('/api/orders', { method: 'POST' });
}
",
    );
    write(
        &root,
        "api/routes.py",
        "from fastapi import FastAPI
from .models import Order

app = FastAPI()


@app.post(\"/api/orders\")
def create_order(payload: dict):
    return Order(**payload)


@app.get(\"/api/health\")
def health():
    return {}
",
    );
    write(
        &root,
        "api/models.py",
        "from sqlalchemy.orm import DeclarativeBase


class Base(DeclarativeBase):
    pass


class Order(Base):
    __tablename__ = \"orders\"
",
    );
    root
}

fn write(root: &Path, relative: &str, contents: &str) {
    let path = root.join(relative);
    std::fs::create_dir_all(path.parent().expect("has a parent")).expect("directory");
    std::fs::write(path, contents).expect("write");
}

fn trace(root: &Path, symbol: &str) -> (String, i32) {
    let output = Command::new(env!("CARGO_BIN_EXE_cartograph"))
        .args(["trace", symbol, "--path"])
        .arg(root)
        .output()
        .expect("binary runs");
    (
        String::from_utf8_lossy(&output.stdout).into_owned(),
        output.status.code().unwrap_or(-1),
    )
}

#[test]
fn a_chain_with_an_http_edge_and_a_table_says_it_crosses_the_boundary() {
    let root = project("full");
    let (stdout, code) = trace(&root, "placeOrder");
    assert_eq!(code, 0, "{stdout}");
    assert!(stdout.contains("HTTP-CALL"), "{stdout}");
    assert!(stdout.contains(CROSSES), "{stdout}");
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn a_model_traced_to_its_table_claims_no_http_boundary() {
    let root = project("model");
    let (stdout, code) = trace(&root, "Order");
    assert_eq!(code, 0, "{stdout}");
    assert!(!stdout.contains("HTTP-CALL"), "{stdout}");
    assert!(
        !stdout.contains("crosses the HTTP boundary"),
        "a class-to-table path has no HTTP edge: {stdout}"
    );
    assert!(stdout.contains(TABLE_ONLY), "{stdout}");
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn a_handler_traced_to_a_table_claims_no_http_boundary() {
    let root = project("handler");
    let (stdout, code) = trace(&root, "create_order");
    assert_eq!(code, 0, "{stdout}");
    assert!(stdout.contains("ORM-ACCESS"), "{stdout}");
    assert!(!stdout.contains("crosses the HTTP boundary"), "{stdout}");
    assert!(stdout.contains(TABLE_ONLY), "{stdout}");
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn a_request_whose_handler_reaches_no_table_says_so() {
    let root = project("request");
    let (stdout, code) = trace(&root, "ping");
    assert_eq!(code, 0, "{stdout}");
    assert!(stdout.contains("HTTP-CALL"), "{stdout}");
    assert!(!stdout.contains("crosses the HTTP boundary"), "{stdout}");
    assert!(stdout.contains(NO_TABLE), "{stdout}");
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn two_http_edges_of_which_one_reaches_a_table_is_a_crossing() {
    let root = project("two");
    let (stdout, code) = trace(&root, "both");
    assert_eq!(code, 0, "{stdout}");
    assert_eq!(stdout.matches("HTTP-CALL").count(), 2, "{stdout}");
    assert!(stdout.contains(CROSSES), "{stdout}");
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn an_unresolved_symbol_prints_no_summary_sentence_at_all() {
    let root = project("missing");
    let (stdout, code) = trace(&root, "noSuchSymbol");
    assert_eq!(code, 5, "{stdout}");
    assert!(!stdout.contains("HTTP boundary"), "{stdout}");
    let _ = std::fs::remove_dir_all(&root);
}
