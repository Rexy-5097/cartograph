//! Cartograph's MCP server: the graph of one authorized repository, and
//! nothing else.
//!
//! # What this crate is, and what it is not
//!
//! A **client**, on equal footing with the CLI and the desktop shell
//! ([ADR-0001](../../../docs/adr/ADR-0001-rust-core-is-the-product.md),
//! [ADR-0016](../../../docs/adr/ADR-0016-desktop-boundary.md)). It holds no
//! analysis logic: `map` is `cartograph_pipeline::map`, `trace` is
//! `cartograph_graph::trace`, `blast` is `cartograph_graph::blast`, `diff` is
//! `cartograph_graph::diff`. If a question about a repository's architecture
//! were answered here it would be in the wrong place (RULE 002).
//!
//! It does **not** depend on `cartograph-cli`, which is a binary crate with no
//! library target — the fact that forced
//! [ADR-0019](../../../docs/adr/ADR-0019-trace-belongs-to-the-graph.md).
//!
//! # The security invariant
//!
//! > One MCP session has exactly one authorized repository identity. Every
//! > graph query is bound to that identity. A query naming any other
//! > repository is refused **before any analysis runs**.
//!
//! Enforcement is not here. It is in
//! `cartograph_pipeline::authorization`, *below* the transport, so that no
//! client can bypass it by talking to something else
//! ([ADR-0020](../../../docs/adr/ADR-0020-mcp-boundary-and-authorization.md)).
//! This crate holds the session and hands every request through that guard.
//!
//! # Where authorization comes from
//!
//! **argv, at process start** — ADR-0020 Amendment 2, option G1. The grant is
//! complete before the first byte of MCP protocol is read, so:
//!
//! - there is no tool to set, switch or add a repository, and adding one would
//!   make the guarded party its own authority;
//! - the session's [`AuthorizedRepository`] is built once and never replaced;
//! - a request naming an ungranted tree is refused without touching the disk.
//!
//! The identity value itself is **opaque**. This crate never interprets,
//! canonicalises or hashes it, and never prints it. Its derivation is still
//! an open question in ADR-0020, and nothing here may be read as settling it.
//!
//! # stdout and stderr
//!
//! stdout carries the MCP protocol and nothing else; diagnostics go to stderr.
//! That is the same split `cartograph-cli` has had since M09, and here it is
//! load-bearing rather than tidy: one stray `println!` corrupts the protocol
//! stream.

use std::path::{Path, PathBuf};

use cartograph_pipeline::authorization::{
    AuthorizationError, AuthorizedRepository, AuthorizedTree, RepositoryIdentity,
};
use cartograph_pipeline::error::CliError;
use cartograph_pipeline::pipeline::{Analysis, Options};
use cartograph_pipeline::{authorization, map};

pub mod server;

/// What the launcher granted this process.
///
/// Built from argv and nothing else. There is no constructor that reads a
/// configuration file or an environment variable — not because those are
/// unimaginable, but because ADR-0020 Amendment 2 chose argv and a second
/// channel would be a second thing to secure.
#[derive(Debug, Clone)]
pub struct Grant {
    identity: String,
    trees: Vec<PathBuf>,
}

impl Grant {
    /// Builds a grant from an opaque identity and the trees it covers.
    #[must_use]
    pub fn new(identity: impl Into<String>, trees: Vec<PathBuf>) -> Self {
        Self {
            identity: identity.into(),
            trees,
        }
    }

    /// Turns the grant into the session's authorization state.
    ///
    /// # Errors
    ///
    /// [`AuthorizationError::EmptyIdentity`] or
    /// [`AuthorizationError::EmptyGrant`] when the launcher supplied neither an
    /// identity nor a tree. Refused at startup rather than becoming a session
    /// that answers nothing for a reason nobody can see.
    pub fn authorize(self) -> Result<AuthorizedRepository, AuthorizationError> {
        AuthorizedRepository::from_grant(RepositoryIdentity::from_grant(self.identity)?, self.trees)
    }
}

