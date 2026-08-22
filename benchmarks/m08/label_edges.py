#!/usr/bin/env python3
"""Label every produced edge from repository source, independently.

M07 established that a benchmark is only as trustworthy as the independence of
its labels. This module extends that discipline from *declarations* to *edges*:
for each relationship Cartograph asserts, it re-derives the underlying facts
from source and decides whether the assertion holds.

Nothing here imports the analyser, reads its intermediate state, or consults
its evidence strings for anything but the file/line coordinates it must go and
check. In particular the server-side path composition below is a **second
implementation** of the rule `cartograph-resolver` applies — written from the
frameworks' semantics rather than from that code — because a label produced by
the thing being measured is not a label.

Standard library only, matching the rest of `benchmarks/`.
"""

from __future__ import annotations

import json
import os
import re

# ── Independent source readers ───────────────────────────────────────

CLASS_DECL = re.compile(r"^\s*class\s+(\w+)\s*[\(:]")
TABLENAME = re.compile(r"""^\s*__tablename__\s*=\s*["']([^"']+)["']""")
DB_TABLE = re.compile(r"""^\s*db_table\s*=\s*["']([^"']+)["']""")
ROUTER_DECL = re.compile(r"""^\s*(\w+)\s*=\s*\w*Router\s*\((.*)\)""")
ROUTER_OPEN = re.compile(r"""^\s*(\w+)\s*=\s*\w*Router\s*\(""")
INCLUDE = re.compile(r"""^\s*([\w.]+)\.include_router\s*\(\s*([\w.]+)""")
PREFIX_KW = re.compile(r"""prefix\s*=\s*["']([^"']*)["']""")
DECORATOR = re.compile(r"^\s*@([\w.]+)\.(get|post|put|patch|delete|head|options)\s*\(", re.I)
# Client-side URL, both shapes.
POSITIONAL = re.compile(
    r"""[\w.$\])]+\.(get|post|put|patch|delete|head|options)\s*\(\s*[`'"]([^`'"]+)""", re.I
)
CONFIG_URL = re.compile(r"""\b(?:url|endpoint)\s*:\s*[`'"]([^`'"]+)""")
CONFIG_METHOD = re.compile(r"""\bmethod\s*:\s*['"]([A-Za-z]+)['"]""")
FETCH = re.compile(r"""\bfetch\s*\(\s*[`'"]([^`'"]+)""")


SKIP_DIRS = {
    "node_modules", ".git", "dist", "build", "coverage", "target",
    "__pycache__", "site-packages", "venv", ".venv", "migrations",
}


def python_files(root):
    """Every Python file the analyser would have seen.

    Router composition has to search the whole project: a router is declared
    in one module and mounted in another, and Airflow mounts its public router
    in a package `__init__.py` that declares no route at all. Passing only the
    files that contain route declarations makes composition fail and turns
    every correct edge into a false positive — which is exactly what the first
    run of this labeller reported.
    """
    out = []
    for dirpath, dirnames, filenames in os.walk(root):
        dirnames[:] = [d for d in dirnames if d not in SKIP_DIRS and not d.startswith(".")]
        for name in filenames:
            if name.endswith(".py") and not name.startswith("._"):
                out.append(os.path.relpath(os.path.join(dirpath, name), root).replace(os.sep, "/"))
    return sorted(out)


