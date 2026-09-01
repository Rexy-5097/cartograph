//! Turning a validated folder into something the window can draw.
//!
//! # What this is, and what it deliberately is not
//!
//! This is orchestration. It calls `cartograph_pipeline::pipeline::run` and
//! `cartograph_graph::layout::layout`, in that order, and shapes the result.
//! **It contains no analysis**: no parsing, no resolution, no clustering, no
//! positioning. If a question about the architecture of a repository is being
//! answered here, it is in the wrong place (RULE 002, ADR-0001).
//!
//! The same rule runs one step further out: the Tauri layer above calls into
//! this module and does nothing else, and the React layer above that draws
//! what it is given.
//!
//! # Cancellation
//!
//! There is none, and saying so plainly is better than implying otherwise.
//! `pipeline::run` is a synchronous call with no interruption point, so
//! "cancel" means the frontend stops waiting and discards the result the
//! moment it arrives. The analysis itself runs to completion.
//!
//! That is safe, because a session holds no state that a discarded result
//! could corrupt — [`analyze`] is a pure function of the path, and each call
//! builds its own caches. It is *not* free: a cancelled analysis of a large
//! repository keeps a core busy until it finishes. Interrupting the analyser
//! would mean threading a cancellation token through the pipeline, which is a
//! change to M10's accepted code and does not belong in a shell slice.

use serde::{Deserialize, Serialize};

use cartograph_graph::layout::{self, LayoutParams};

use crate::scene::{self, Scene};
use cartograph_pipeline::pipeline::{self, Options};

use crate::error::DesktopError;
use crate::repository::ValidatedRepository;

/// What was found, in the quantities a window puts on screen.
///
/// Counts only. No file names, no source text, no paths.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AnalysisSummary {
    /// Files the analyser read.
    pub files: usize,
    /// Files that could not be parsed. Not a failure: real repositories
    /// contain files no parser handles, and M09 made that a reported outcome
    /// rather than an abort.
    pub failed: usize,
    /// Nodes in the resulting graph.
    pub nodes: usize,
    /// Edges in the resulting graph.
    pub edges: usize,
    /// Clusters the layout grouped those nodes into.
    pub clusters: usize,
}

/// A completed analysis, ready to hand to the window.
///
/// The layout is carried whole because it is already exactly the drawing
/// contract ADR-0015 defines — `{id, x, y, cluster}` per node and
/// `{id, label}` per cluster. Nothing is added to it here; a renderer slice
/// that needs more must widen that contract deliberately rather than have this
/// module quietly attach fields.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AnalysisPayload {
    /// The safe-to-show repository name. Never an absolute path.
    pub repository: String,
    /// Quantities for the summary line.
    pub summary: AnalysisSummary,
    /// Everything the renderer draws: positioned nodes, edges and clusters.
    ///
    /// Composed from the graph and the layout by [`crate::scene::compose`];
    /// coordinates are copied out of the layout without arithmetic.
    pub scene: Scene,
}

/// Analyses a validated repository and lays the result out.
///
/// Taking [`ValidatedRepository`] rather than a path is the point: an
/// unchecked path cannot reach the analyser, because there is no way to
/// construct one without passing [`crate::repository::validate`].
///
/// # Errors
///
/// Returns whatever the pipeline reported, mapped onto
/// [`crate::error::DesktopErrorKind`]. A repository holding no supported
/// source is an error rather than an empty drawing, because a blank window
/// with no explanation is the worst of both.
pub fn analyze(repository: &ValidatedRepository) -> Result<AnalysisPayload, DesktopError> {
    let analysis = pipeline::run(repository.path(), Options { quiet: true })?;

    let layout = layout::layout(&analysis.graph, &LayoutParams::default());
    let scene = scene::compose(&analysis.graph, &layout);

    Ok(AnalysisPayload {
        repository: repository.display_name().to_owned(),
        summary: AnalysisSummary {
            files: analysis.totals.files,
            failed: analysis.totals.files_failed,
            nodes: analysis.graph.node_count(),
            edges: analysis.graph.edge_count(),
            clusters: layout.cluster_count(),
        },
        scene,
    })
}
