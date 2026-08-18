//! The `cartograph` command-line interface.
//!
//! One of three clients of the same core graph API — the desktop application
//! (M11) and the MCP server (M15) are the others. The CLI holds no analysis
//! logic of its own; when `analyze` arrives at M09 it will call into the core
//! exactly as the other two clients do.
//!
//! # What exists at M04
//!
//! `cartograph version`; `cartograph parse`, the extractor over a file or tree
//! reporting syntactic facts and diagnostics; `cartograph normalize`, which
//! canonicalises the observations `parse` finds; and `cartograph match`, which
//! joins client calls to backend routes and reports the evidence.
//! `analyze` and `trace` are the deliverables of later milestones and are
//! deliberately absent rather than present and stubbed: a command that exists
//! but does not work is a documentation defect.

use std::path::PathBuf;

use anyhow::Result;
use clap::{Parser, Subcommand};
use serde::Serialize;
use tracing::info;
use tracing_subscriber::EnvFilter;

/// Compute the architecture. Prove the relationships.
#[derive(Debug, Parser)]
#[command(name = "cartograph", version, about, long_about = None)]
struct Cli {
    /// Increase log verbosity. Repeat for more detail (-v, -vv).
    ///
    /// Overridden by the `CARTOGRAPH_LOG` environment variable when set.
    #[arg(short, long, action = clap::ArgAction::Count, global = true)]
    verbose: u8,

    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Extract syntactic facts from TypeScript/TSX source (M01).
    ///
    /// Reports what the files say - symbols, imports, call sites, string and
    /// template structure, HTTP-looking call shapes - plus parse diagnostics.
    /// Facts are observations; nothing here is a resolved relationship.
    Parse {
        /// A TypeScript/TSX file, or a directory to walk.
        path: PathBuf,
        /// Emit the full fact model as JSON instead of a summary.
        #[arg(long)]
        json: bool,
    },
    /// Canonicalise the route and HTTP-call observations under a path (M03).
    ///
    /// Shows each observation's raw form beside its canonical form. Client
    /// calls are not matched against route declarations; that is M04.
    Normalize {
        /// A TypeScript/TSX/Python file, or a directory to walk.
        path: PathBuf,
        /// Emit the full canonical model as JSON instead of a summary.
        #[arg(long)]
        json: bool,
    },
    /// Match client HTTP calls against backend routes (M04).
    ///
    /// Reports each decision with its candidates, confidence and evidence.
    /// Ambiguous results produce no edge and say so.
    Match {
        /// A file, or a directory to walk.
        path: PathBuf,
        /// Emit the full match model as JSON instead of a summary.
        #[arg(long)]
        json: bool,
    },
    /// Print version and build information.
    Version {
        /// Emit JSON instead of text.
        #[arg(long)]
        json: bool,
    },
}

mod discovery;
mod match_cmd;
mod normalize_cmd;
mod parse_cmd;

/// Version information, in the shape the `--json` output promises.
#[derive(Debug, Serialize)]
struct VersionInfo {
    /// The released version of the binary.
    version: &'static str,
    /// The frozen specification version the domain model implements.
    spec_version: &'static str,
    /// The most recent completed milestone.
    milestone: &'static str,
}

impl VersionInfo {
    fn current() -> Self {
        Self {
            version: env!("CARGO_PKG_VERSION"),
            spec_version: cartograph_core::SPEC_VERSION,
            milestone: "M01",
        }
    }
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    init_tracing(cli.verbose);

    match cli.command {
        Command::Normalize { path, json } => normalize_cmd::run(&path, json)?,
        Command::Parse { path, json } => parse_cmd::run(&path, json)?,
        Command::Match { path, json } => match_cmd::run(&path, json)?,
        Command::Version { json } => version(json)?,
    }

    Ok(())
}

fn version(json: bool) -> Result<()> {
    let info = VersionInfo::current();

    if json {
        println!("{}", serde_json::to_string_pretty(&info)?);
    } else {
        println!("cartograph {}", info.version);
        println!("specification {}", info.spec_version);
        println!("milestone {}", info.milestone);
    }

    info!(version = info.version, "reported version");
    Ok(())
}

/// Configures logging.
///
/// Logs carry spans and structured fields, never source-file contents,
/// environment variable values or credentials (RULE 015). Nothing in this
/// binary is permitted to log a file's text.
fn init_tracing(verbosity: u8) {
    let default = match verbosity {
        0 => "warn",
        1 => "info",
        2 => "debug",
        _ => "trace",
    };

    // CARTOGRAPH_LOG rather than RUST_LOG: a developer analysing a Rust
    // project should be able to set RUST_LOG for their own program without
    // Cartograph's logging changing underneath them.
    let filter =
        EnvFilter::try_from_env("CARTOGRAPH_LOG").unwrap_or_else(|_| EnvFilter::new(default));

    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_writer(std::io::stderr)
        .init();
}

#[cfg(test)]
mod tests {
    use clap::CommandFactory;

    use super::*;

    #[test]
    fn the_command_tree_is_well_formed() {
        Cli::command().debug_assert();
    }

    #[test]
    fn version_json_reports_the_binarys_own_version() {
        let info = VersionInfo::current();
        assert_eq!(info.version, env!("CARGO_PKG_VERSION"));
        assert_eq!(info.spec_version, "V3");
        assert_eq!(info.milestone, "M01");
    }

    #[test]
    fn analysis_commands_are_absent_until_the_milestone_that_implements_them() {
        // The command set is a deliberate milestone decision: `parse` landed
        // with M01, `normalize` with M03, `match` with M04; `analyze`/`trace`
        // are M09 deliverables and must not appear to work before they do. If
        // this test fails, the scope moved.
        let command = Cli::command();
        let names: Vec<_> = command
            .get_subcommands()
            .map(clap::Command::get_name)
            .collect();
        // Declaration order, which is also pipeline order in `--help`:
        // parse produces observations, normalize canonicalises them.
        assert_eq!(names, vec!["parse", "normalize", "match", "version"]);
    }
}
