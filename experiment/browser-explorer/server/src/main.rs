//! `cartograph-serve`: Cartograph's analysis in a browser tab.
//!
//! **Experimental (M18; proposed ADR-0024, not accepted).** One repository,
//! chosen on the command line, is analysed by the same code the desktop uses
//! (`cartograph_desktop::session::analyze_report_as` →
//! `cartograph_pipeline::pipeline::run`). The result is served to a browser on
//! the loopback interface only.
//!
//! # What this server is allowed to do
//!
//! - **Loopback only.** It binds `127.0.0.1`; nothing on another machine can
//!   connect.
//! - **One repository.** The path is fixed at start. No request names a path
//!   to analyse, so the API cannot be pointed at another folder.
//! - **A per-launch token.** Every `/api` request must carry the token printed
//!   in the URL fragment (`#token=…`). A fragment is never sent to a server and
//!   never appears in a `Referer`, so the token reaches only the page.
//! - **No cross-origin use.** The token travels in a custom header, which a
//!   cross-origin page cannot send without a CORS preflight this server never
//!   approves. A `Host` header other than this listener's is refused, which
//!   defeats DNS rebinding, and an `Origin` other than this listener's is
//!   refused.
//! - **Only cited files.** Source previews go through
//!   `cartograph_desktop::source::excerpt`, which refuses any file the
//!   analysis does not cite.
//! - **No absolute paths out.** Payloads carry the repository's display name
//!   and repository-relative paths only.
//! - **No network egress.** ASK is not exposed; nothing here calls out.
//!
//! It adds no analysis: every graph, decision, trace, blast radius and piece
//! of evidence comes from `cartograph-desktop`, which takes it from the core.

use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use cartograph_core::{EdgeId, NodeId};
use cartograph_desktop::error::{DesktopError, DesktopErrorKind};
use cartograph_desktop::evidence::{AnalysisId, AnalysisSession};
use cartograph_desktop::repository::{ValidatedRepository, validate};
use cartograph_desktop::{blast, inspect, session, source, trace};

const MAX_HEAD: usize = 16 * 1024;
const MAX_BODY: usize = 1024;

fn main() {
    let args = match Args::parse(std::env::args().skip(1)) {
        Ok(args) => args,
        Err(message) => {
            eprintln!("cartograph-serve: {message}\n\n{USAGE}");
            std::process::exit(2);
        }
    };
    let repository = match validate(&args.repository) {
        Ok(repository) => repository,
        Err(error) => {
            eprintln!("cartograph-serve: {}", error.message);
            std::process::exit(1);
        }
    };
    let listener = match TcpListener::bind(("127.0.0.1", args.port)) {
        Ok(listener) => listener,
        Err(error) => {
            eprintln!(
                "cartograph-serve: cannot listen on 127.0.0.1:{}: {error}",
                args.port
            );
            std::process::exit(1);
        }
    };
    let port = listener.local_addr().map_or(args.port, |a| a.port());
    let shared = Arc::new(Shared {
        name: repository.display_name().to_owned(),
        repository,
        token: token(),
        port,
        assets: args.assets,
        state: Mutex::new(State::default()),
    });

    start_analysis(&shared);

    let url = format!("http://127.0.0.1:{port}/#token={}", shared.token);
    println!(
        "Cartograph browser explorer (experimental) for {}",
        shared.name
    );
    println!("Open: {url}");
    println!("Listening on 127.0.0.1:{port} only. Press Ctrl+C to stop.");
    if args.open {
        open_browser(&url);
    }

    for stream in listener.incoming() {
        let Ok(stream) = stream else { continue };
        let shared = Arc::clone(&shared);
        std::thread::spawn(move || {
            let _ = handle(stream, &shared);
        });
    }
}

const USAGE: &str = "usage: cartograph-serve [--open] [--port N] [--assets DIR] [REPOSITORY]

  REPOSITORY  the folder to analyse (default: the current directory)
  --open      open the explorer in the default browser
  --port N    listen on 127.0.0.1:N (default: a free port)
  --assets    the built web interface (default: desktop/dist-browser)";

struct Args {
    repository: PathBuf,
    open: bool,
    port: u16,
    assets: PathBuf,
}

