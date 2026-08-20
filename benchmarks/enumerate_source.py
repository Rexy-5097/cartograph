#!/usr/bin/env python3
"""Enumerate route declarations and client calls from source, independently.

This is the ground-truth *instrument*, and it exists to disagree with
Cartograph. Its patterns describe what the web frameworks actually serve, not
what `cartograph-parser` recognises, and they deliberately over-approximate:
a form Cartograph cannot see must still appear here, or a false negative could
never be discovered (PART 11).

It shares no code with the analyser. It is regex over text — crude on purpose,
because a second implementation of the same clever thing would make the same
clever mistakes.

Output is a candidate list. Classification against the supported subset is a
human judgement recorded in benchmarks/ground-truth/, not something this
script decides.
"""

from __future__ import annotations

import argparse
import json
import os
import re
import sys

VERBS = "get|post|put|patch|delete|head|options"

# `patch` is both an HTTP verb and the name of the most common test decorator
# in Python, so `@mock.patch("app.core.settings")` matches a naive verb rule.
# A route decorator's first argument is a URL path, which contains a slash;
# a mock target is a dotted import path, which does not. Requiring the slash
# separates them without naming individual libraries.
PATH_ARG = r"""\s*\(\s*f?["'`][^"'`]*/"""

ROUTE_PATTERNS = [
    # FastAPI / Flask verb decorator on any receiver, including ones outside
    # Cartograph's receiver rule (e.g. @orders_bp.get).
    ("verb_decorator", re.compile(rf"^\s*@([\w.]+)\.({VERBS}){PATH_ARG}", re.I)),
    # Flask classic, any receiver.
    ("route_decorator", re.compile(rf"^\s*@([\w.]+)\.route{PATH_ARG}")),
    # Django URL conf.
    ("urlconf", re.compile(r"^\s*(?:re_)?path\s*\(")),
    # Django URL conf via a project-local helper (Zulip's rest_path, etc.).
    ("urlconf_helper", re.compile(r"^\s*(\w*_?path|rest_path|url)\s*\(\s*[\"']")),
    # Imperative registration, which no decorator pattern can catch.
    ("imperative", re.compile(r"\.(add_url_rule|add_api_route|register|add_route)\s*\(")),
    # Flask-AppBuilder class-based exposure.
    ("expose", re.compile(rf"^\s*@expose{PATH_ARG}")),
    # Django REST Framework / class-based views.
    ("class_based_view", re.compile(r"class\s+\w+\s*\(.*(APIView|ViewSet|GenericAPIView)")),
]

CALL_PATTERNS = [
    ("fetch", re.compile(r"\bfetch\s*\(")),
    ("verb_call", re.compile(rf"\b[\w.$\]\[]+\.({VERBS})\s*\(", re.I)),
    ("axios_config", re.compile(r"\baxios\s*\(\s*\{")),
    ("request_config", re.compile(r"\.\s*(request|call|fetchApi|apiRequest)\s*\(\s*\{")),
    ("url_property", re.compile(r"""\burl\s*:\s*[`'"](/[^`'"]*)""")),
]

SKIP_DIRS = {
    "node_modules", ".git", "dist", "build", "coverage", "target",
    "__pycache__", "site-packages", "venv", ".venv", "migrations",
}


def inert_lines(lines):
    """Line numbers inside a triple-quoted string or a comment.

    A regex over raw text cannot tell a route declaration from an example of
    one. Onyx and AutoGPT both document their dependencies with

        Usage from FastAPI::

            @router.post("/tokens")
            def create_token(...):

    inside a docstring. Cartograph, which parses, ignored all three; this
    instrument counted them as routes and charged Cartograph three false
    negatives for correctly declining to extract documentation.
    """
    skip, inside = set(), None
    for number, line in enumerate(lines, start=1):
        if inside is not None:
            skip.add(number)
            if inside in line:
                inside = None
            continue
        if line.lstrip().startswith("#"):
            skip.add(number)
            continue
        for quote in ('"""', "'''"):
            start = line.find(quote)
            if start != -1:
                if quote not in line[start + 3 :]:
                    inside = quote
                    skip.add(number)
                break
    return skip


def walk(root, extensions):
    for dirpath, dirnames, filenames in os.walk(root):
        dirnames[:] = [d for d in dirnames if d not in SKIP_DIRS and not d.startswith(".")]
        for name in sorted(filenames):
            if name.startswith("._"):
                continue
            if os.path.splitext(name)[1] in extensions:
                yield os.path.join(dirpath, name)


def scan(root, extensions, patterns):
    found = []
    for path in sorted(walk(root, extensions)):
        try:
            with open(path, encoding="utf-8", errors="replace") as handle:
                lines = handle.readlines()
        except OSError:
            continue
        rel = os.path.relpath(path, root)
        inert = inert_lines(lines)
        for number, line in enumerate(lines, start=1):
            if len(line) > 500 or number in inert:
                continue
            # A decorator's path often sits on the following line:
            #
            #     @router.get(
            #         "/users/{id}",
            #
            # so each line is examined together with the two after it. The
            # recorded position stays the first line, which is where the
            # declaration begins.
            window = line
            if line.rstrip().endswith(("(", ",")):
                window = " ".join(
                    part.strip() for part in lines[number - 1 : number + 2]
                )
            for kind, pattern in patterns:
                match = pattern.search(window)
                if match:
                    found.append(
                        {
                            "kind": kind,
                            "file": rel.replace(os.sep, "/"),
                            "line": number,
                            "text": line.strip()[:200],
                        }
                    )
                    break
    return found


def stride_sample(items, target):
    """A deterministic sample, fixed before any result is seen.

    Sorted by (file, line) and taken at an even stride so the selection cannot
    track which entries Cartograph happened to get right (PART 17).
    """
    items = sorted(items, key=lambda i: (i["file"], i["line"]))
    if len(items) <= target:
        return items
    step = len(items) / target
    return [items[int(i * step)] for i in range(target)]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("root")
    parser.add_argument("--what", choices=["routes", "calls"], required=True)
    parser.add_argument("--sample", type=int, default=0, help="0 means every match")
    parser.add_argument("--summary", action="store_true")
    args = parser.parse_args()

    if args.what == "routes":
        found = scan(args.root, {".py"}, ROUTE_PATTERNS)
    else:
        found = scan(args.root, {".ts", ".tsx"}, CALL_PATTERNS)

    if args.summary:
        counts = {}
        for item in found:
            counts[item["kind"]] = counts.get(item["kind"], 0) + 1
        json.dump({"total": len(found), "by_kind": counts}, sys.stdout, indent=2, sort_keys=True)
        print()
        return 0

    selected = stride_sample(found, args.sample) if args.sample else found
    json.dump(
        {"root": args.root, "total_found": len(found), "selected": len(selected), "items": selected},
        sys.stdout,
        indent=1,
    )
    print()
    return 0


if __name__ == "__main__":
    sys.exit(main())
