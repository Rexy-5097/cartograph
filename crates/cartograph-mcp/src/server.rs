//! The MCP surface: four tools over one authorized repository.
//!
//! # Why the trait is implemented by hand
//!
//! `rmcp` offers a macro router. This implements [`ServerHandler`] directly
//! instead, because the tool list *is* the security surface: there is no
//! `authorize`, no `set_repository`, no `switch_repository`, and a reader
//! should be able to confirm that by reading one function rather than by
//! trusting what a macro expanded to.
//!
//! # Every tool goes through the same guard
//!
//! A tool takes a `tree` argument, and that argument is a *claim*, not an
//! authorization. It is handed to [`crate::Session`], which passes it to
//! `cartograph_pipeline::authorization` — below the transport — and a tree the
//! launcher did not grant is refused before the analyser opens anything.

use std::borrow::Cow;
use std::path::PathBuf;
use std::sync::Arc;

use rmcp::ErrorData as McpError;
use rmcp::ServerHandler;
use rmcp::model::{
    CallToolRequestParams, CallToolResponse, CallToolResult, ContentBlock, Implementation,
    JsonObject, ListToolsResult, PaginatedRequestParams, ServerCapabilities, ServerInfo, Tool,
};
use rmcp::service::{RequestContext, RoleServer};

use crate::{QueryError, Session};

/// The MCP server. Holds the session and nothing else.
#[derive(Debug)]
pub struct CartographServer {
    session: Arc<Session>,
}

impl CartographServer {
    /// Wraps an already-authorized session.
    ///
    /// There is no other constructor, and no method that mutates the session:
    /// the authorization this server serves was decided before it existed.
    #[must_use]
    pub fn new(session: Session) -> Self {
        Self {
            session: Arc::new(session),
        }
    }
}

/// Builds a JSON Schema object for a tool's parameters.
fn schema(properties: &serde_json::Value, required: &[&str]) -> Arc<JsonObject> {
    let value = serde_json::json!({
        "type": "object",
        "properties": properties,
        "required": required,
    });
    Arc::new(
        value
            .as_object()
            .cloned()
            .unwrap_or_else(serde_json::Map::new),
    )
}

fn tool(name: &'static str, description: &'static str, input_schema: Arc<JsonObject>) -> Tool {
    // `Tool` is `#[non_exhaustive]`; its constructor is the supported way to
    // build one, and it keeps compiling when the SDK adds a field.
    Tool::new(
        Cow::Borrowed(name),
        Cow::Borrowed(description),
        input_schema,
    )
}

/// The `tree` argument every tool takes.
const TREE_DESCRIPTION: &str = "A repository tree this session was granted at launch. A tree the session \
     was not granted is refused before any analysis runs.";

fn string_arg(description: &str) -> serde_json::Value {
    serde_json::json!({ "type": "string", "description": description })
}

/// Reads a required string argument.
fn required_str(request: &CallToolRequestParams, name: &str) -> Result<String, QueryError> {
    request
        .arguments
        .as_ref()
        .and_then(|args| args.get(name))
        .and_then(serde_json::Value::as_str)
        .map(ToOwned::to_owned)
        .ok_or_else(|| QueryError::Invalid(format!("`{name}` is required and must be a string")))
}

/// Reads an optional positive integer argument.
fn optional_usize(request: &CallToolRequestParams, name: &str) -> Option<usize> {
    request
        .arguments
        .as_ref()
        .and_then(|args| args.get(name))
        .and_then(serde_json::Value::as_u64)
        .and_then(|value| usize::try_from(value).ok())
}

/// Serialises an answer, or reports that it could not be.
fn json_result<T: serde::Serialize>(value: &T) -> Result<CallToolResult, McpError> {
    match serde_json::to_string_pretty(value) {
        Ok(text) => Ok(CallToolResult::success(vec![ContentBlock::text(text)])),
        // Nothing here can name the repository: the message is about the
        // serialiser, not about what was asked for.
        Err(error) => Err(McpError::internal_error(
            format!("the answer could not be serialised: {error}"),
            None,
        )),
    }
}

/// Turns a query failure into a tool error.
///
/// A refusal is an ordinary tool error rather than a protocol error: the
/// request was well formed and the server declined it. The message carries no
/// path, no parent directory and no identity — [`crate::QueryError::message`]
/// is built from fixed strings and already-redacted pipeline messages.
fn failed(error: &QueryError) -> CallToolResult {
    CallToolResult::error(vec![ContentBlock::text(error.message())])
}

impl ServerHandler for CartographServer {
    fn get_info(&self) -> ServerInfo {
        // Built through the SDK's constructors: `InitializeResult` and
        // `Implementation` are `#[non_exhaustive]`, so a struct literal would
        // stop compiling the first time the protocol gains a field.
        ServerInfo::new(ServerCapabilities::builder().enable_tools().build())
            .with_server_info(Implementation::new("cartograph", env!("CARGO_PKG_VERSION")))
            .with_instructions(
                "Cartograph answers questions about the architecture of ONE repository, \
                 authorized when this server was launched. Every relationship it reports was \
                 derived by static analysis and carries evidence, provenance and a confidence \
                 that is an uncalibrated prior rather than a probability. A tree this session \
                 was not granted is refused.",
            )
    }