impl Args {
    fn parse(mut raw: impl Iterator<Item = String>) -> Result<Self, String> {
        let mut args = Self {
            repository: PathBuf::from("."),
            open: false,
            port: 0,
            assets: PathBuf::from(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../../desktop/dist-browser"
            )),
        };
        while let Some(arg) = raw.next() {
            match arg.as_str() {
                "--open" => args.open = true,
                "--port" => {
                    let value = raw.next().ok_or("--port needs a number")?;
                    args.port = value.parse().map_err(|_| "--port needs a number")?;
                }
                "--assets" => {
                    args.assets = PathBuf::from(raw.next().ok_or("--assets needs a folder")?);
                }
                "-h" | "--help" => return Err("help".into()),
                other if other.starts_with('-') => return Err(format!("unknown option {other}")),
                other => args.repository = PathBuf::from(other),
            }
        }
        Ok(args)
    }
}

/// Everything a request handler may read.
struct Shared {
    repository: ValidatedRepository,
    name: String,
    token: String,
    port: u16,
    assets: PathBuf,
    state: Mutex<State>,
}

/// The analysis on screen, and the one being computed.
#[derive(Default)]
struct State {
    /// The last id handed out. Ids are issued under the lock, before the
    /// analysis runs, so two analyses can never share one.
    issued: Option<AnalysisId>,
    /// The analysis currently running, if any.
    running: Option<AnalysisId>,
    /// The newest completed analysis.
    current: Option<Arc<Current>>,
    /// Why the newest analysis failed, when it did. A failure never replaces a
    /// good graph.
    failure: Option<(AnalysisId, DesktopError)>,
}

/// A completed analysis, serialised once.
struct Current {
    id: AnalysisId,
    payload: String,
    report: String,
    session: AnalysisSession,
}

/// Starts an analysis unless one is already running; returns its id.
fn start_analysis(shared: &Arc<Shared>) -> AnalysisId {
    let id = {
        let Ok(mut state) = shared.state.lock() else {
            return AnalysisId::first();
        };
        if let Some(running) = state.running {
            return running;
        }
        let id = state.issued.map_or(AnalysisId::first(), AnalysisId::next);
        state.issued = Some(id);
        state.running = Some(id);
        id
    };
    let worker = Arc::clone(shared);
    std::thread::spawn(move || {
        let result = session::analyze_report_as(&worker.repository, id);
        let Ok(mut state) = worker.state.lock() else {
            return;
        };
        state.running = None;
        match result {
            Ok((payload, report, session)) => {
                let payload = serde_json::to_string(&payload).unwrap_or_default();
                let report = serde_json::to_string(&report).unwrap_or_default();
                state.current = Some(Arc::new(Current {
                    id,
                    payload,
                    report,
                    session,
                }));
                state.failure = None;
            }
            Err(error) => state.failure = Some((id, error)),
        }
    });
    id
}

struct Request {
    method: String,
    path: String,
    query: HashMap<String, String>,
    headers: HashMap<String, String>,
}

struct Response {
    status: u16,
    content_type: &'static str,
    body: Vec<u8>,
}

impl Response {
    fn json(status: u16, body: impl Into<Vec<u8>>) -> Self {
        Self {
            status,
            content_type: "application/json; charset=utf-8",
            body: body.into(),
        }
    }

    fn error(status: u16, kind: &str, message: &str) -> Self {
        let body = serde_json::json!({ "kind": kind, "message": message });
        Self::json(status, body.to_string())
    }

    fn desktop(error: &DesktopError) -> Self {
        let status = match error.kind {
            DesktopErrorKind::StaleSelection | DesktopErrorKind::NoAnalysis => 409,
            DesktopErrorKind::UnknownEdge
            | DesktopErrorKind::UnknownNode
            | DesktopErrorKind::NotFound => 404,
            _ => 400,
        };
        Self::json(status, serde_json::to_string(error).unwrap_or_default())
    }
}

fn handle(mut stream: TcpStream, shared: &Arc<Shared>) -> std::io::Result<()> {
    stream.set_read_timeout(Some(Duration::from_secs(10)))?;
    let response = match read_request(&mut stream) {
        Ok(request) => route(&request, shared),
        Err(message) => Response::error(400, "bad-request", message),
    };
    write_response(&mut stream, &response)
}

