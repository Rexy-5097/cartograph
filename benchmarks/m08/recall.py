#!/usr/bin/env python3
"""Independent recall denominators for the relationship classes M07 left open.

M07 reported HttpCall, OrmAccess and chain recall as UNMEASURED, because no
denominator existed that had not been produced by the analyser itself. This
builds those denominators from source, inside scopes narrow enough to be
enumerated **completely**, and says exactly where each scope ends.

A denominator is only honest if it is complete within a stated boundary. None
of these is "recall on the corpus"; each is recall within a scope named in the
result.

Standard library only.
"""

from __future__ import annotations

import argparse
import json
import os
import re
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
sys.path.insert(0, os.path.join(ROOT, "benchmarks", "m08"))
import label_edges as L  # noqa: E402

# A generated-client operation: `method: 'POST',` then `url: '/api/v2/pools',`.
GEN_OP = re.compile(
    r"""method:\s*'([A-Z]+)',\s*\n\s*url:\s*'([^']+)'""", re.M
)
MODEL_CONSTRUCT = re.compile(r"\b(\w+)\s*\(")
MANAGER_CALL = re.compile(r"\b(\w+)\.objects\.\w+\s*\(")


def http_call_recall(mirror, repo, graph, client_glob):
    """Recall over a generated client whose operations can be enumerated.

    Scope: every request the generated client can issue, read from its own
    source. Complete within that file — the generator emits one entry per
    operation, each with a literal method and URL, so the set is closed.
    """
    root = os.path.join(mirror, repo)
    source = L.Source(root)
    routers = L.RouterIndex(source, L.python_files(root))

    # Independent: the served path of every route declaration in the project.
    served = {}
    for rel in L.python_files(root):
        lines = source.lines(rel)
        for i, line in enumerate(lines, start=1):
            if not L.DECORATOR.match(line):
                continue
            path, verb = L.compose_served_path(source, rel, i, routers)
            if path is not None:
                served.setdefault((verb, L.canonical(path)), []).append(f"{rel}:{i}")

    # Independent: every operation the generated client declares.
    operations = []
    for rel in _files_matching(root, client_glob):
        text = "".join(source.lines(rel))
        for m in GEN_OP.finditer(text):
            verb, url = m.group(1), m.group(2)
            line = text[: m.start()].count("\n") + 1
            operations.append({"file": rel, "line": line, "method": verb, "url": url})

    # The denominator: operations a route actually serves.
    in_scope, unmatched = [], []
    for op in operations:
        key = (op["method"], L.canonical(L.request_path(op["url"])))
        if key in served:
            in_scope.append({**op, "routes": served[key]})
        else:
            unmatched.append(op)

    produced = set()
    for e in graph["edges"]:
        if e["kind"] == "http-call":
            produced.add((e["file"], e["line"]))
    # A generated operation spans several lines; accept an edge anywhere in it.
    found, missed = [], []
    for op in in_scope:
        hit = any(op["file"] == f and op["line"] - 2 <= l <= op["line"] + 2 for f, l in produced)
        (found if hit else missed).append(op)

    return {
        "scope": f"{repo}: every operation declared by the generated client ({client_glob})",
        "operations_declared": len(operations),
        "operations_a_route_serves": len(in_scope),
        "operations_no_route_serves": len(unmatched),
        "recovered": len(found),
        "missed": len(missed),
        "recall": round(len(found) / len(in_scope), 4) if in_scope else None,
        "missed_examples": missed[:10],
        "boundary": (
            "Complete for this generated client. It is NOT recall over every "
            "client call in the repository: hand-written callers, other "
            "clients and dynamic URLs are outside the scope and are neither "
            "counted nor claimed."
        ),
    }


def _files_matching(root, needle):
    out = []
    for dirpath, dirnames, filenames in os.walk(root):
        dirnames[:] = [d for d in dirnames if d not in L.SKIP_DIRS and not d.startswith(".")]
        for name in filenames:
            rel = os.path.relpath(os.path.join(dirpath, name), root).replace(os.sep, "/")
            if needle in rel:
                out.append(rel)
    return sorted(out)