class Source:
    """Read-only access to a pinned checkout. Executes nothing."""

    def __init__(self, root):
        self.root = root
        self._cache = {}

    def lines(self, rel):
        if rel not in self._cache:
            try:
                with open(os.path.join(self.root, rel), encoding="utf-8", errors="replace") as h:
                    self._cache[rel] = h.readlines()
            except OSError:
                self._cache[rel] = []
        return self._cache[rel]

    def line(self, rel, n):
        ls = self.lines(rel)
        return ls[n - 1].rstrip("\n") if 0 < n <= len(ls) else None

    def window(self, rel, n, span=6):
        ls = self.lines(rel)
        return "".join(ls[max(0, n - 1) : n - 1 + span])

    # ── declarations ────────────────────────────────────────────────

    def declared_table(self, rel, class_name):
        """The table a class declares, read straight from source."""
        ls = self.lines(rel)
        start = None
        for i, line in enumerate(ls):
            m = CLASS_DECL.match(line)
            if m and m.group(1) == class_name and not line.startswith((" ", "\t")):
                start = i
                break
        if start is None:
            return None
        for line in ls[start + 1 : start + 400]:
            if CLASS_DECL.match(line) and not line.startswith((" ", "\t")):
                break
            m = TABLENAME.match(line) or DB_TABLE.match(line)
            if m:
                return m.group(1)
        return None

    def binds(self, rel, name):
        """How a name is bound in a file: 'here', a module it came from, or None."""
        text = "".join(self.lines(rel))
        for line in self.lines(rel):
            m = CLASS_DECL.match(line)
            if m and m.group(1) == name:
                return "here"
            if re.match(rf"^\s*(?:async\s+)?def\s+{re.escape(name)}\s*\(", line):
                return "here"
            if re.match(rf"^(?:export\s+)?(?:const|let|var|function|class)\s+{re.escape(name)}\b", line):
                return "here"
        for m in re.finditer(r"^\s*from\s+([\w.]+)\s+import\s+(\(.*?\)|[^\n]*)", text, re.M | re.S):
            if re.search(rf"(^|[\s(,]){re.escape(name)}\s*(,|\)|$|\s+as\s)", m.group(2), re.M):
                return m.group(1)
            if "*" in m.group(2):
                return "*"
        for m in re.finditer(r"""^\s*import\s+(?:\{[^}]*\}|[\w*\s,]+)\s+from\s+['"]([^'"]+)""", text, re.M):
            if re.search(rf"\b{re.escape(name)}\b", m.group(0).split(" from ")[0]):
                return m.group(1)
        return None


# ── Independent server-path composition ──────────────────────────────


class RouterIndex:
    """Router declarations and mountings, read once from source.

    Independent of `cartograph-resolver`: built from what FastAPI does with
    `APIRouter(prefix=…)` and `include_router`, not from how that crate models
    it. Two things make a naive version wrong, and Airflow exhibits both:

    * **Two different routers can share a name.** `authenticated_router` is
      declared in both `core_api/routes/public/__init__.py` and
      `execution_api/routes/__init__.py`. Matching mounts by bare name reports
      each as mounted in several places and refuses every route under them.
      A mount counts only when the name at the mounting site resolves — through
      that file's own imports — to this declaration.
    * **Not every `x.include_router(y)` composes a served path.** A test that
      builds a throwaway `app = FastAPI()` and mounts a router on it is not
      part of the deployed path. Only a parent that is itself a declared
      router chains.
    """

    def __init__(self, source, files):
        self.source = source
        self.decls = {}      # (file, name) -> (prefix or None, literal?)
        self.by_name = {}    # name -> [file]
        self.includes = []   # (file, parent, child, prefix_at_site)
        for rel in files:
            lines = source.lines(rel)
            for i, line in enumerate(lines):
                # A router constructor often spans several lines:
                #
                #     authenticated_router = AirflowRouter(
                #         dependencies=[Depends(get_user)],
                #     )
                #
                # Reading only the first line finds no declaration, and every
                # route beneath that router then resolves without its parent's
                # prefix. Airflow writes two of its three top-level routers
                # this way.
                if ROUTER_OPEN.match(line) and line.count("(") > line.count(")"):
                    joined, depth = "", 0
                    for nxt in lines[i : i + 12]:
                        joined += nxt.rstrip("\n")
                        depth += nxt.count("(") - nxt.count(")")
                        if depth <= 0:
                            break
                    line = joined
                m = ROUTER_DECL.match(line)
                if m:
                    name, args = m.group(1), m.group(2)
                    kw = PREFIX_KW.search(args)
                    literal = not ("prefix" in args and kw is None)
                    self.decls[(rel, name)] = (kw.group(1) if kw else "", literal)
                    self.by_name.setdefault(name, []).append(rel)
                    continue
                m = INCLUDE.match(line)
                if m:
                    at = PREFIX_KW.search(line)
                    self.includes.append(
                        (rel, m.group(1).split(".")[-1], m.group(2).split(".")[-1],
                         at.group(1) if at else "")
                    )

    def declaring_file(self, at_file, name):
        """Which declaration of `name` the file `at_file` refers to."""
        if (at_file, name) in self.decls:
            return at_file
        binding = self.source.binds(at_file, name)
        if binding in (None, "here", "*"):
            return None
        tail = binding.replace(".", "/")
        for cand in self.by_name.get(name, []):
            if cand.endswith(f"{tail}.py") or cand.endswith(f"{tail}/__init__.py"):
                return cand
        return None

    def prefix(self, rel, name, seen=None, depth=0):
        """The composed prefix of the router `name` as declared in `rel`."""
        seen = seen if seen is not None else set()
        if depth > 12 or (rel, name) in seen:
            return None, "router inclusion cycles or is too deep"
        seen.add((rel, name))

        decl = self.decls.get((rel, name))
        if decl is None:
            return None, f"`{name}` is not a router declared in {rel}"
        own, literal = decl
        if not literal:
            return None, f"`{name}` has a prefix that is not a literal"

        mounts = []
        for at_file, parent, child, at_prefix in self.includes:
            if child != name:
                continue
            if self.declaring_file(at_file, child) != rel:
                continue  # a different router that happens to share the name
            parent_file = self.declaring_file(at_file, parent)
            if parent_file is None:
                continue  # mounted on something that is not a declared router
            mounts.append((parent_file, parent, at_prefix))

        distinct = {(f, n, p) for f, n, p in mounts}
        if not distinct:
            return own, "root"
        if len(distinct) > 1:
            return None, f"`{name}` is mounted in {len(distinct)} places"
        pfile, pname, at = distinct.pop()
        outer, reason = self.prefix(pfile, pname, seen, depth + 1)
        if outer is None:
            return None, reason
        return _join(_join(outer, at), own), "composed"


