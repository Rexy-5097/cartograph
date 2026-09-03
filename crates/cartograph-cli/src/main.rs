//! The `cartograph` command-line interface.
//!
//! One of three planned clients of the same core graph API — the desktop
//! application and the MCP server are the others. The CLI holds no analysis
//! logic of its own: every command composes library calls, and `pipeline`
//! runs the same sequence for all of them so two commands cannot report on
//! differently-built graphs.
//!
//! # The command surface at M09
//!
//! `cartograph <path>` — analyse a repository and summarise what was found.
//! `cartograph trace <symbol>` — follow one relationship across the stack.
//! `cartograph parse` — extracted syntactic facts and diagnostics.
//! `cartograph normalize` — each observation's raw form beside its canonical one.
//! `cartograph match` — every match decision, including the refusals.
//! `cartograph version` — release identity.
//!
//! The surface is deliberately small. A command that exists but does not work
//! is a documentation defect, so nothing is present as a stub.
//!
//! # Output discipline
//!
//! stdout carries the command's result and nothing else; diagnostics, progress
//! and logs go to stderr. Under `--json`, stdout is a single JSON document —
//! on success *and* on failure, so a consumer never has to decide whether to
//! parse. Colour is not used at all: plain text is what stays readable in a
//! pipe, a log file and a screen reader alike.

use std::path::PathBuf;

use clap::{Parser, Subcommand};

mod blast_cmd;
mod diff_cmd;
mod json;
mod match_cmd;
mod normalize_cmd;
mod output;
mod parse_cmd;
mod summary_cmd;
mod trace_cmd;
mod version;

// Analysis is composed once, in `cartograph-pipeline`, and every client uses
// that sequence rather than its own. Re-exported at the crate root so the
// command modules keep addressing them as `crate::pipeline` and friends: the
// move changed where the code lives, not what the CLI does with it.
pub(crate) use cartograph_pipeline::{discovery, error, pipeline};

use error::{CliError, ExitCode};

/// Compute the architecture. Prove the relationships.
#[derive(Debug, Parser)]
#[command(
    name = "cartograph",
    version,
    about,
    long_about = None,
    args_conflicts_with_subcommands = true,
    after_help = "\
Run `cartograph .` to analyse the current repository.

Exit codes:
  0  success            2  usage error       3  path or input error
  4  analysis error     5  symbol not found  6  ambiguous symbol
  7  partial analysis (only with --strict)"
)]
struct Cli {
    /// Repository to analyse. Use `.` for the current directory.
    ///
    /// Given on its own this runs the default summary.
    #[arg(value_name = "PATH")]
    path: Option<PathBuf>,

    /// Emit a JSON document instead of a human summary.
    ///
    /// stdout carries the JSON and nothing else, including when the command
    /// fails: an error is reported as a JSON document too.
    #[arg(long, global = true)]
    json: bool,

    /// Exit non-zero when some files could not be parsed.
    ///
    /// Off by default: real repositories contain files no parser handles, and
    /// a partial analysis is a normal, reported outcome rather than a failure.
    #[arg(long, global = true)]
    strict: bool,