/// Why a query could not be answered.
///
/// Two shapes, kept apart deliberately: a refusal is *authorization* declining,
/// and it says nothing about the tree. An analysis failure is the pipeline's,
/// and its message is the one the CLI would print.
#[derive(Debug)]
pub enum QueryError {
    /// Authorization declined. Carries no path and no identity.
    Refused(AuthorizationError),
    /// The analyser could not do the work.
    Analysis(Box<CliError>),
    /// The request named something the graph does not contain.
    NotFound(String),
    /// The request was malformed.
    Invalid(String),
}

impl QueryError {
    /// A message safe to return to a client.
    #[must_use]
    pub fn message(&self) -> String {
        match self {
            Self::Refused(error) => error.message().to_owned(),
            // `CliError`'s own rendering is already path-redacted: M09's
            // Windows privacy fix made `discovery::is_rooted` the one predicate
            // every message goes through.
            Self::Analysis(error) => error.to_string(),
            Self::NotFound(what) | Self::Invalid(what) => what.clone(),
        }
    }
}

impl From<AuthorizationError> for QueryError {
    fn from(error: AuthorizationError) -> Self {
        Self::Refused(error)
    }
}

impl From<CliError> for QueryError {
    fn from(error: CliError) -> Self {
        Self::Analysis(Box::new(error))
    }
}

/// One MCP session: exactly one authorized repository, for the process's life.
///
/// There is no method that replaces or widens the authorization, and that is
/// the point — the type is the enforcement of *"the MCP client is not the
/// authority"*.
#[derive(Debug)]
pub struct Session {
    authorized: AuthorizedRepository,
}

impl Session {
    /// Opens a session over an already-authorized repository.
    #[must_use]
    pub const fn new(authorized: AuthorizedRepository) -> Self {
        Self { authorized }
    }

    /// How many trees the grant covers. Never which.
    #[must_use]
    pub fn tree_count(&self) -> usize {
        self.authorized.tree_count()
    }

    /// Admits a tree, or refuses before anything is read.
    fn admit(&self, tree: &Path) -> Result<AuthorizedTree, QueryError> {
        Ok(self.authorized.authorize(tree)?)
    }

    /// Analyses an admitted tree.
    fn analyse(&self, tree: &Path) -> Result<Analysis, QueryError> {
        let admitted = self.admit(tree)?;
        Ok(authorization::analyze(&admitted, Options { quiet: true })?)
    }

    /// **map** — what is this system?
    ///
    /// # Errors
    ///
    /// Refused when the tree was not granted; otherwise whatever the pipeline
    /// could not do.
    pub fn map(&self, tree: &Path) -> Result<map::SystemMap, QueryError> {
        Ok(map::system_map(&self.analyse(tree)?.graph))
    }

    /// **trace** — follow one symbol's relationships outward.
    ///
    /// # Errors
    ///
    /// Refused, not found, ambiguous, or an analysis failure.
    pub fn trace(
        &self,
        tree: &Path,
        symbol: &str,
        max_depth: usize,
    ) -> Result<TraceAnswer, QueryError> {
        use cartograph_graph::trace;

        let analysis = self.analyse(tree)?;
        let graph = &analysis.graph;

        let start = trace::resolve_symbol(graph, symbol).map_err(|error| match error {
            trace::SymbolError::NotFound { near } => QueryError::NotFound(format!(
                "no symbol named `{symbol}` is in the graph{}",
                describe_candidates(graph, &near, "; near matches: ")
            )),
            trace::SymbolError::Ambiguous { matches } => QueryError::NotFound(format!(
                "`{symbol}` matches {} symbols{}",
                matches.len(),
                describe_candidates(graph, &matches, ": ")
            )),
        })?;

        let walk = trace::trace(graph, start, max_depth);
        Ok(TraceAnswer {
            from: describe_node(graph, walk.from),
            max_depth: walk.max_depth,
            reaches_table: reaches_table(graph, &walk.steps),
            steps: steps_of(graph, &walk.steps),
            stop: walk.stop.map(stop_token),
        })
    }

