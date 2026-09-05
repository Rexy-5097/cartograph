//! MAP: what is this system?
//!
//! # The question, and why the obvious answers were rejected
//!
//! `README.md` defines MAP as the surface answering *"what is this system?"*.
//! [ADR-0020](../../../docs/adr/ADR-0020-mcp-boundary-and-authorization.md)
//! chose option **A4** for its MCP form — *a purpose-built, filtered and
//! aggregated representation* — and named what it must not be:
//!
//! - **not the raw graph.** Airflow serialises to 2,054,974 bytes. An answer
//!   nobody can read is not an answer;
//! - **not the desktop `Scene`.** That belongs to a peer client and carries
//!   x/y coordinates, which mean nothing to a reader who cannot draw;
//! - **not `Totals`.** Those answer a different question — *"can Cartograph
//!   analyse this repository, and what did it find?"* — and relabelling them
//!   MAP would make the tool claim something it does not do.
//!
//! # Why this lives in the pipeline rather than in the graph
//!
//! A map that could not say *"TypeScript talks to Python here"* would miss the
//! thing Cartograph exists to show. Language classification is
//! `cartograph_parser::SourceLanguage`, and `cartograph-graph` depends on
//! neither the parser nor this crate. This crate already depends on both, so
//! the capability lands here — and it is analysis, so it is not in a client
//! (RULE 002).
//!
//! # What every field is for
//!
//! Aggregates only. No source text, no absolute paths — `SourceLocation`
//! refuses those at construction, and nothing here reassembles one. Every list
//! is **bounded** and **ordered deterministically**, because a map that
//! reshuffled between runs would make every re-read look like a change.
//!
//! | Field | The question it answers |
//! |---|---|
//! | `languages` | what is this written in |
//! | `artefacts` | what kinds of thing are in it |
//! | `relationships` | how do they relate |
//! | `areas` | what are its parts |
//! | `boundaries` | where do the languages meet — the product's whole thesis |
//! | `hubs` | what would you read first |

use std::collections::BTreeMap;

use cartograph_core::{NodeId, identity::kind_token};
use cartograph_graph::{ArchitectureGraph, diff::edge_kind_token};
use cartograph_parser::model::SourceLanguage;
use serde::Serialize;

/// How many areas a map reports. Beyond this the list stops being a map and
/// starts being a directory listing.
const MAX_AREAS: usize = 20;

/// How many hubs a map reports.
const MAX_HUBS: usize = 20;

/// A named count.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Tally {
    /// The domain token — `function`, `orm-access`. Never a display label:
    /// `summary_cmd` keys its counts on presentation strings, which merges
    /// every kind sharing a fallback label, and that is a rendering decision
    /// baked into a number.
    pub kind: String,
    /// How many.
    pub count: usize,
}

/// One part of the system, as its top-level source directory.
///
/// Directory structure is the v1 clustering prior — the same choice
/// [ADR-0015](../../../docs/adr/ADR-0015-layout-contract.md) made for layout,
/// and for the same reason: it is usually right and it is never a guess.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Area {
    /// Repository-relative first path segment, or `.` for files at the root.
    pub path: String,
    /// Artefacts whose location falls inside it.
    pub artefacts: usize,
}

/// A place where two languages meet.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Boundary {
    /// The language the relationship starts in.
    pub from: String,
    /// The language it lands in.
    pub to: String,
    /// What kind of relationship crosses.
    pub kind: String,
    /// How many do.
    pub count: usize,
}

/// A well-connected artefact — somewhere a reader would start.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Hub {
    /// The artefact's name.
    pub name: String,
    /// What kind of artefact.
    pub kind: String,
    /// Repository-relative file, when it has a location.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file: Option<String>,
    /// Relationships in and out.
    pub degree: usize,
}

/// The whole-system counts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct MapTotals {
    /// Artefacts in the graph.
    pub artefacts: usize,
    /// Relationships in the graph.
    pub relationships: usize,
    /// Relationships whose endpoints are in different languages.
    pub cross_language: usize,
}

/// A compact, deterministic answer to *"what is this system?"*.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SystemMap {
    /// Whole-system counts.
    pub totals: MapTotals,
    /// Languages present, by artefact count, most first.
    pub languages: Vec<Tally>,
    /// Artefact kinds, by count, most first.
    pub artefacts: Vec<Tally>,
    /// Relationship kinds, by count, most first.
    pub relationships: Vec<Tally>,
    /// The system's parts, by artefact count, most first. Bounded.
    pub areas: Vec<Area>,
    /// Where languages meet, most first.
    pub boundaries: Vec<Boundary>,
    /// The best-connected artefacts, most first. Bounded.
    pub hubs: Vec<Hub>,
    /// Whether any list was cut short, so a reader is never quietly shown
    /// less than exists.
    pub truncated: Truncation,
}

