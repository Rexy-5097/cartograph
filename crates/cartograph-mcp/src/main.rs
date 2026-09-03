//! The MCP server binary: argv in, stdio protocol out.
//!
//! # Why the grant is in argv
//!
//! [ADR-0020](../../../docs/adr/ADR-0020-mcp-boundary-and-authorization.md)
//! Amendment 2 chose launch-time argv (option G1). The consequence is visible
//! here and nowhere else: by the time `serve` is called the authorization is
//! already complete and immutable, so no message a client sends can widen it.
//!
//! The alternative — a client-initiated handshake — was rejected because it
//! makes the guarded party its own authority. There is deliberately no
//! `--allow-any`, no configuration file and no environment variable: a second
//! grant channel would be a second thing to secure.
//!
//! # Identity is not the path
//!
//! `--repository` says *which directory to analyse*. `--repository-identity`
//! says *which repository this session is authorized for*. They are different
//! things, and conflating them is what ADR-0020 Amendment 1 rejected: two
//! checkouts of one repository are two paths, and a path names a location on
//! this machine rather than a repository.
//!
//! The identity value is opaque. This binary passes it through and never
//! interprets, canonicalises, hashes or prints it.
//!
//! # stdout belongs to the protocol
//!
//! Every diagnostic in this file goes to stderr. One stray `println!` would
//! corrupt the JSON-RPC stream, which is why the startup banner, the failure
//! messages and the shutdown notice all use `eprintln!`.

use std::path::PathBuf;
use std::process::ExitCode;

use cartograph_mcp::server::CartographServer;
use cartograph_mcp::{Grant, Session};
use clap::Parser;
use rmcp::ServiceExt;

/// Cartograph's MCP server, over stdio.
#[derive(Debug, Parser)]
#[command(
    name = "cartograph-mcp",
    about = "Answer architecture questions about one authorized repository, over MCP.",
    long_about = "Serves Cartograph's map, trace, blast and diff over the Model Context \
                  Protocol on stdio.\n\nThe repository this session may analyse is fixed by \
                  these arguments before any request is read. No MCP request can change it: \
                  there is no tool to set, switch or add a repository."
)]
struct Cli {
    /// A repository tree this session may analyse. Repeat for a diff's two trees.
    ///
    /// Only these paths can be queried. Anything else is refused before the
    /// analyser reads a byte.
    #[arg(long = "repository", value_name = "PATH", required = true)]
    repositories: Vec<PathBuf>,

    /// The opaque identity of the repository these trees belong to.
    ///
    /// Supplied by whoever launches this process, which is the party that knows
    /// two trees are one repository. Cartograph stores and compares it and
    /// never interprets it; its derivation is deliberately not decided here.
    #[arg(long = "repository-identity", value_name = "IDENTITY")]
    identity: String,
}

fn main() -> ExitCode {
    let cli = Cli::parse();

    // argv → grant → authorization, before any protocol byte is read.
    let grant = Grant::new(cli.identity, cli.repositories);
    let authorized = match grant.authorize() {
        Ok(authorized) => authorized,
        Err(error) => {
            // Says what was wrong with the grant, never what it contained.
            eprintln!("cartograph-mcp: {error}");
            return ExitCode::from(3);
        }
    };

    let session = Session::new(authorized);
    eprintln!(
        "cartograph-mcp: serving one authorized repository ({} tree(s)) over stdio",
        session.tree_count()
    );

    let runtime = match tokio::runtime::Runtime::new() {
        Ok(runtime) => runtime,
        Err(error) => {
            eprintln!("cartograph-mcp: could not start the async runtime: {error}");
            return ExitCode::from(4);
        }
    };

    runtime.block_on(async move {
        let service = match CartographServer::new(session)
            .serve(rmcp::transport::stdio())
            .await
        {
            Ok(service) => service,
            Err(error) => {
                eprintln!("cartograph-mcp: the MCP handshake failed: {error}");
                return ExitCode::from(4);
            }
        };

        match service.waiting().await {
            Ok(reason) => {
                eprintln!("cartograph-mcp: session closed ({reason:?})");
                ExitCode::SUCCESS
            }
            Err(error) => {
                eprintln!("cartograph-mcp: session ended in error: {error}");
                ExitCode::from(4)
            }
        }
    })
}