def orm_access_recall(mirror, repo, graph, models, limit_files=None):
    """Recall over model uses that source states directly.

    Scope: for each model whose table this project declares, every Python site
    that names it as a constructor or a manager call **and** binds the name to
    that model's own module. Import-bound only — a name reached by a star
    import is excluded from the denominator rather than guessed at.
    """
    root = os.path.join(mirror, repo)
    source = L.Source(root)
    files = L.python_files(root)
    if limit_files:
        files = files[:limit_files]

    by_name = {m["model"]: m for m in models if m.get("table")}
    expected = []
    for rel in files:
        text = "".join(source.lines(rel))
        if not any(n in text for n in by_name):
            continue
        for i, line in enumerate(source.lines(rel), start=1):
            for pat in (MODEL_CONSTRUCT, MANAGER_CALL):
                for m in pat.finditer(line):
                    name = m.group(1)
                    if name not in by_name:
                        continue
                    binding = source.binds(rel, name)
                    if binding in (None, "*", "here"):
                        continue  # not import-bound to the model's module
                    tail = binding.replace(".", "/")
                    decl = by_name[name]["file"]
                    if decl.endswith(f"{tail}.py") or decl.endswith(f"{tail}/__init__.py"):
                        expected.append({"model": name, "file": rel, "line": i})
    seen = set()
    unique = []
    for e in expected:
        key = (e["model"], e["file"], e["line"])
        if key not in seen:
            seen.add(key)
            unique.append(e)

    produced = set()
    for e in graph["edges"]:
        if e["kind"] != "orm-access":
            continue
        m = L.ACCESS_EVIDENCE.match(e["evidence"])
        if m:
            produced.add((m.group("file"), int(m.group("line"))))
    found = [e for e in unique if (e["file"], e["line"]) in produced]
    missed = [e for e in unique if (e["file"], e["line"]) not in produced]
    return {
        "scope": f"{repo}: import-bound constructor and manager uses of models with a declared table",
        "expected": len(unique),
        "recovered": len(found),
        "missed": len(missed),
        "recall": round(len(found) / len(unique), 4) if unique else None,
        "missed_examples": missed[:10],
        "boundary": (
            "Excludes star-imported and locally-declared names, uses reached "
            "through aliases this reader does not follow, and any access shape "
            "other than `Model(...)` or `Model.objects.…`."
        ),
    }


def chain_recall(mirror, repo, graph, http, models):
    """Recall over complete chains, within the same enumerated client scope.

    A chain is in scope when source independently shows all of: a generated
    client operation, a route that serves it, a handler that constructs a model
    with a declared table. It counts as recovered only when Cartograph's graph
    traverses the whole path through shared nodes.
    """
    root = os.path.join(mirror, repo)
    source = L.Source(root)
    by_name = {m["model"]: m for m in models if m.get("table")}

    expected = []
    for op in http.get("_in_scope", []):
        for route in op["routes"]:
            rel, line = route.rsplit(":", 1)
            handler, body = _handler_at(source, rel, int(line))
            if handler is None:
                continue
            for name in set(MODEL_CONSTRUCT.findall(body)):
                if name not in by_name:
                    continue
                binding = source.binds(rel, name)
                if binding in (None, "*", "here"):
                    continue
                tail = binding.replace(".", "/")
                decl = by_name[name]["file"]
                if decl.endswith(f"{tail}.py") or decl.endswith(f"{tail}/__init__.py"):
                    expected.append({
                        "client": f"{op['file']}:{op['line']}",
                        "method": op["method"], "url": op["url"],
                        "handler": handler, "model": name,
                        "table": by_name[name]["table"],
                    })

    produced = _graph_chains(graph)
    found = [c for c in expected
             if any(p["handler"] == c["handler"] and p["model"] == c["model"]
                    and p["table"] == c["table"] for p in produced)]
    missed = [c for c in expected if c not in found]
    return {
        "scope": f"{repo}: chains whose client operation, route, handler and model are all independently readable",
        "expected": len(expected),
        "recovered": len(found),
        "missed": len(missed),
        "recall": round(len(found) / len(expected), 4) if expected else None,
        "recovered_examples": found[:6],
        "missed_examples": missed[:6],
        "boundary": (
            "Bounded by the generated-client scope above and by the model "
            "access shapes this reader recognises. A handler reaching its "
            "model through `select(Model)` or a helper is outside it."
        ),
    }


