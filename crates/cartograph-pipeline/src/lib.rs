//! Repository analysis, composed once for every client.
//!
//! # Why this crate exists
//!
//! [ADR-0001](../../../docs/adr/ADR-0001-rust-core-is-the-product.md) says the
//! CLI, the desktop application and the MCP server are *peer clients* of one
//! core, and that none of them holds analysis logic. `cartograph-cli`'s own
//! documentation has said the same since M09.
//!
//! It was not quite true. The individual analysis steps live in
//! `cartograph-parser`, `cartograph-resolver` and `cartograph-graph`, but the
//! **sequence** that turns a directory into a graph — discovery, parsing,
//! caching, resolution, edge construction — lived inside the CLI *binary*, and
//! a binary crate cannot be depended on. The second client could therefore
//! either duplicate that sequence or reach into the first client, and both
//! answers are wrong: two clients composing the same steps differently is
//! exactly the drift ADR-0001 exists to prevent.
//!
//! So the sequence moved here, unchanged, when M11 needed a second client. The
//! CLI now depends on this crate rather than owning it.
//!
//! # What moved, and what did not
//!
//! Moved verbatim from `cartograph-cli`: [`discovery`], [`pipeline`],
//! [`incremental`] and [`error`]. No behaviour changed — the analyser, the
//! incremental caches, the exit codes and the error payloads are the ones M09
//! and M10 accepted, and their test suites moved with them.
//!
//! Not moved: everything that renders output for a terminal — the command
//! modules, the JSON document builder, the human-readable summaries. Those are
//! the CLI's own concern, and a desktop client wants none of them.
//!
//! # A note on the error type
//!
//! [`error::CliError`] keeps its name. It is not only the CLI's error: it is a
//! structured, serialisable failure with a stable [`error::ErrorCode`], and it
//! is what every client should report. Renaming it would have touched every
//! command module and the JSON conformance suite for no behavioural gain, so
//! the name is left as a wart rather than paid for during a move. See
//! ADR-0016.

pub mod authorization;
pub mod discovery;
pub mod error;
pub mod incremental;
pub mod pipeline;