fn read_request(stream: &mut TcpStream) -> Result<Request, &'static str> {
    let mut buffer = Vec::with_capacity(1024);
    let mut chunk = [0_u8; 1024];
    let head_end = loop {
        let read = stream.read(&mut chunk).map_err(|_| "unreadable request")?;
        if read == 0 {
            return Err("incomplete request");
        }
        buffer.extend_from_slice(&chunk[..read]);
        if let Some(at) = find(&buffer, b"\r\n\r\n") {
            break at;
        }
        if buffer.len() > MAX_HEAD {
            return Err("request head too large");
        }
    };
    let head = std::str::from_utf8(&buffer[..head_end]).map_err(|_| "request head is not UTF-8")?;
    let mut lines = head.split("\r\n");
    let request_line = lines.next().ok_or("empty request")?;
    let mut parts = request_line.split(' ');
    let method = parts.next().ok_or("no method")?.to_owned();
    let target = parts.next().ok_or("no target")?;
    let mut headers = HashMap::new();
    for line in lines {
        if let Some((name, value)) = line.split_once(':') {
            headers.insert(name.trim().to_ascii_lowercase(), value.trim().to_owned());
        }
    }
    // A body is never used; it is drained (up to a small limit) so the client
    // sees a complete exchange.
    let length: usize = headers
        .get("content-length")
        .and_then(|v| v.parse().ok())
        .unwrap_or(0);
    if length > MAX_BODY {
        return Err("request body too large");
    }
    let (path, query) = match target.split_once('?') {
        Some((path, query)) => (path, parse_query(query)),
        None => (target, HashMap::new()),
    };
    Ok(Request {
        method,
        path: percent_decode(path).ok_or("malformed path")?,
        query,
        headers,
    })
}

fn route(request: &Request, shared: &Arc<Shared>) -> Response {
    // DNS rebinding: a page on another name that resolves to 127.0.0.1 sends
    // its own Host. Only this listener's own names are served.
    let host = request.headers.get("host").map_or("", String::as_str);
    let own = [
        format!("127.0.0.1:{}", shared.port),
        format!("localhost:{}", shared.port),
    ];
    if !own.iter().any(|h| h == host) {
        return Response::error(
            421,
            "wrong-host",
            "This server answers only on its own address.",
        );
    }

    if let Some(api) = request.path.strip_prefix("/api/") {
        if let Some(origin) = request.headers.get("origin") {
            let allowed = [
                format!("http://127.0.0.1:{}", shared.port),
                format!("http://localhost:{}", shared.port),
            ];
            if !allowed.iter().any(|o| o == origin) {
                return Response::error(403, "cross-origin", "Cross-origin requests are refused.");
            }
        }
        let presented = request
            .headers
            .get("x-cartograph-session")
            .map_or("", String::as_str);
        if !same(presented.as_bytes(), shared.token.as_bytes()) {
            return Response::error(
                401,
                "no-session",
                "This request carries no valid session token.",
            );
        }
        return api_route(&request.method, api, &request.query, shared);
    }

    if request.method != "GET" {
        return Response::error(405, "method", "Only GET is served here.");
    }
    static_file(&shared.assets, &request.path)
}

fn api_route(
    method: &str,
    api: &str,
    query: &HashMap<String, String>,
    shared: &Arc<Shared>,
) -> Response {
    match (method, api) {
        ("GET", "status") => status(shared),
        ("POST", "analyze") => {
            let id = start_analysis(shared);
            Response::json(202, serde_json::json!({ "started": id }).to_string())
        }
        ("GET", "analysis") => with_current(shared, |c| Response::json(200, c.payload.clone())),
        ("GET", "report") => with_current(shared, |c| Response::json(200, c.report.clone())),
        ("GET", "evidence") => with_current(shared, |c| {
            match (analysis_param(query), number(query, "edge")) {
                (Some(analysis), Some(edge)) => {
                    match c.session.evidence(analysis, EdgeId::from_raw(edge)) {
                        Ok(record) => Response::json(200, to_json(&record)),
                        Err(error) => Response::desktop(&error),
                    }
                }
                _ => Response::error(400, "bad-request", "analysis and edge are required."),
            }
        }),
        ("GET", "blast") => with_current(shared, |c| {
            match (analysis_param(query), number(query, "node")) {
                (Some(analysis), Some(node)) => {
                    match blast::radius(&c.session, analysis, NodeId::from_raw(node)) {
                        Ok(found) => Response::json(200, to_json(&found)),
                        Err(error) => Response::desktop(&error),
                    }
                }
                _ => Response::error(400, "bad-request", "analysis and node are required."),
            }
        }),
        ("GET", "node") => with_current(shared, |c| {
            match (analysis_param(query), number(query, "node")) {
                (Some(analysis), Some(node)) => {
                    match inspect::node(&c.session, analysis, NodeId::from_raw(node)) {
                        Ok(found) => Response::json(200, to_json(&found)),
                        Err(error) => Response::desktop(&error),
                    }
                }
                _ => Response::error(400, "bad-request", "analysis and node are required."),
            }
        }),
        ("GET", "trace") => with_current(shared, |c| {
            match (analysis_param(query), number(query, "node")) {
                (Some(analysis), Some(node)) => {
                    match trace::walk(&c.session, analysis, NodeId::from_raw(node)) {
                        Ok(found) => Response::json(200, to_json(&found)),
                        Err(error) => Response::desktop(&error),
                    }
                }
                _ => Response::error(400, "bad-request", "analysis and node are required."),
            }
        }),
        ("GET", "source") => with_current(shared, |c| {
            let (Some(analysis), Some(file)) = (analysis_param(query), query.get("file")) else {
                return Response::error(400, "bad-request", "analysis and file are required.");
            };
            let line = number(query, "line").and_then(|l| u32::try_from(l).ok());
            match source::excerpt(&shared.repository, &c.session, analysis, file, line) {
                Ok(found) => Response::json(200, to_json(&found)),
                Err(error) => Response::desktop(&error),
            }
        }),
        (
            _,
            "status" | "analyze" | "analysis" | "report" | "evidence" | "blast" | "node" | "trace"
            | "source",
        ) => Response::error(405, "method", "That method is not allowed here."),
        _ => Response::error(404, "not-found", "No such endpoint."),
    }
}