def compose_served_path(source, rel, line, routers):
    """Re-derive the path a route decorator is actually served on.

    Returns `(path, verb)`, or `(None, reason)`.
    """
    text = source.window(rel, line, 8)
    m = DECORATOR.search(text)
    if not m:
        return None, "no decorator at the cited line"
    receiver, verb = m.group(1).split(".")[-1], m.group(2).upper()
    after = text[m.end():]
    lit = re.match(r"""\s*[`'"]([^`'"]*)""", after)
    if lit is None:
        return None, "route path is not a literal"
    own = lit.group(1)

    declaring = routers.declaring_file(rel, receiver)
    if declaring is None:
        return None, f"`{receiver}` is not a router this project declares"
    prefix, reason = routers.prefix(declaring, receiver)
    if prefix is None:
        return None, reason
    return _join(prefix, own), verb


def _join(a, b):
    left = a[:-1] if a.endswith("/") else a
    if not b or b == "/":
        return left + ("/" if b == "/" else "")
    return f"{left}{b}" if b.startswith("/") else f"{left}/{b}"


def request_path(url):
    """The path part of a client URL.

    A query string is not part of the route, and M03 canonicalises it away; an
    absolute URL carries a scheme and host that no route declares. Keeping
    either made this reader disagree with the matcher about paths that agree.
    """
    url = url.split("#", 1)[0].split("?", 1)[0]
    m = re.match(r"^[a-zA-Z][\w+.-]*://[^/]*(/.*)?$", url)
    if m:
        url = m.group(1) or "/"
    return url


def segments(path):
    return [s for s in request_path(path).strip("/").split("/") if s]


def is_parameter(segment):
    """A position whose value the source does not fix."""
    return (
        segment.startswith("{")
        or segment.startswith("<")
        or segment.startswith(":")
        or "${" in segment
        or "{" in segment
    )


def canonical(path):
    """Readable form for reporting; parameter positions become `{*}`."""
    out = ["{*}" if is_parameter(s) else s for s in segments(path)]
    return "/" + "/".join(out) + ("/" if path.endswith("/") and len(path) > 1 else "")


UNKNOWN_AGAINST_LITERAL = "UNKNOWN_AGAINST_LITERAL"


def paths_agree(client, served):
    """Does a client path reach a route?

    The rule the matcher implements, restated here from the frameworks'
    semantics: a route's **declared parameter accepts any client segment** —
    a concrete value, an interpolation, anything — while a **static** route
    segment must be equal. An earlier version compared canonical forms on both
    sides and called 256 correct edges false, because it demanded that a
    client passing `claude-haiku-4-5` into `/{model}` should somehow have
    written `{model}` itself.
    """
    c, s = segments(client), segments(served)
    if len(c) != len(s):
        return False
    unknown_against_literal = False
    for cs, ss in zip(c, s):
        if is_parameter(ss):
            continue          # the route accepts whatever the client sent
        if is_parameter(cs):
            # The client's value is not fixed by the source, so whether it
            # equals this literal cannot be decided here. M04 accepts such a
            # match as possible-but-unverified and lowers its confidence; this
            # reader reports it as unverifiable rather than pretending to know.
            unknown_against_literal = True
            continue
        if cs != ss:
            return False
    return UNKNOWN_AGAINST_LITERAL if unknown_against_literal else True