/// What a bounded list left out.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct Truncation {
    /// Areas beyond the internal `MAX_AREAS` cap.
    pub areas_omitted: usize,
    /// Hubs beyond the internal `MAX_HUBS` cap.
    pub hubs_omitted: usize,
}

/// The language a repository-relative file is written in.
///
/// Rendered through `SourceLanguage`'s own `Display`, deliberately. The enum is
/// `#[non_exhaustive]`, so a `match` here would need a wildcard arm — and a
/// wildcard would quietly label a language added later as something it is not.
fn language_of(file: &str) -> Option<String> {
    SourceLanguage::from_path(file).map(|language| language.to_string())
}

/// The top-level directory a repository-relative file sits in.
fn area_of(file: &str) -> String {
    match file.split_once('/') {
        Some((head, _)) if !head.is_empty() => head.to_owned(),
        _ => ".".to_owned(),
    }
}

/// Turns a count map into a list ordered by count, then by name.
///
/// The tie-break is what makes the order total: two kinds with equal counts
/// must not swap between runs.
fn ranked(counts: BTreeMap<String, usize>) -> Vec<Tally> {
    let mut out: Vec<Tally> = counts
        .into_iter()
        .map(|(kind, count)| Tally { kind, count })
        .collect();
    out.sort_by(|a, b| b.count.cmp(&a.count).then_with(|| a.kind.cmp(&b.kind)));
    out
}

/// What one pass over the nodes learns.
struct NodePass {
    languages: BTreeMap<String, usize>,
    artefacts: BTreeMap<String, usize>,
    areas: BTreeMap<String, usize>,
    node_language: BTreeMap<NodeId, String>,
}

/// Counts artefacts by kind, area and language, in one pass.
fn walk_nodes(graph: &ArchitectureGraph) -> NodePass {
    let mut pass = NodePass {
        languages: BTreeMap::new(),
        artefacts: BTreeMap::new(),
        areas: BTreeMap::new(),
        node_language: BTreeMap::new(),
    };

    for node in graph.nodes() {
        *pass
            .artefacts
            .entry(kind_token(node.kind()).to_owned())
            .or_insert(0) += 1;

        if let Some(location) = node.location() {
            *pass.areas.entry(area_of(location.file())).or_insert(0) += 1;
            if let Some(language) = language_of(location.file()) {
                *pass.languages.entry(language.clone()).or_insert(0) += 1;
                pass.node_language.insert(node.id(), language);
            }
        }
    }

    pass
}

/// What one pass over the edges learns.
struct EdgePass {
    relationships: BTreeMap<String, usize>,
    boundaries: BTreeMap<(String, String, String), usize>,
    degree: BTreeMap<NodeId, usize>,
    cross_language: usize,
}

/// Counts relationships, degrees and language boundaries, in one pass.
///
/// A boundary is an edge whose endpoints are in files of different languages —
/// the thing Cartograph exists to show, so it is counted rather than inferred
/// from the kind.
fn walk_edges(graph: &ArchitectureGraph, node_language: &BTreeMap<NodeId, String>) -> EdgePass {
    let mut pass = EdgePass {
        relationships: BTreeMap::new(),
        boundaries: BTreeMap::new(),
        degree: BTreeMap::new(),
        cross_language: 0,
    };

    for edge in graph.edges() {
        *pass
            .relationships
            .entry(edge_kind_token(edge.kind()).to_owned())
            .or_insert(0) += 1;
        *pass.degree.entry(edge.source()).or_insert(0) += 1;
        *pass.degree.entry(edge.target()).or_insert(0) += 1;

        if let (Some(from), Some(to)) = (
            node_language.get(&edge.source()),
            node_language.get(&edge.target()),
        ) && from != to
        {
            pass.cross_language += 1;
            *pass
                .boundaries
                .entry((
                    from.clone(),
                    to.clone(),
                    edge_kind_token(edge.kind()).to_owned(),
                ))
                .or_insert(0) += 1;
        }
    }

    pass
}