fn status(shared: &Arc<Shared>) -> Response {
    let Ok(state) = shared.state.lock() else {
        return Response::error(500, "internal", "The server state is unavailable.");
    };
    let phase = match (&state.running, &state.current, &state.failure) {
        (Some(_), _, _) => "running",
        (None, _, Some((failed, _))) if Some(*failed) == state.issued => "failed",
        (None, Some(_), _) => "ready",
        _ => "idle",
    };
    let body = serde_json::json!({
        "repository": shared.name,
        "phase": phase,
        "analysis": state.current.as_ref().map(|c| c.id),
        "running": state.running,
        "error": state.failure.as_ref().map(|(_, e)| e),
        "experimental": true,
    });
    Response::json(200, body.to_string())
}

/// Runs `f` against the newest completed analysis, without holding the lock
/// while it runs.
fn with_current(shared: &Arc<Shared>, f: impl FnOnce(&Current) -> Response) -> Response {
    let current = match shared.state.lock() {
        Ok(state) => state.current.clone(),
        Err(_) => return Response::error(500, "internal", "The server state is unavailable."),
    };
    match current {
        Some(current) => f(&current),
        None => Response::desktop(&DesktopError::new(
            DesktopErrorKind::NoAnalysis,
            "No analysis has finished yet.",
        )),
    }
}

fn analysis_param(query: &HashMap<String, String>) -> Option<AnalysisId> {
    serde_json::from_str(&number(query, "analysis")?.to_string()).ok()
}

fn number(query: &HashMap<String, String>, name: &str) -> Option<u64> {
    query.get(name)?.parse().ok()
}

fn to_json(value: &impl serde::Serialize) -> String {
    serde_json::to_string(value).unwrap_or_default()
}

/// Serves a file from the built interface, and only from there.
fn static_file(assets: &Path, path: &str) -> Response {
    // The browser build's entry is `browser.html` (the desktop's `index.html`
    // is a different build); `/` serves whichever the folder holds.
    let relative = if path == "/" {
        if assets.join("index.html").is_file() {
            "index.html"
        } else {
            "browser.html"
        }
    } else {
        path.trim_start_matches('/')
    };
    if relative
        .split('/')
        .any(|part| part == ".." || part.is_empty())
        || relative.contains('\\')
    {
        return Response::error(404, "not-found", "No such file.");
    }
    let Ok(root) = assets.canonicalize() else {
        return Response::error(
            500,
            "no-assets",
            "The web interface has not been built: run `npm run build:browser` in desktop/.",
        );
    };
    let Ok(full) = root.join(relative).canonicalize() else {
        return Response::error(404, "not-found", "No such file.");
    };
    if !full.starts_with(&root) || !full.is_file() {
        return Response::error(404, "not-found", "No such file.");
    }
    let content_type = match full.extension().and_then(|e| e.to_str()) {
        Some("html") => "text/html; charset=utf-8",
        Some("js") => "text/javascript; charset=utf-8",
        Some("css") => "text/css; charset=utf-8",
        Some("svg") => "image/svg+xml",
        Some("png") => "image/png",
        Some("json") => "application/json",
        Some("woff2") => "font/woff2",
        _ => "application/octet-stream",
    };
    match std::fs::read(&full) {
        Ok(body) => Response {
            status: 200,
            content_type,
            body,
        },
        Err(_) => Response::error(404, "not-found", "No such file."),
    }
}