def call_arguments(source, rel, line):
    """The argument text of the call that begins at `line`.

    Anchored deliberately. An earlier version searched a twelve-line window
    and took the first URL-shaped match anywhere in it, which picked up the
    *next* request in the file: Onyx's SSO test issues
    `client.post(f"/admin/sso/provider/{provider_id}/enabled", ...)` and the
    window reached far enough to find a later `client.get("/admin/sso/provider")`,
    labelling 254 correct edges as false positives.
    """
    lines = source.lines(rel)
    if not (0 < line <= len(lines)):
        return None
    first = lines[line - 1]
    start = first.find("(")
    if start == -1:
        return None
    text, depth = "", 0
    for offset, raw in enumerate(lines[line - 1 : line - 1 + 20]):
        chunk = raw[start:] if offset == 0 else raw
        for ch in chunk:
            if ch == "(":
                depth += 1
                if depth == 1:
                    continue
            elif ch == ")":
                depth -= 1
                if depth == 0:
                    return text
            if depth >= 1:
                text += ch
        text += "\n"
    return text or None


# A string literal, allowing the f/r/b prefixes Python and JS use.
STRING_LITERAL = re.compile(r"""(?:[fFrRbBu]{0,2})[`'"]([^`'"]*)""")


def client_request(source, rel, line):
    """Re-derive the URL and method the call at `line` states."""
    args = call_arguments(source, rel, line)
    if args is None:
        return None, None

    method = None
    url = None

    # A leading string literal is the request path, and it wins. Reading the
    # configuration object first found `url: "https://example.com"` buried in
    # an OpenAPI document that Onyx passes as the *body* of a request whose
    # own path is the first argument.
    first = STRING_LITERAL.match(args.strip())
    if first and (first.group(1).startswith("/") or first.group(1).startswith("http")):
        url = first.group(1)
    if url is None:
        cu = CONFIG_URL.search(args)
        if cu:
            url = cu.group(1)
            cm = CONFIG_METHOD.search(args)
            method = cm.group(1).upper() if cm else None
    if url is None:
        return None, None
    if method is None:
        head = source.line(rel, line) or ""
        v = re.search(r"\.(get|post|put|patch|delete|head|options)\s*\(", head, re.I)
        if v:
            method = v.group(1).upper()
        elif re.search(r"\bfetch\s*\(", head):
            mm = re.search(r"""method\s*:\s*['"]([A-Za-z]+)['"]""", args)
            method = mm.group(1).upper() if mm else None
    return url, method


# ── Per-edge labelling ───────────────────────────────────────────────

ACCESS_EVIDENCE = re.compile(r"^(?P<expr>\S+)\(\.\.\.\) in (?P<handler>\S+) at (?P<file>.+):(?P<line>\d+)$")

TRUE_POSITIVE = "TRUE_POSITIVE"
FALSE_POSITIVE = "FALSE_POSITIVE"
UNVERIFIABLE = "UNVERIFIABLE"


def label_graph(graph, source, routers):
    """Labels every edge. Returns a list of records, one per edge.

    `UNVERIFIABLE` is a first-class outcome, never folded into either of the
    other two. An instrument that cannot check something must say so; counting
    its silence as agreement is how a benchmark flatters itself.
    """
    nodes = {n["id"]: n for n in graph["nodes"]}
    out = []
    for edge in graph["edges"]:
        target = nodes[edge["target"]]
        src = nodes[edge["source"]]
        rec = {
            "edge": edge["id"],
            "kind": edge["kind"],
            "confidence": round(edge["confidence"], 6),
            "provenance": edge["provenance"],
            "at": f"{edge['file']}:{edge['line']}",
        }
        if edge["kind"] == "queries":
            rec.update(_label_queries(edge, src, target, source))
        elif edge["kind"] == "orm-access":
            rec.update(_label_orm_access(edge, target, source))
        elif edge["kind"] == "call":
            rec.update(_label_call(edge, target, source))
        elif edge["kind"] == "http-call":
            rec.update(_label_http(edge, target, source, routers))
        else:
            rec.update({"label": UNVERIFIABLE, "why": f"no labeller for {edge['kind']}"})
        out.append(rec)
    return out


