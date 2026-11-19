//! Keeping the client wrapper on the path from a component to a handler.
//!
//! ```text
//! usePoolServicePostPool  ──Call──►  postPool  ──HttpCall──►  post_pool
//!   (queries.ts)                     (services.gen.ts)        (pools.py)
//! ```
//!
//! # Why the wrapper must stay
//!
//! A generated client wraps every request in a function. Attaching a caller
//! straight to the backend handler would assert a relationship that no line of
//! source states, and would lose the only place a reader could go to check it.
//! The wrapper is a node, and the call to it is an edge.
//!
//! # What refuses
//!
//! A callee whose name resolves to nothing, to a module outside the project,
//! or to a build-tool alias. A call whose enclosing function is unknown, since
//! an edge needs both ends. Nothing here matches on names alone.

use std::collections::HashMap;

use cartograph_core::{CommitId, EdgeKind, Evidence, NodeId, NodeKind, Provenance, SourceLocation};
use cartograph_graph::{ArchitectureGraph, EdgeSpec, GraphError};
use cartograph_parser::model::{Callee, FileAnalysis, SourceLanguage, Symbol, SymbolKind};

use crate::imports::{ModuleIndex, Resolution};

/// A function that issues an HTTP request, keyed by where it is declared.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ClientFunction {
    /// Repository-relative file.
    pub file: String,
    /// The function's name.
    pub name: String,
    /// Where the declaration starts.
    pub line: u32,
}

/// Every function in the project whose body issues an HTTP request.
///
/// These are the wrapper functions a component imports and calls.
#[must_use]
pub fn client_functions(files: &[FileAnalysis]) -> HashMap<(String, String), ClientFunction> {
    let mut out = HashMap::new();
    for file in files {
        if file.language == SourceLanguage::Python {
            continue; // a Python request is not a frontend client wrapper
        }
        for call in &file.http_calls {
            let Some(name) = &call.enclosing else {
                continue; // a request at module scope wraps nothing
            };
            let Some(symbol) = enclosing_declaration(file, name) else {
                continue;
            };
            out.insert(
                (file.path.clone(), name.clone()),
                ClientFunction {
                    file: file.path.clone(),
                    name: name.clone(),
                    line: symbol.span.start_line,
                },
            );
        }
    }
    out
}

/// The declaration of a named callable in one file.
///
/// A `const f = () => …` is recorded as a variable, and in TypeScript that is
/// how most functions are written. Excluding it found no client wrapper in
/// Airflow's generated query layer, where every hook is a const arrow
/// function.
fn enclosing_declaration<'f>(file: &'f FileAnalysis, name: &str) -> Option<&'f Symbol> {
    file.symbols.iter().find(|s| {
        s.name == name
            && matches!(
                s.kind,
                SymbolKind::Function | SymbolKind::Method | SymbolKind::Variable
            )
    })
}

/// How many call hops are followed from a component toward a request.
///
/// A component calls a hook, the hook calls a generated client, the client
/// issues the request. Three hops covers every layering in the corpus; beyond
/// that the path stops being something a reader could check.
const MAX_CALL_HOPS: usize = 4;

/// A callable, identified by where it is declared.
type FunctionId = (String, String);

/// One call this analysis resolved to a declaration, with both ends named.
struct ResolvedCall {
    from: FunctionId,
    to: FunctionId,
    at_file: String,
    at_line: u32,
}

/// Resolves every `TypeScript` call whose callee reaches a declaration.
///
/// Resolution is memoised per (file, name): a large frontend re-imports the
/// same handful of modules thousands of times.
fn resolve_calls(files: &[FileAnalysis], modules: &ModuleIndex<'_>) -> Vec<ResolvedCall> {
    let mut memo: HashMap<(String, String), Option<String>> = HashMap::new();
    let mut resolved = Vec::new();

    for file in files {
        if file.language == SourceLanguage::Python {
            continue;
        }
        for call in &file.calls {
            let Some(origin) = enclosing_symbol(file, call.span.start_line) else {
                continue; // an edge needs both ends
            };
            let (root, invoked) = match &call.callee {
                Callee::Identifier { name } => (name.clone(), name.clone()),
                Callee::Member { object, property } => {
                    let Some(first) = object.first() else {
                        continue;
                    };
                    (first.clone(), property.clone())
                }
            };
            let declared_in = memo
                .entry((file.path.clone(), root.clone()))
                .or_insert_with(|| match modules.resolve_typescript(file, &root) {
                    Resolution::Declared { file: at, .. } => Some(at),
                    Resolution::Local { .. } => Some(file.path.clone()),
                    // Unresolved, ambiguous or unbound: none of those names a
                    // declaration, and guessing which was meant is the defect
                    // this module exists to avoid.
                    _ => None,
                })
                .clone();
            let Some(declaring_file) = declared_in else {
                continue;
            };

            let from = (file.path.clone(), origin.name.clone());
            let to = (declaring_file, invoked);
            if from == to {
                continue; // recursion is not a relationship worth an edge
            }
            resolved.push(ResolvedCall {
                from,
                to,
                at_file: file.path.clone(),
                at_line: call.span.start_line,
            });
        }
    }
    resolved
}