fn write_response(stream: &mut TcpStream, response: &Response) -> std::io::Result<()> {
    let reason = match response.status {
        200 => "OK",
        202 => "Accepted",
        400 => "Bad Request",
        401 => "Unauthorized",
        403 => "Forbidden",
        404 => "Not Found",
        405 => "Method Not Allowed",
        409 => "Conflict",
        421 => "Misdirected Request",
        _ => "Internal Server Error",
    };
    let head = format!(
        "HTTP/1.1 {} {reason}\r\n\
         Content-Type: {}\r\n\
         Content-Length: {}\r\n\
         Connection: close\r\n\
         Cache-Control: no-store\r\n\
         X-Content-Type-Options: nosniff\r\n\
         Referrer-Policy: no-referrer\r\n\
         Cross-Origin-Resource-Policy: same-origin\r\n\
         X-Frame-Options: DENY\r\n\
         Content-Security-Policy: default-src 'self'; script-src 'self'; style-src 'self' 'unsafe-inline'; img-src 'self' data:; connect-src 'self'; frame-ancestors 'none'; base-uri 'none'; form-action 'none'\r\n\
         \r\n",
        response.status,
        response.content_type,
        response.body.len(),
    );
    stream.write_all(head.as_bytes())?;
    stream.write_all(&response.body)?;
    stream.flush()
}

fn parse_query(query: &str) -> HashMap<String, String> {
    query
        .split('&')
        .filter_map(|pair| {
            let (key, value) = pair.split_once('=')?;
            Some((percent_decode(key)?, percent_decode(value)?))
        })
        .collect()
}

fn percent_decode(text: &str) -> Option<String> {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'%' => {
                let hex = text.get(i + 1..i + 3)?;
                out.push(u8::from_str_radix(hex, 16).ok()?);
                i += 3;
            }
            b'+' => {
                out.push(b' ');
                i += 1;
            }
            byte => {
                out.push(byte);
                i += 1;
            }
        }
    }
    String::from_utf8(out).ok()
}

fn find(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack.windows(needle.len()).position(|w| w == needle)
}

/// Compares in time independent of where the inputs first differ.
fn same(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    a.iter().zip(b).fold(0_u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

/// A 256-bit per-launch token.
///
/// From the standard library's `RandomState`, whose keys are seeded from the
/// operating system's random source; no crate is added for it. Mixed with the
/// clock and the process id so two launches differ even if a platform seeded
/// poorly.
fn token() -> String {
    use std::fmt::Write as _;
    use std::hash::{BuildHasher, Hasher};
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos());
    let mut out = String::with_capacity(64);
    for i in 0_u64..4 {
        let mut hasher = std::collections::hash_map::RandomState::new().build_hasher();
        hasher.write_u64(i);
        hasher.write_u128(nanos);
        hasher.write_u32(std::process::id());
        let _ = write!(out, "{:016x}", hasher.finish());
    }
    out
}

fn open_browser(url: &str) {
    let result = if cfg!(target_os = "windows") {
        std::process::Command::new("rundll32")
            .args(["url.dll,FileProtocolHandler", url])
            .spawn()
    } else if cfg!(target_os = "macos") {
        std::process::Command::new("open").arg(url).spawn()
    } else {
        std::process::Command::new("xdg-open").arg(url).spawn()
    };
    if result.is_err() {
        eprintln!("cartograph-serve: could not open a browser; open the URL above yourself.");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_query_is_percent_decoded() {
        let q = parse_query("file=web%2Fapi.ts&line=2&x");
        assert_eq!(q.get("file").map(String::as_str), Some("web/api.ts"));
        assert_eq!(q.get("line").map(String::as_str), Some("2"));
        assert!(!q.contains_key("x"));
        assert_eq!(percent_decode("%zz"), None);
        assert_eq!(percent_decode("%2"), None);
    }

    #[test]
    fn tokens_compare_only_when_identical() {
        assert!(same(b"abc", b"abc"));
        assert!(!same(b"abc", b"abd"));
        assert!(!same(b"abc", b"ab"));
    }

    #[test]
    fn two_tokens_differ_and_are_256_bits_of_hex() {
        let (a, b) = (token(), token());
        assert_eq!(a.len(), 64);
        assert!(a.chars().all(|c| c.is_ascii_hexdigit()));
        assert_ne!(a, b);
    }

    #[test]
    fn a_static_path_cannot_leave_the_assets_folder() {
        let dir =
            std::env::temp_dir().join(format!("cartograph-serve-assets-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("assets")).expect("dir");
        std::fs::write(dir.join("index.html"), "<p>ok</p>").expect("index");
        assert_eq!(static_file(&dir, "/").status, 200);
        for path in ["/../Cargo.toml", "/assets/../../x", "/a//b", "/..\\x"] {
            assert_eq!(static_file(&dir, path).status, 404, "{path}");
        }
        let _ = std::fs::remove_dir_all(&dir);
    }
}