    /// Increase log verbosity. Repeat for more detail (-v, -vv).
    ///
    /// Overridden by the `CARTOGRAPH_LOG` environment variable when set.
    #[arg(short, long, action = clap::ArgAction::Count, global = true)]
    verbose: u8,

    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Follow one symbol's relationships across the stack.
    ///
    /// Walks the graph outward from the named symbol, reporting each hop with
    /// its confidence, provenance, evidence and location. Where the chain
    /// stops, it says so rather than implying the path is complete.
    Trace {
        /// The symbol to trace. Qualify as `file:name` to disambiguate.
        #[arg(value_name = "SYMBOL")]
        symbol: String,
        /// Repository to analyse.
        #[arg(long, default_value = ".", value_name = "PATH")]
        path: PathBuf,
        /// How many hops to follow before stopping.
        #[arg(long, default_value_t = trace_cmd::DEFAULT_MAX_DEPTH, value_name = "N")]
        max_depth: usize,
    },
    /// Report what depends on a symbol, across language boundaries.
    ///
    /// Walks the graph *inward* to the named symbol: everything that can reach
    /// it through a dependency relationship, with the confidence of the
    /// best-supported route and one representative route per artefact. The
    /// inverse of `trace`, which walks outward.
    Blast {
        /// The symbol to query. Qualify as `file:name` to disambiguate.
        #[arg(value_name = "SYMBOL")]
        symbol: String,
        /// Repository to analyse.
        #[arg(long, default_value = ".", value_name = "PATH")]
        path: PathBuf,
    },
    /// Compare two checked-out trees and report what changed architecturally.
    ///
    /// Takes two directories rather than two revisions: Cartograph has no git
    /// dependency, so the caller decides how the trees came to exist -- `git
    /// worktree add` is the usual answer. Correspondence between the analyses
    /// is by stable `(kind, name, file)` identity, never by node id, so a
    /// relationship that merely moved down its file is not reported as churn.
    Diff {
        /// The earlier tree.
        #[arg(value_name = "BEFORE")]
        before: PathBuf,
        /// The later tree.
        #[arg(value_name = "AFTER")]
        after: PathBuf,
        /// Render the architecture review as Markdown, for a pull request
        /// comment. M14's Action runs the binary and posts what it returns,
        /// so the review is produced here rather than assembled in YAML.
        #[arg(long, conflicts_with = "json")]
        markdown: bool,
    },
    /// Extract syntactic facts from TypeScript, TSX and Python source.
    ///
    /// Reports what the files say — symbols, imports, call sites, string and
    /// template structure, HTTP-looking call shapes — plus parse diagnostics.
    /// Facts are observations; nothing here is a resolved relationship.
    Parse {
        /// A source file, or a directory to walk.
        path: PathBuf,
    },
    /// Canonicalise the route and HTTP-call observations under a path.
    ///
    /// Shows each observation's raw form beside its canonical form. Client
    /// calls are not matched against route declarations.
    Normalize {
        /// A source file, or a directory to walk.
        path: PathBuf,
    },
    /// Match client HTTP calls against backend routes.
    ///
    /// Reports each decision with its candidates, confidence and evidence.
    /// Ambiguous results produce no edge and say so.
    Match {
        /// A file, or a directory to walk.
        path: PathBuf,
    },
    /// Print version and build information.
    Version,
}

impl Command {
    /// The name used in `--json` documents and error envelopes.
    fn name(&self) -> &'static str {
        match self {
            Self::Trace { .. } => "trace",
            Self::Blast { .. } => "blast",
            Self::Diff { .. } => "diff",
            Self::Parse { .. } => "parse",
            Self::Normalize { .. } => "normalize",
            Self::Match { .. } => "match",
            Self::Version => "version",
        }
    }
}

fn main() -> std::process::ExitCode {
    let cli = Cli::parse();
    init_tracing(cli.verbose);

    let command_name = cli.command.as_ref().map_or("summary", Command::name);

    let outcome = dispatch(&cli);

    let code = match outcome {
        Ok((text, code)) => {
            let written = output::emit(&text);
            // A write failure outranks a successful analysis: the caller did
            // not receive the result, so reporting success would be false.
            if written == ExitCode::Success {
                code
            } else {
                written
            }
        }
        Err(error) => report_failure(&error, command_name, cli.json),
    };

    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    std::process::ExitCode::from(code.code() as u8)
}

/// Runs the requested command, returning what to print and how to exit.
fn dispatch(cli: &Cli) -> Result<(String, ExitCode), CliError> {
    match &cli.command {
        Some(Command::Trace {
            symbol,
            path,
            max_depth,
        }) => trace_cmd::run(path, symbol, *max_depth, cli.json),
        Some(Command::Blast { symbol, path }) => blast_cmd::run(path, symbol, cli.json),
        Some(Command::Diff {
            before,
            after,
            markdown,
        }) => {
            // `--json` and `--markdown` are mutually exclusive at the parser,
            // so the order of these arms cannot hide one behind the other.
            let form = if *markdown {
                diff_cmd::Form::Markdown
            } else if cli.json {
                diff_cmd::Form::Json
            } else {
                diff_cmd::Form::Text
            };
            diff_cmd::run(before, after, form)
        }
        Some(Command::Parse { path }) => parse_cmd::run(path, cli.json),
        Some(Command::Normalize { path }) => normalize_cmd::run(path, cli.json),
        Some(Command::Match { path }) => match_cmd::run(path, cli.json),
        Some(Command::Version) => version::run(cli.json).map(|text| (text, ExitCode::Success)),
        // Nothing to do and nothing named: show how to use the tool. That is
        // a usage error, not a silent success, so it exits 2.
        None => {
            let Some(path) = &cli.path else {
                use clap::CommandFactory;
                // Help on stderr, not stdout: this is the "you have not told
                // me what to do" path, and it exits non-zero. stdout is
                // reserved for a command's result, so a pipe gets nothing.
                output::emit_error(&Cli::command().render_help().to_string());
                std::process::exit(ExitCode::Usage.code());
            };
            summary_cmd::run(path, cli.json, cli.strict)
        }
    }
}