DEF = re.compile(r"^\s*(?:async\s+)?def\s+(\w+)\s*\(")


def _handler_at(source, rel, line):
    """The function a decorator at `line` decorates, and its body text."""
    lines = source.lines(rel)
    for i in range(line - 1, min(line + 30, len(lines))):
        m = DEF.match(lines[i])
        if m:
            body = "".join(lines[i : i + 80])
            return m.group(1), body
    return None, ""


def _graph_chains(graph):
    nodes = {n["id"]: n for n in graph["nodes"]}
    out = {}
    for e in graph["edges"]:
        out.setdefault(e["source"], []).append(e)
    chains = []
    for e in graph["edges"]:
        if e["kind"] != "http-call":
            continue
        for a in [x for x in out.get(e["target"], []) if x["kind"] == "orm-access"]:
            for q in [x for x in out.get(a["target"], []) if x["kind"] == "queries"]:
                chains.append({
                    "handler": nodes[e["target"]]["name"],
                    "model": nodes[a["target"]]["name"],
                    "table": nodes[q["target"]]["name"],
                })
    return chains


def main():
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--raw", required=True)
    ap.add_argument("--mirror", required=True)
    ap.add_argument("--repo", default="airflow")
    ap.add_argument("--client", default="openapi-gen/requests/services.gen.ts")
    ap.add_argument("--out", required=True)
    args = ap.parse_args()

    graph = json.load(open(os.path.join(args.raw, f"{args.repo}.json"), encoding="utf-8"))["graph"]
    import subprocess
    result = subprocess.run(
        ["python3", os.path.join(ROOT, "benchmarks/enumerate_models.py"),
         os.path.join(args.mirror, args.repo)],
        capture_output=True, text=True, check=True,
    )
    models = json.loads(result.stdout)["models"]

    http = http_call_recall(args.mirror, args.repo, graph, args.client)
    # rebuild the in-scope operation list for the chain pass
    http["_in_scope"] = _rebuild_in_scope(args.mirror, args.repo, args.client)
    chains = chain_recall(args.mirror, args.repo, graph, http, models)
    orm = orm_access_recall(args.mirror, args.repo, graph, models)
    http.pop("_in_scope", None)

    document = {
        "schema": "cartograph.m08-recall/1",
        "milestone": "M08",
        "status": "INTERNAL",
        "repository": args.repo,
        "http_call_recall": http,
        "orm_access_recall": orm,
        "chain_recall": chains,
        "note": (
            "Each figure is recall within the scope it names, complete inside "
            "that boundary. None is recall over the whole corpus, and none "
            "should be quoted as such."
        ),
    }
    os.makedirs(os.path.dirname(args.out), exist_ok=True)
    json.dump(document, open(args.out, "w", encoding="utf-8"), indent=2, sort_keys=True)
    print(f"wrote {args.out}")
    return 0


def _rebuild_in_scope(mirror, repo, client_glob):
    root = os.path.join(mirror, repo)
    source = L.Source(root)
    routers = L.RouterIndex(source, L.python_files(root))
    served = {}
    for rel in L.python_files(root):
        for i, line in enumerate(source.lines(rel), start=1):
            if not L.DECORATOR.match(line):
                continue
            path, verb = L.compose_served_path(source, rel, i, routers)
            if path is not None:
                served.setdefault((verb, L.canonical(path)), []).append(f"{rel}:{i}")
    ops = []
    for rel in _files_matching(root, client_glob):
        text = "".join(source.lines(rel))
        for m in GEN_OP.finditer(text):
            verb, url = m.group(1), m.group(2)
            key = (verb, L.canonical(L.request_path(url)))
            if key in served:
                ops.append({"file": rel, "line": text[: m.start()].count("\n") + 1,
                            "method": verb, "url": url, "routes": served[key]})
    return ops


if __name__ == "__main__":
    raise SystemExit(main())