def _label_queries(edge, src, target, source):
    model, table, rel = src["name"], target["name"], src.get("file")
    if not rel:
        return {"label": UNVERIFIABLE, "why": "model node has no file"}
    declared = source.declared_table(rel, model)
    if declared is None:
        return {"label": UNVERIFIABLE, "why": "no literal table declared in this class"}
    if declared == table:
        return {"label": TRUE_POSITIVE, "why": f"{model} declares {table}"}
    return {"label": FALSE_POSITIVE, "why": f"source declares {declared!r}, edge says {table!r}"}


def _label_orm_access(edge, target, source):
    m = ACCESS_EVIDENCE.match(edge["evidence"])
    if not m:
        return {"label": UNVERIFIABLE, "why": "evidence not in the expected shape"}
    rel, line = m.group("file"), int(m.group("line"))
    root = m.group("expr").split(".")[0]
    text = source.line(rel, line)
    if text is None or root not in text:
        return {"label": FALSE_POSITIVE, "why": "the expression is not at the cited line"}
    if not rel.endswith(".py"):
        return {"label": FALSE_POSITIVE, "why": "a Python ORM model cannot be used from this file"}
    binding = source.binds(rel, root)
    declaring = target.get("file")
    if binding == "here":
        return ({"label": TRUE_POSITIVE, "why": "declared in this file"} if declaring == rel
                else {"label": FALSE_POSITIVE, "why": "a local declaration shadows the model"})
    if binding is None:
        return {"label": UNVERIFIABLE, "why": "the name is not bound by any import this reader sees"}
    if binding == "*":
        return {"label": UNVERIFIABLE, "why": "bound by a star import"}
    tail = binding.replace(".", "/")
    if declaring and (declaring.endswith(f"{tail}.py") or declaring.endswith(f"{tail}/__init__.py")):
        return {"label": TRUE_POSITIVE, "why": f"imported from {binding}, which declares it"}
    return {"label": UNVERIFIABLE, "why": f"imported from {binding}; re-export not followed by this reader"}


def _label_call(edge, target, source):
    rel, line = edge["file"], edge["line"]
    text = source.line(rel, line)
    if text is None:
        return {"label": UNVERIFIABLE, "why": "cited line not readable"}
    name = target["name"]
    window = source.window(rel, line, 3)
    if name not in window:
        return {"label": FALSE_POSITIVE, "why": f"{name} is not called at the cited line"}
    binding = source.binds(rel, name)
    if binding == "here":
        return {"label": TRUE_POSITIVE, "why": "callee declared in this file"}
    if binding is None:
        return {"label": UNVERIFIABLE, "why": "callee binding not visible to this reader"}
    return {"label": TRUE_POSITIVE, "why": f"callee imported from {binding}"}


def _label_http(edge, target, source, routers):
    url, method = client_request(source, edge["file"], edge["line"])
    if url is None:
        return {"label": UNVERIFIABLE, "why": "no literal URL at the client site"}
    trel, tline = target.get("file"), target.get("line")
    if not trel or not tline:
        return {"label": UNVERIFIABLE, "why": "target has no source location"}
    if not trel.endswith(".py"):
        return {"label": FALSE_POSITIVE, "why": "target is not Python"}

    served, served_method = compose_served_path(source, trel, tline, routers)
    if served is None:
        # `served_method` carries the refusal reason in this branch.
        return {"label": UNVERIFIABLE, "why": f"served path not derivable: {served_method}",
                "client": f"{method} {url}"}
    common = {"client": f"{method} {url}", "served": f"{served_method} {served}"}
    agreement = paths_agree(url, served)
    if agreement is False:
        return {"label": FALSE_POSITIVE,
                "why": f"client {canonical(url)} does not reach served {canonical(served)}",
                **common}
    if agreement == UNKNOWN_AGAINST_LITERAL:
        return {"label": UNVERIFIABLE,
                "why": "a client value the source does not fix, against a literal route segment",
                **common}
    if method and served_method and method != served_method:
        return {"label": FALSE_POSITIVE,
                "why": f"method {method} != served {served_method}", **common}
    return {"label": TRUE_POSITIVE,
            "why": f"{method or 'UNKNOWN'} {canonical(url)} is served here", **common}