/// Builds the map.
///
/// Deterministic: every list is totally ordered, so two runs over one graph
/// produce identical output.
#[must_use]
pub fn system_map(graph: &ArchitectureGraph) -> SystemMap {
    let artefact_pass = walk_nodes(graph);
    let NodePass {
        languages,
        artefacts,
        areas,
        node_language,
    } = artefact_pass;

    let EdgePass {
        relationships,
        boundaries,
        degree,
        cross_language,
    } = walk_edges(graph, &node_language);

    // Areas: by size, then by path so equal sizes cannot swap.
    let mut ranked_areas: Vec<Area> = areas
        .into_iter()
        .map(|(path, artefacts)| Area { path, artefacts })
        .collect();
    ranked_areas.sort_by(|a, b| {
        b.artefacts
            .cmp(&a.artefacts)
            .then_with(|| a.path.cmp(&b.path))
    });
    let areas_omitted = ranked_areas.len().saturating_sub(MAX_AREAS);
    ranked_areas.truncate(MAX_AREAS);

    let mut ranked_boundaries: Vec<Boundary> = boundaries
        .into_iter()
        .map(|((from, to, kind), count)| Boundary {
            from,
            to,
            kind,
            count,
        })
        .collect();
    ranked_boundaries.sort_by(|a, b| {
        b.count
            .cmp(&a.count)
            .then_with(|| (&a.from, &a.to, &a.kind).cmp(&(&b.from, &b.to, &b.kind)))
    });

    // Hubs: by degree, then by (name, kind, file) — never by NodeId, which is
    // graph-local and would make the order an artefact of allocation.
    let mut ranked_hubs: Vec<Hub> = degree
        .into_iter()
        .filter_map(|(id, degree)| {
            graph.node(id).map(|node| Hub {
                name: node.name().to_owned(),
                kind: kind_token(node.kind()).to_owned(),
                file: node.location().map(|l| l.file().to_owned()),
                degree,
            })
        })
        .collect();
    ranked_hubs.sort_by(|a, b| {
        b.degree
            .cmp(&a.degree)
            .then_with(|| (&a.name, &a.kind, &a.file).cmp(&(&b.name, &b.kind, &b.file)))
    });
    let hubs_omitted = ranked_hubs.len().saturating_sub(MAX_HUBS);
    ranked_hubs.truncate(MAX_HUBS);

    SystemMap {
        totals: MapTotals {
            artefacts: graph.node_count(),
            relationships: graph.edge_count(),
            cross_language,
        },
        languages: ranked(languages),
        artefacts: ranked(artefacts),
        relationships: ranked(relationships),
        areas: ranked_areas,
        boundaries: ranked_boundaries,
        hubs: ranked_hubs,
        truncated: Truncation {
            areas_omitted,
            hubs_omitted,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use cartograph_core::{Confidence, EdgeKind, Evidence, NodeKind, Provenance, SourceLocation};
    use cartograph_graph::EdgeSpec;

    fn graph_of(edges: &[(&str, &str, &str, &str)]) -> ArchitectureGraph {
        let mut graph = ArchitectureGraph::new();
        for (from, from_file, to, to_file) in edges {
            let source = graph
                .node_for(
                    NodeKind::Function,
                    *from,
                    Some(SourceLocation::new(*from_file, 1).expect("location")),
                )
                .expect("node");
            let target = graph
                .node_for(
                    NodeKind::Function,
                    *to,
                    Some(SourceLocation::new(*to_file, 1).expect("location")),
                )
                .expect("node");
            graph
                .add_edge(EdgeSpec {
                    source,
                    target,
                    kind: EdgeKind::HttpCall,
                    confidence: Confidence::new(0.9).expect("confidence"),
                    provenance: Provenance::RouteMatcher,
                    evidence: Evidence::new(format!("{from} calls {to}")).expect("evidence"),
                    location: SourceLocation::new(*from_file, 1).expect("location"),
                    commit: None,
                })
                .expect("edge");
        }
        graph
    }

    #[test]
    fn an_empty_graph_maps_to_empty_lists_rather_than_a_failure() {
        let map = system_map(&ArchitectureGraph::new());
        assert_eq!(map.totals.artefacts, 0);
        assert_eq!(map.totals.relationships, 0);
        assert!(map.languages.is_empty() && map.hubs.is_empty());
    }

    #[test]
    fn the_map_counts_languages_areas_and_kinds() {
        let graph = graph_of(&[
            ("button", "web/ui/button.ts", "handler", "api/routes.py"),
            ("form", "web/ui/form.ts", "handler", "api/routes.py"),
        ]);
        let map = system_map(&graph);

        assert_eq!(map.totals.artefacts, 3);
        assert_eq!(map.totals.relationships, 2);
        assert_eq!(
            map.languages,
            vec![
                Tally {
                    kind: "typescript".to_owned(),
                    count: 2
                },
                Tally {
                    kind: "python".to_owned(),
                    count: 1
                },
            ]
        );
        assert_eq!(
            map.areas,
            vec![
                Area {
                    path: "web".to_owned(),
                    artefacts: 2
                },
                Area {
                    path: "api".to_owned(),
                    artefacts: 1
                },
            ]
        );
        assert_eq!(
            map.artefacts,
            vec![Tally {
                kind: "function".to_owned(),
                count: 3
            }]
        );
    }

    /// The product's thesis, in one field: TypeScript reaching Python.
    #[test]
    fn a_cross_language_relationship_is_reported_as_a_boundary() {
        let graph = graph_of(&[("button", "web/button.ts", "handler", "api/routes.py")]);
        let map = system_map(&graph);

        assert_eq!(map.totals.cross_language, 1);
        assert_eq!(
            map.boundaries,
            vec![Boundary {
                from: "typescript".to_owned(),
                to: "python".to_owned(),
                kind: "http-call".to_owned(),
                count: 1,
            }]
        );
    }

    #[test]
    fn a_same_language_relationship_is_not_a_boundary() {
        let graph = graph_of(&[("a", "api/a.py", "b", "api/b.py")]);
        let map = system_map(&graph);

        assert_eq!(map.totals.cross_language, 0);
        assert!(map.boundaries.is_empty());
    }

    #[test]
    fn hubs_are_ordered_by_degree() {
        let graph = graph_of(&[
            ("a", "api/a.py", "hub", "api/hub.py"),
            ("b", "api/b.py", "hub", "api/hub.py"),
            ("c", "api/c.py", "hub", "api/hub.py"),
        ]);
        let map = system_map(&graph);

        assert_eq!(map.hubs[0].name, "hub");
        assert_eq!(map.hubs[0].degree, 3);
    }

    /// A map that reshuffled between runs would make every re-read look like a
    /// change to whoever is reading it.
    #[test]
    fn the_map_is_byte_identical_across_runs() {
        let graph = graph_of(&[
            ("button", "web/button.ts", "handler", "api/routes.py"),
            ("form", "web/form.ts", "handler", "api/routes.py"),
            ("handler", "api/routes.py", "model", "api/models.py"),
        ]);

        let first = serde_json::to_string(&system_map(&graph)).expect("json");
        for _ in 0..5 {
            assert_eq!(
                first,
                serde_json::to_string(&system_map(&graph)).expect("json")
            );
        }
    }

    #[test]
    fn a_bounded_list_says_what_it_left_out() {
        let mut edges = Vec::new();
        let files: Vec<(String, String)> = (0..MAX_AREAS + 5)
            .map(|i| (format!("area{i}/a.py"), format!("area{i}/b.py")))
            .collect();
        for (from, to) in &files {
            edges.push((from.as_str(), from.as_str(), to.as_str(), to.as_str()));
        }
        let graph = graph_of(&edges);
        let map = system_map(&graph);

        assert_eq!(map.areas.len(), MAX_AREAS);
        assert_eq!(map.truncated.areas_omitted, 5);
    }

    /// `SourceLocation` refuses an absolute path at construction, so a map
    /// built from a graph cannot contain one. This pins that the map adds no
    /// path of its own.
    #[test]
    fn the_map_carries_no_absolute_path() {
        let graph = graph_of(&[("a", "api/a.py", "b", "web/b.ts")]);
        let json = serde_json::to_string(&system_map(&graph)).expect("json");

        assert!(!json.contains("C:\\"), "{json}");
        assert!(!json.contains("/home/"), "{json}");
        assert!(!json.contains("/Users/"), "{json}");
    }

    /// The map is an aggregate. It must not become a way to read the source.
    #[test]
    fn the_map_carries_no_evidence_or_source_text() {
        let graph = graph_of(&[("a", "api/a.py", "b", "api/b.py")]);
        let json = serde_json::to_string(&system_map(&graph)).expect("json");

        assert!(!json.contains("calls"), "evidence text leaked: {json}");
        assert!(!json.contains("confidence"), "{json}");
    }
}