/// Renders a failure and returns the exit status it produces.
///
/// In JSON mode the error becomes a JSON document on stdout, so a consumer
/// parsing `--json` output always receives JSON. In text mode it goes to
/// stderr, leaving stdout empty.
fn report_failure(error: &CliError, command: &'static str, as_json: bool) -> ExitCode {
    if as_json {
        let envelope = json::ErrorEnvelope::new(command, error);
        match serde_json::to_string_pretty(&envelope) {
            Ok(mut text) => {
                text.push('\n');
                output::emit(&text);
            }
            // Serialising a fixed-shape error document cannot realistically
            // fail, but falling back to the human form beats printing nothing.
            Err(_) => output::emit_error(&error.to_string()),
        }
    } else {
        output::emit_error(&error.to_string());
    }
    error.exit()
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
    let filter = tracing_subscriber::EnvFilter::try_from_env("CARTOGRAPH_LOG")
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new(default));

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
    fn the_command_surface_is_the_accepted_surface() {
        // The command set is a deliberate milestone decision, and this test
        // exists so widening it cannot happen by accident. `trace`, `parse`,
        // `normalize`, `match`, `version` and the default summary are M09's;
        // `blast` is M12's and `diff` is M13's, each added when the milestone
        // that owns it was built. If this test fails, the scope moved --
        // decide whether it should have, then change this list on purpose.
        let command = Cli::command();
        let names: Vec<_> = command
            .get_subcommands()
            .map(clap::Command::get_name)
            .collect();
        assert_eq!(
            names,
            vec![
                "trace",
                "blast",
                "diff",
                "parse",
                "normalize",
                "match",
                "version"
            ]
        );
    }

    #[test]
    fn no_unbuilt_milestone_command_is_present() {
        // Named explicitly so an accidental addition fails loudly rather than
        // shipping a capability the project has not built. `diff` has left the
        // list: M13 built it. The rest are still unbuilt milestones.
        let command = Cli::command();
        let names: Vec<_> = command
            .get_subcommands()
            .map(clap::Command::get_name)
            .collect();
        for forbidden in ["watch", "serve", "ask", "mcp", "ui", "desktop"] {
            assert!(
                !names.contains(&forbidden),
                "`{forbidden}` is a later milestone's deliverable"
            );
        }
    }

    #[test]
    fn a_bare_path_is_the_default_summary() {
        let cli = Cli::try_parse_from(["cartograph", "."]).expect("parses");
        assert!(cli.command.is_none());
        assert_eq!(cli.path, Some(PathBuf::from(".")));
    }

    #[test]
    fn json_is_global_and_may_follow_the_subcommand() {
        let cli = Cli::try_parse_from(["cartograph", "match", ".", "--json"]).expect("parses");
        assert!(cli.json);
        assert!(matches!(cli.command, Some(Command::Match { .. })));
    }

    #[test]
    fn json_is_global_and_may_precede_the_path() {
        let cli = Cli::try_parse_from(["cartograph", "--json", "."]).expect("parses");
        assert!(cli.json);
        assert_eq!(cli.path, Some(PathBuf::from(".")));
    }

    #[test]
    fn trace_defaults_to_the_current_directory() {
        let cli = Cli::try_parse_from(["cartograph", "trace", "CheckoutButton"]).expect("parses");
        match cli.command {
            Some(Command::Trace {
                symbol,
                path,
                max_depth,
            }) => {
                assert_eq!(symbol, "CheckoutButton");
                assert_eq!(path, PathBuf::from("."));
                assert_eq!(max_depth, trace_cmd::DEFAULT_MAX_DEPTH);
            }
            other => panic!("expected trace, got {other:?}"),
        }
    }

    #[test]
    fn strict_is_off_unless_asked_for() {
        let cli = Cli::try_parse_from(["cartograph", "."]).expect("parses");
        assert!(!cli.strict);
        let strict = Cli::try_parse_from(["cartograph", ".", "--strict"]).expect("parses");
        assert!(strict.strict);
    }

    #[test]
    fn command_names_match_the_json_command_field() {
        let cli = Cli::try_parse_from(["cartograph", "trace", "X"]).expect("parses");
        assert_eq!(cli.command.as_ref().map(Command::name), Some("trace"));
    }
}