    /// **blast** — what breaks if this changes?
    ///
    /// # Errors
    ///
    /// Refused, not found, or an analysis failure.
    pub fn blast(&self, tree: &Path, symbol: &str) -> Result<BlastAnswer, QueryError> {
        use cartograph_graph::{blast, trace};

        let analysis = self.analyse(tree)?;
        let graph = &analysis.graph;

        let target = trace::resolve_symbol(graph, symbol).map_err(|error| match error {
            trace::SymbolError::NotFound { .. } => {
                QueryError::NotFound(format!("no symbol named `{symbol}` is in the graph"))
            }
            trace::SymbolError::Ambiguous { matches } => QueryError::NotFound(format!(
                "`{symbol}` matches {} symbols{}",
                matches.len(),
                describe_candidates(graph, &matches, ": ")
            )),
        })?;

        let radius = blast::blast_radius(graph, target);
        Ok(BlastAnswer {
            target: describe_node(graph, target),
            dependents: radius
                .reached
                .iter()
                .map(|entry| Dependent {
                    node: describe_node(graph, entry.node),
                    confidence: entry.confidence,
                    depth: entry.depth,
                })
                .collect(),
        })
    }

    /// **diff** — what did this change do to the architecture?
    ///
    /// Both trees are authorized **before either analysis runs**: `before` from
    /// one repository and `after` from another is refused whole, in either
    /// position, because the second was never granted.
    ///
    /// # Errors
    ///
    /// Refused when either tree was not granted; otherwise an analysis failure.
    pub fn diff(&self, before: &Path, after: &Path) -> Result<DiffAnswer, QueryError> {
        use cartograph_graph::diff;

        // Both, first. This ordering is the diff rule.
        let pair = self.authorized.authorize_pair(before, after)?;
        let (left, right) = authorization::analyze_pair(&pair, Options { quiet: true })?;

        let result = diff::diff(&left.graph, &right.graph);
        Ok(DiffAnswer {
            added: result.added.len(),
            removed: result.removed.len(),
            changed: result.changed.len(),
            unchanged: result.unchanged,
        })
    }
}

// ── answer shapes ───────────────────────────────────────────────────
//
// Deliberately small. An MCP response is read by an agent with a context
// budget, so these carry what answers the question and stop.

/// One artefact, named the way a reader recognises it.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct NodeRef {
    /// The artefact's name.
    pub name: String,
    /// What kind of artefact.
    pub kind: String,
    /// Repository-relative file, when it has one. Never absolute:
    /// `SourceLocation` refuses those at construction.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file: Option<String>,
    /// One-based line, when it has one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line: Option<u32>,
}

/// One hop of a trace.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct Step {
    /// What kind of relationship was traversed.
    pub kind: String,
    /// An uncalibrated prior, not a probability.
    pub confidence: f32,
    /// How the relationship was determined.
    pub provenance: String,
    /// The observation behind it.
    pub evidence: String,
    /// Where it led.
    pub to: NodeRef,
    /// Further hops.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub next: Vec<Step>,
    /// Why the walk stopped here, when it did.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop: Option<&'static str>,
}

/// A completed trace.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct TraceAnswer {
    /// Where the walk began.
    pub from: NodeRef,
    /// The hops.
    pub steps: Vec<Step>,
    /// Whether any branch reached a database table.
    pub reaches_table: bool,
    /// Why the walk stopped immediately, when it did.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop: Option<&'static str>,
    /// The depth limit in force.
    pub max_depth: usize,
}