    /// The whole tool surface, in one place.
    ///
    /// Note what is absent: nothing here authorizes, sets, switches or adds a
    /// repository. The session's authorization arrived in argv before this
    /// server existed and cannot be changed by anything a client sends.
    async fn list_tools(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListToolsResult, McpError> {
        Ok(ListToolsResult::with_all_items(vec![
            tool(
                "map",
                "What is this system? A compact, deterministic overview of the authorized \
                     repository: its languages, the kinds of artefact it contains, how they \
                     relate, its parts, where languages meet, and its best-connected artefacts.",
                schema(
                    &serde_json::json!({ "tree": string_arg(TREE_DESCRIPTION) }),
                    &["tree"],
                ),
            ),
            tool(
                "trace",
                "Follow one symbol's relationships outward, hop by hop, reporting the \
                     evidence for each. Qualify an ambiguous name as `file:name`.",
                schema(
                    &serde_json::json!({
                        "tree": string_arg(TREE_DESCRIPTION),
                        "symbol": string_arg("The symbol to trace, optionally `file:name`."),
                        "max_depth": {
                            "type": "integer",
                            "minimum": 0,
                            "description": "How deep to walk. Defaults to 10.",
                        },
                    }),
                    &["tree", "symbol"],
                ),
            ),
            tool(
                "blast",
                "What breaks if this changes? Reverse reachability from one symbol, with \
                     each dependent's weakest-link confidence and distance.",
                schema(
                    &serde_json::json!({
                        "tree": string_arg(TREE_DESCRIPTION),
                        "symbol": string_arg("The symbol that would change."),
                    }),
                    &["tree", "symbol"],
                ),
            ),
            tool(
                "diff",
                "What did a change do to the architecture? Compares two trees of the SAME \
                     authorized repository. Both must have been granted at launch; a pair \
                     naming an ungranted tree is refused before either is analysed.",
                schema(
                    &serde_json::json!({
                        "before": string_arg(TREE_DESCRIPTION),
                        "after": string_arg(TREE_DESCRIPTION),
                    }),
                    &["before", "after"],
                ),
            ),
        ]))
    }

    async fn call_tool(
        &self,
        request: CallToolRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> Result<CallToolResponse, McpError> {
        let session = Arc::clone(&self.session);
        let name = request.name.to_string();

        // The analyser is synchronous and can run for minutes on a large
        // repository, so it goes to a blocking thread rather than stalling the
        // runtime that is also reading the protocol stream.
        let answer = tokio::task::spawn_blocking(move || dispatch(&session, &name, &request))
            .await
            .map_err(|error| {
                McpError::internal_error(format!("the analysis task failed: {error}"), None)
            })?;

        match answer {
            Ok(result) => Ok(result.into()),
            Err(error) => Ok(failed(&error).into()),
        }
    }
}

/// Routes one tool call. Separated from the trait so it can be tested without
/// a transport or a runtime.
///
/// # Errors
///
/// Whatever the query could not do, including a refusal.
pub fn dispatch(
    session: &Session,
    name: &str,
    request: &CallToolRequestParams,
) -> Result<CallToolResult, QueryError> {
    match name {
        "map" => {
            let tree = PathBuf::from(required_str(request, "tree")?);
            json_result(&session.map(&tree)?)
                .map_err(|error| QueryError::Invalid(error.to_string()))
        }
        "trace" => {
            let tree = PathBuf::from(required_str(request, "tree")?);
            let symbol = required_str(request, "symbol")?;
            let max_depth = optional_usize(request, "max_depth")
                .unwrap_or(cartograph_graph::trace::DEFAULT_MAX_DEPTH);
            json_result(&session.trace(&tree, &symbol, max_depth)?)
                .map_err(|error| QueryError::Invalid(error.to_string()))
        }
        "blast" => {
            let tree = PathBuf::from(required_str(request, "tree")?);
            let symbol = required_str(request, "symbol")?;
            json_result(&session.blast(&tree, &symbol)?)
                .map_err(|error| QueryError::Invalid(error.to_string()))
        }
        "diff" => {
            let before = PathBuf::from(required_str(request, "before")?);
            let after = PathBuf::from(required_str(request, "after")?);
            json_result(&session.diff(&before, &after)?)
                .map_err(|error| QueryError::Invalid(error.to_string()))
        }
        // Named without echoing anything the caller supplied beyond the tool
        // name itself, which the protocol already carries.
        other => Err(QueryError::Invalid(format!(
            "`{other}` is not a tool this server offers; it offers map, trace, blast and diff"
        ))),
    }
}
