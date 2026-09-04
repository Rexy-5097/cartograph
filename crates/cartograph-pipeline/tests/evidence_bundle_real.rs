//! Evidence bundles over a real analysed tree.
//!
//! Hand-built graphs in `cartograph-graph` establish that the bundle's rules
//! are right in the small. They cannot establish that the rules survive
//! evidence the analyser actually produced: real evidence is written by the
//! resolver, not by a test, and it is the resolver's wording that a citation
//! has to quote verbatim.
//!
//! The corpus here is **vendored** rather than opt-in. `blast_real.rs` reaches
//! for the pinned external corpora because blast radius needs scale to be
//! interesting; a bundle does not. What it needs is evidence that no test
//! author wrote, and the cross-language fixture already in this repository
//! supplies that: a TypeScript client, a Python route and a Django model,
//! analysed end to end, producing `http-call`, `orm-access` and `queries`
//! edges with the resolver's own wording.
//!
//! Read-only: the analyser opens files and never writes, and nothing here
//! mutates the fixture.

use std::path::PathBuf;

use cartograph_graph::bundle::EvidenceBundle;
use cartograph_pipeline::pipeline::{self, Options};

/// The cross-language fixture, analysed.
///
/// Located from `CARGO_MANIFEST_DIR` rather than the working directory, so the
/// test does not depend on where cargo was invoked from.
fn analysed() -> pipeline::Analysis {
    let root =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../cartograph-cli/tests/fixtures/blast");
    pipeline::run(&root, Options { quiet: true }).expect("the fixture analyses")
}

/// Every edge the analysis produced, bundled.
fn bundle_of(analysis: &pipeline::Analysis) -> EvidenceBundle {
    let ids: Vec<_> = analysis
        .graph
        .edges()
        .map(cartograph_core::Edge::id)
        .collect();
    EvidenceBundle::from_edges(&analysis.graph, ids).expect("real evidence bundles")
}

#[test]
fn a_real_analysis_bundles_and_carries_every_relationship_it_found() {
    let analysis = analysed();
    assert!(
        analysis.graph.edge_count() > 0,
        "precondition: the fixture must produce relationships"
    );

    let bundle = bundle_of(&analysis);

    assert_eq!(bundle.len(), analysis.graph.edge_count());
    assert!(!bundle.is_empty());
}

#[test]
fn evidence_survives_bundling_byte_for_byte() {
    let analysis = analysed();
    let bundle = bundle_of(&analysis);

    for edge in analysis.graph.edges() {
        let bundled = bundle.get(edge.id()).expect("edge is bundled");
        assert_eq!(
            bundled.evidence,
            edge.evidence().as_str(),
            "bundling altered the claim the analysis made"
        );
        assert_eq!(bundled.provenance, edge.provenance());
        assert_eq!(bundled.kind, edge.kind());
    }
}

#[test]
fn a_real_bundle_is_identical_across_two_analyses_of_the_same_tree() {
    let first = bundle_of(&analysed());
    let second = bundle_of(&analysed());

    assert_eq!(first.scope(), second.scope(), "scope is not deterministic");
    assert_eq!(first, second, "bundle is not deterministic");
}

#[test]
fn every_claim_in_a_real_bundle_can_be_cited_verbatim() {
    let analysis = analysed();
    let bundle = bundle_of(&analysis);

    for bundled in bundle.edges() {
        // The whole claim, and a proper substring of it, both quote cleanly.
        let citation = bundle
            .cite(bundled.edge, &bundled.evidence)
            .expect("the evidence quotes itself");
        assert_eq!(bundle.verify(&citation), Ok(()));

        if let Some((head, _)) = bundled.evidence.split_once(' ') {
            assert!(
                bundle.cite(bundled.edge, head).is_ok(),
                "a real substring of real evidence must cite: {head}"
            );
        }
    }
}

#[test]
fn a_real_bundle_carries_no_machine_path_and_no_source_file() {
    let analysis = analysed();
    let bundle = bundle_of(&analysis);

    // The fixture's own source, which the bundle must not have copied.
    let root =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../cartograph-cli/tests/fixtures/blast");
    let client = std::fs::read_to_string(root.join("web/client.ts")).expect("fixture readable");
    let models = std::fs::read_to_string(root.join("api/models.py")).expect("fixture readable");

    let rendered = bundle
        .edges()
        .iter()
        .map(|e| {
            format!(
                "{}|{}|{}|{}",
                e.from.as_str(),
                e.to.as_str(),
                e.evidence,
                e.location.file()
            )
        })
        .collect::<Vec<_>>()
        .join("\n");

    for marker in ["/home/", "/Users/", "AppData", "runner/work"] {
        assert!(
            !rendered.contains(marker),
            "bundle carries a machine path fragment: {marker}"
        );
    }
    assert!(
        !rendered.contains(":\\"),
        "bundle carries a drive-rooted path"
    );
    for edge in bundle.edges() {
        assert!(
            !edge.location.file().starts_with('/'),
            "absolute location reached a bundle: {}",
            edge.location.file()
        );
    }

    // Whole source files are what must never appear. A single line of a
    // fixture can legitimately be echoed by evidence describing it, so the
    // assertion is about the file, not about every line in it.
    assert!(
        !rendered.contains(client.trim()),
        "bundle carries the contents of client.ts"
    );
    assert!(
        !rendered.contains(models.trim()),
        "bundle carries the contents of models.py"
    );
}
