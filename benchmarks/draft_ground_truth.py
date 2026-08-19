#!/usr/bin/env python3
"""Draft ground-truth records from repository source.

Reads a corpus checkout and nothing else. It has no access to Cartograph's
output, cannot import the analyser, and is given only a filesystem path — so
generated output cannot influence the labels it is later scored against
(PART 5).

What it does is narrow on purpose:

  * enumerate route declarations with `enumerate_source`, whose patterns
    describe the frameworks rather than the analyser;
  * read the receiver, verb, path and handler name out of the source text;
  * apply the scope rules declared in supported-subset.json.

What it does NOT do is decide scope. Every classification is a lookup of a
rule id fixed before any measurement. A draft is then reviewed against the
source by hand before it becomes a ground-truth file; the review is recorded
in each file's `verification` block.
"""

from __future__ import annotations

import argparse
import json
import os
import re
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import enumerate_source as es  # noqa: E402

DEF = re.compile(r"^\s*(?:async\s+)?def\s+(\w+)")
DECORATOR = re.compile(r"@([\w.]+)\.(get|post|put|patch|delete|head|options|route)\s*\(", re.I)
PATH_LITERAL = re.compile(r"""["'`]([^"'`]*)["'`]""")
URLCONF = re.compile(r"""^\s*(re_path|path|\w+)\s*\(\s*r?["']([^"']*)["']""")
METHODS = re.compile(r"""methods\s*=\s*[\[(]([^\])]*)[\])]""")

# Receivers cartograph-parser recognises. Recorded so a miss can be attributed
# to GAP-1 rather than merely observed; it never changes the scope decision.
RECOGNISED_RECEIVERS = {"app", "router", "api", "blueprint", "bp"}

# Callables that wrap a view rather than being one.
WRAPPERS = {
    "csrf_exempt",
    "never_cache",
    "login_required",
    "cache_page",
    "require_POST",
    "require_GET",
    "staff_member_required",
    "include",
}


def recognised(receiver):
    tail = receiver.split(".")[-1]
    return tail in RECOGNISED_RECEIVERS or tail.endswith(("_router", "_app"))


def classify(kind, callee, path):
    """Applies the pre-declared scope rules. Decides nothing on its own."""
    if kind == "expose" or kind == "class_based_view":
        return "UNSUPPORTED", "OUT-2"
    if kind == "imperative":
        return "UNSUPPORTED", "OUT-3"
    if kind == "urlconf":
        if callee == "re_path":
            return "UNSUPPORTED", "OUT-4"
        return "IN_SCOPE", "IN-BE-3"
    if kind == "urlconf_helper":
        if callee in {"path", "re_path"}:
            return ("UNSUPPORTED", "OUT-4") if callee == "re_path" else ("IN_SCOPE", "IN-BE-3")
        # A project-local wrapper around path(). Recognising an arbitrary
        # wrapper requires knowing what it forwards to, which is a different
        # capability from reading a declaration.
        return "UNSUPPORTED", "OUT-9"
    if kind in {"verb_decorator", "route_decorator"}:
        return "IN_SCOPE", "IN-BE-2" if kind == "route_decorator" else "IN-BE-1"
    return "UNSUPPORTED", "OUT-3"


def read(path):
    with open(path, encoding="utf-8", errors="replace") as handle:
        return handle.readlines()


def handler_after(lines, index):
    """The decorated function's name, for a decorator declaration."""
    for line in lines[index : index + 25]:
        match = DEF.match(line)
        if match:
            return match.group(1)
    return None


URLCONF_VIEW = re.compile(
    r"""^\s*(?:re_)?path\s*\(\s*r?["'][^"']*["']\s*,\s*([A-Za-z_][\w.]*(?:\.as_view\(\))?)"""
)


def urlconf_view(lines, index):
    """The view a URL-conf entry names.

    A `path(...)` entry does not decorate anything, so looking for a following
    `def` finds whatever function happens to come next in the file — which is
    how an admin helper ended up recorded as the handler for an unrelated
    route during review of the first draft.
    """
    window = " ".join(part.strip() for part in lines[index : index + 3])
    match = URLCONF_VIEW.match(window)
    return match.group(1) if match else None


def prefix_in_file(lines):
    text = "".join(lines[:80])
    match = re.search(r"""(?:APIRouter|Blueprint)\([^)]*?(?:url_)?prefix\s*=\s*["']([^"']+)""", text, re.S)
    return match.group(1) if match else None


