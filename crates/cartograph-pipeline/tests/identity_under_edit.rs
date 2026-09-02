//! Identity under real edits, through the real analyser.
//!
//! ADR-0014's gate points 3 and 4 are about what happens when source changes:
//! an unrelated edit must not renumber unaffected nodes, and move and rename
//! must have defined, tested answers. Unit tests on hand-built nodes state the
//! rule; they cannot show that the *analyser* produces nodes the rule then
//! holds for, because the line an artefact is reported at is the analyser's
//! output, not the test's.
//!
//! So each case here writes a small repository into a temporary directory,
//! analyses it, edits it, analyses it again, and compares identities. Nothing
//! outside the temporary directory is touched, and no corpus is required — the
//! trees are written by the test, so these run by default rather than opt-in.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use cartograph_core::{NodeIdentity, identity_of};
use cartograph_pipeline::pipeline::{self, Options};

/// A throwaway repository, removed when the test finishes.
struct TempRepo {
    root: PathBuf,
}

impl TempRepo {
    fn new(tag: &str) -> Self {
        let root = std::env::temp_dir().join(format!(
            "cartograph-identity-{tag}-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).expect("create temp repository");
        Self { root }
    }

    fn write(&self, relative: &str, contents: &str) {
        let path = self.root.join(relative);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).expect("create directory");
        }
        std::fs::write(path, contents).expect("write source");
    }

    fn identities(&self) -> BTreeSet<NodeIdentity> {
        let analysis = pipeline::run(Path::new(&self.root), Options { quiet: true })
            .expect("the temporary repository analyses");
        analysis.graph.nodes().map(identity_of).collect()
    }
}

impl Drop for TempRepo {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

/// The source used throughout: a Flask handler reading a Django model.
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

const MODELS: &str = "from django.db import models

class Order(models.Model):
    class Meta:
        db_table = 'orders'
";

// ── Gate 3: an unrelated edit leaves unaffected identities alone ────────

/// Adding an import at the top of a file pushes every artefact below it down.
/// If the reported line took part in identity, this single edit would rename
/// everything in the file and a diff would show the file as pure churn.
#[test]
fn an_unrelated_edit_above_an_artefact_preserves_every_identity() {
    let repo = TempRepo::new("edit");
    repo.write("api/models.py", MODELS);
    repo.write("api/routes.py", &routes("", "list_orders"));
    let before = repo.identities();
    assert!(!before.is_empty(), "precondition: the tree must analyse");

    // Five unrelated lines at the top of the file. Nothing else changes.
    repo.write(
        "api/routes.py",
        &routes(
            "import os\nimport sys\nimport json\nimport logging\nimport datetime\n",
            "list_orders",
        ),
    );
    let after = repo.identities();

    assert_eq!(
        before, after,
        "an edit above the handler changed identities that should not have moved"
    );
}

// ── Gate 4, move: matched ──────────────────────────────────────────────

/// The same artefact, further down its own file, is the same artefact.
#[test]
fn moving_an_artefact_down_its_own_file_preserves_its_identity() {
    let repo = TempRepo::new("move");
    repo.write("api/models.py", MODELS);
    repo.write("api/routes.py", &routes("", "list_orders"));
    let before = repo.identities();

    // Twenty blank-ish lines of padding: the handler is now far from where it
    // started, and nothing about what it *is* has changed.
    let padding = "# padding\n".repeat(20);
    repo.write("api/routes.py", &routes(&padding, "list_orders"));
    let after = repo.identities();

    assert_eq!(
        before, after,
        "moving the handler within its file must not change its identity"
    );
}

// ── Gate 4, rename: deliberately remove-plus-add ───────────────────────

/// A rename is reported as a removal and an addition, by policy. No similarity
/// heuristic runs, so nothing claims the old artefact "became" the new one.
#[test]
fn renaming_an_artefact_is_a_removal_and_an_addition() {
    let repo = TempRepo::new("rename");
    repo.write("api/models.py", MODELS);
    repo.write("api/routes.py", &routes("", "list_orders"));
    let before = repo.identities();

    repo.write("api/routes.py", &routes("", "list_all_orders"));
    let after = repo.identities();

    let removed: Vec<_> = before.difference(&after).collect();
    let added: Vec<_> = after.difference(&before).collect();

    assert!(
        removed.iter().any(|i| i.as_str().contains("list_orders")),
        "the old name should be gone: {removed:#?}"
    );
    assert!(
        added.iter().any(|i| i.as_str().contains("list_all_orders")),
        "the new name should appear: {added:#?}"
    );
    assert_ne!(
        before, after,
        "a rename must be visible, not silently matched"
    );
}

/// Moving an artefact to a different file is also remove-plus-add: the path is
/// part of what the artefact is.
#[test]
fn moving_an_artefact_to_another_file_is_a_removal_and_an_addition() {
    let repo = TempRepo::new("relocate");
    repo.write("api/models.py", MODELS);
    repo.write("api/routes.py", &routes("", "list_orders"));
    let before = repo.identities();

    std::fs::remove_file(repo.root.join("api/routes.py")).expect("remove");
    repo.write("api/handlers.py", &routes("", "list_orders"));
    let after = repo.identities();

    assert_ne!(
        before, after,
        "relocating the handler to another file must be visible"
    );
    assert!(
        before
            .difference(&after)
            .any(|i| i.as_str().contains("routes.py")),
        "the old file's artefacts should be gone"
    );
    assert!(
        after
            .difference(&before)
            .any(|i| i.as_str().contains("handlers.py")),
        "the new file's artefacts should appear"
    );
}

/// Identity must survive re-analysis of an untouched tree — the degenerate
/// case of gate point 2, checked without needing a vendored corpus.
#[test]
fn re_analysing_an_unchanged_tree_reproduces_every_identity() {
    let repo = TempRepo::new("stable");
    repo.write("api/models.py", MODELS);
    repo.write("api/routes.py", &routes("", "list_orders"));

    let first = repo.identities();
    let second = repo.identities();

    assert_eq!(first, second);
    assert!(!first.is_empty(), "precondition: the tree must analyse");
}