/// One artefact that would be affected.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct Dependent {
    /// The artefact.
    pub node: NodeRef,
    /// The weakest link along its best-supported route — an uncalibrated
    /// prior, not a probability ([ADR-0018](../../../docs/adr/ADR-0018-blast-radius-confidence.md)).
    pub confidence: f32,
    /// How many hops away.
    pub depth: u32,
}

/// A blast radius.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct BlastAnswer {
    /// The artefact asked about.
    pub target: NodeRef,
    /// What depends on it, most-affected first.
    pub dependents: Vec<Dependent>,
}

/// A structural diff, in counts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
pub struct DiffAnswer {
    /// Relationships only in the after tree.
    pub added: usize,
    /// Relationships only in the before tree.
    pub removed: usize,
    /// Relationships in both, with different evidence.
    pub changed: usize,
    /// Relationships in both, unchanged.
    pub unchanged: usize,
}

// ── shaping helpers ─────────────────────────────────────────────────

fn describe_node(
    graph: &cartograph_graph::ArchitectureGraph,
    id: cartograph_core::NodeId,
) -> NodeRef {
    graph.node(id).map_or_else(
        || NodeRef {
            name: "<unknown>".to_owned(),
            kind: "node".to_owned(),
            file: None,
            line: None,
        },
        |node| NodeRef {
            name: node.name().to_owned(),
            kind: cartograph_core::identity::kind_token(node.kind()).to_owned(),
            file: node.location().map(|l| l.file().to_owned()),
            line: node.location().map(cartograph_core::SourceLocation::line),
        },
    )
}

/// Renders candidate artefacts for a not-found or ambiguous answer.
///
/// Repository-relative locations only, and bounded: a graph with a thousand
/// near matches must not turn a refusal into a listing.
fn describe_candidates(
    graph: &cartograph_graph::ArchitectureGraph,
    ids: &[cartograph_core::NodeId],
    prefix: &str,
) -> String {
    const MAX: usize = 10;
    if ids.is_empty() {
        return String::new();
    }
    let mut described: Vec<String> = ids
        .iter()
        .map(|id| {
            let node = describe_node(graph, *id);
            node.file.map_or_else(
                || format!("{} ({})", node.name, node.kind),
                |file| format!("{} ({}) {file}", node.name, node.kind),
            )
        })
        .collect();
    described.sort();
    described.dedup();
    let omitted = described.len().saturating_sub(MAX);
    described.truncate(MAX);
    let more = if omitted > 0 {
        format!(" and {omitted} more")
    } else {
        String::new()
    };
    format!("{prefix}{}{more}", described.join(", "))
}

const fn stop_token(stop: cartograph_graph::trace::Stop) -> &'static str {
    match stop {
        cartograph_graph::trace::Stop::ChainEnd => "chain-end",
        cartograph_graph::trace::Stop::MaxDepth => "max-depth",
        cartograph_graph::trace::Stop::Cycle => "cycle",
    }
}

fn steps_of(
    graph: &cartograph_graph::ArchitectureGraph,
    steps: &[cartograph_graph::trace::TraceStep],
) -> Vec<Step> {
    steps
        .iter()
        .filter_map(|step| {
            let edge = graph.edge(step.edge)?;
            Some(Step {
                kind: cartograph_graph::diff::edge_kind_token(edge.kind()).to_owned(),
                confidence: edge.confidence().get(),
                provenance: format!("{:?}", edge.provenance()),
                evidence: edge.evidence().as_str().to_owned(),
                to: describe_node(graph, step.to),
                next: steps_of(graph, &step.next),
                stop: step.stop.map(stop_token),
            })
        })
        .collect()
}

fn reaches_table(
    graph: &cartograph_graph::ArchitectureGraph,
    steps: &[cartograph_graph::trace::TraceStep],
) -> bool {
    steps.iter().any(|step| {
        graph
            .node(step.to)
            .is_some_and(|node| node.kind() == cartograph_core::NodeKind::Table)
            || reaches_table(graph, &step.next)
    })
}