def build(root, sample):
    """Selects records by a stratified stride, fixed before any measurement.

    A flat stride over the whole population tracks whichever declaration form
    happens to dominate a repository. Superset declares 309 routes with
    Flask-AppBuilder's `@expose` and 10 with a Flask decorator, so a flat
    sample of 24 contained no in-scope route at all — the evaluation would
    have said nothing about the form the analyser actually claims to handle.

    Sampling per declaration form fixes that without special-casing any
    repository: each form contributes up to `sample` records, so both the
    dominant form and the rare one are represented everywhere.
    """
    found = es.scan(root, {".py"}, es.ROUTE_PATTERNS)
    if sample:
        buckets = {}
        for item in found:
            buckets.setdefault(item["kind"], []).append(item)
        selected = []
        for kind in sorted(buckets):
            selected.extend(es.stride_sample(buckets[kind], sample))
        selected.sort(key=lambda i: (i["file"], i["line"]))
    else:
        selected = found

    records = []
    for item in selected:
        lines = read(os.path.join(root, item["file"]))
        index = item["line"] - 1
        window = " ".join(part.strip() for part in lines[index : index + 3])

        callee, verb, path = None, None, None
        decorator = DECORATOR.search(window)
        if decorator:
            callee, verb = decorator.group(1), decorator.group(2).lower()
            after = window[decorator.end() :]
            literal = PATH_LITERAL.search(after)
            path = literal.group(1) if literal else None
        else:
            conf = URLCONF.match(item["text"]) or URLCONF.match(window)
            if conf:
                callee, path = conf.group(1), conf.group(2)

        view = None if decorator else urlconf_view(lines, index)
        scope, rule = classify(item["kind"], callee, path)

        # `path("api/x/", include(other.urls))` mounts a URL module. It
        # declares no handler and serves no request itself, so counting it as
        # a route would put an unanswerable record in the denominator.
        if view == "include":
            scope, rule = "UNSUPPORTED", "OUT-10"

        # A view reached through a decorator call or a class — csrf_exempt(v),
        # self.admin_site.admin_view(v), SomeView.as_view() — is named by the
        # wrapper, not by itself. The route declaration is still in scope; the
        # handler's identity is not, so the record cannot support a chain.
        handler_resolvable = True
        if view is not None and (
            view.endswith(".as_view") or view in WRAPPERS or "." in view
        ):
            handler_resolvable = False

        methods = None
        if verb == "route":
            declared = METHODS.search(window)
            methods = (
                [m.strip().strip("\"'").upper() for m in declared.group(1).split(",") if m.strip()]
                if declared
                else []
            )
        elif verb:
            methods = [verb.upper()]

        record = {
            "id": f"{item['file']}:{item['line']}",
            "relationship": "ROUTE_DECLARATION",
            "kind": item["kind"],
            "file": item["file"],
            "line": item["line"],
            "receiver": callee,
            "path_as_written": path,
            "methods_as_written": methods,
            "handler": handler_after(lines, index) if decorator else view,
            "handler_resolvable": handler_resolvable,
            "in_test_code": any(
                part in item["file"].split("/") for part in ("tests", "test", "testing")
            )
            or os.path.basename(item["file"]).startswith("test_"),
            "source": item["text"],
            "classification": scope,
            "rule": rule,
        }
        # GAP-1 is about the receiver a decorator is bound to. A url-conf
        # entry has no receiver, so applying it there would invent a gap.
        if (
            scope == "IN_SCOPE"
            and item["kind"] in {"verb_decorator", "route_decorator"}
            and callee
            and not recognised(callee)
        ):
            record["predicted_gap"] = "GAP-1"
        prefix = prefix_in_file(lines)
        if prefix:
            record["registration_prefix"] = prefix
            record["note"] = "served path requires prefix composition (OUT-5)"
        # An out-of-scope form is out of scope whether or not this instrument
        # can read its path back — @expose and add_url_rule carry paths in
        # shapes these regexes do not parse, and reclassifying them here would
        # erase a correct decision. Only an in-scope record needs a path.
        if path is None and scope == "IN_SCOPE":
            record["classification"] = "INSTRUMENT_NO_PATH"
            record["rule"] = "N/A"
        records.append(record)
    return found, records


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("root")
    parser.add_argument("--name", required=True)
    parser.add_argument("--commit", required=True)
    parser.add_argument("--sample", type=int, default=0)
    parser.add_argument("--out", required=True)
    args = parser.parse_args()

    found, records = build(args.root, args.sample)

    counts = {}
    for record in records:
        counts[record["rule"]] = counts.get(record["rule"], 0) + 1

    document = {
        "schema": "cartograph.ground-truth/1",
        "repository": args.name,
        "commit": args.commit,
        "independence": "Authored from repository source by benchmarks/draft_ground_truth.py, which is given a filesystem path and has no access to Cartograph output. Reviewed against source before use.",
        "sampling": {
            "population": len(found),
            "selected": len(records),
            "rule": "matches from benchmarks/enumerate_source.py, bucketed by declaration form; within each form, sorted by (file, line) and taken at an even stride up to the per-form target. Stratified so a repository dominated by one form still contributes records of the others. Fixed before any result was seen.",
            "per_form_target": args.sample or "all",
        },
        "rule_counts": counts,
        "route_declarations": records,
        "relationships": [],
        "chains": [],
        "must_not_produce": [],
        "verification": {"reviewed": False, "notes": []},
    }
    with open(args.out, "w", encoding="utf-8") as handle:
        json.dump(document, handle, indent=1, sort_keys=True)
    print(f"{args.name}: population={len(found)} selected={len(records)} {counts}", file=sys.stderr)
    return 0


if __name__ == "__main__":
    sys.exit(main())