/// Which callables reach a request, following calls backwards.
///
/// A call is only worth an edge if what it calls eventually issues one.
fn reaching_functions(
    calls: &[ResolvedCall],
    clients: &HashMap<FunctionId, ClientFunction>,
) -> std::collections::HashSet<FunctionId> {
    let mut reaching: std::collections::HashSet<FunctionId> = clients.keys().cloned().collect();
    for _ in 0..MAX_CALL_HOPS {
        let mut grew = false;
        for call in calls {
            if reaching.contains(&call.to) && !reaching.contains(&call.from) {
                reaching.insert(call.from.clone());
                grew = true;
            }
        }
        if !grew {
            break;
        }
    }
    reaching
}

/// Where each named callable is declared.
fn declaration_lines(files: &[FileAnalysis]) -> HashMap<FunctionId, u32> {
    let mut out = HashMap::new();
    for file in files {
        for symbol in &file.symbols {
            if matches!(
                symbol.kind,
                SymbolKind::Function | SymbolKind::Method | SymbolKind::Variable
            ) {
                out.entry((file.path.clone(), symbol.name.clone()))
                    .or_insert(symbol.span.start_line);
            }
        }
    }
    out
}

/// Adds `Call` edges along every path that reaches a function issuing a request.
///
/// Calls that lead nowhere near an HTTP request produce no edge: this is not a
/// general call graph, it is the path from a component to a wrapper.
///
/// Returns how many were added.
///
/// # Errors
///
/// Propagates [`GraphError`] if the graph rejects an endpoint, which cannot
/// happen for nodes created here.
pub fn add_client_call_edges(
    graph: &mut ArchitectureGraph,
    files: &[FileAnalysis],
    modules: &ModuleIndex<'_>,
    commit: Option<&CommitId>,
) -> Result<usize, GraphError> {
    let clients = client_functions(files);
    if clients.is_empty() {
        return Ok(0);
    }
    let calls = resolve_calls(files, modules);
    let reaching = reaching_functions(&calls, &clients);
    let declarations = declaration_lines(files);

    let mut seen: std::collections::HashSet<(FunctionId, FunctionId)> =
        std::collections::HashSet::new();
    let mut added = 0;

    for call in calls {
        if !reaching.contains(&call.to) || !seen.insert((call.from.clone(), call.to.clone())) {
            continue;
        }
        let (Some(&from_line), Some(&to_line)) =
            (declarations.get(&call.from), declarations.get(&call.to))
        else {
            continue;
        };

        let source = graph
            .node_for(
                NodeKind::Function,
                call.from.1.clone(),
                SourceLocation::new(call.from.0.clone(), from_line).ok(),
            )
            .map_err(|_| bad_node())?;
        let sink = graph
            .node_for(
                NodeKind::Function,
                call.to.1.clone(),
                SourceLocation::new(call.to.0.clone(), to_line).ok(),
            )
            .map_err(|_| bad_node())?;

        let directly = clients.contains_key(&call.to);
        graph.add_edge(EdgeSpec {
            source,
            target: sink,
            kind: EdgeKind::Call,
            confidence: Provenance::StaticImportResolution.prior_confidence(),
            provenance: Provenance::StaticImportResolution,
            evidence: Evidence::new(format!(
                "{} calls {} declared at {}:{}, which {}",
                call.from.1,
                call.to.1,
                call.to.0,
                to_line,
                if directly {
                    "issues an HTTP request"
                } else {
                    "reaches one through a call it makes"
                }
            ))
            .map_err(|_| bad_node())?,
            location: SourceLocation::new(call.at_file, call.at_line).map_err(|_| bad_node())?,
            commit: commit.cloned(),
        })?;
        added += 1;
    }
    Ok(added)
}

/// The innermost callable containing a line.
///
/// Variables are included for the same reason as in
/// [`enclosing_declaration`]: the binding of an arrow function is the function
/// that makes the call.
fn enclosing_symbol(file: &FileAnalysis, line: u32) -> Option<&Symbol> {
    file.symbols
        .iter()
        .filter(|s| {
            matches!(
                s.kind,
                SymbolKind::Function | SymbolKind::Method | SymbolKind::Variable
            )
        })
        .filter(|s| s.span.start_line <= line && s.span.end_line >= line)
        .min_by_key(|s| s.span.end_line - s.span.start_line)
}

fn bad_node() -> GraphError {
    GraphError::UnknownNode {
        id: NodeId::from_raw(u64::MAX),
    }
}
