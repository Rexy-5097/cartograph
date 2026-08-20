#!/usr/bin/env python3
"""Run Cartograph over the pinned corpus and record what it produced.

This script measures. It does not judge: no relationship is labelled here, so
a change to the evaluator cannot change what was observed, and a change here
cannot change how it scores. `evaluate.py` reads what this writes.

Standard library only, matching `agentos/tools/scripts/run_gates.py`. The
benchmark adds no dependency to the project (PART 25).
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import platform
import re
import subprocess
import sys
import time

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))


def sha256_file(path):
    h = hashlib.sha256()
    with open(path, "rb") as handle:
        for block in iter(lambda: handle.read(1 << 20), b""):
            h.update(block)
    return h.hexdigest()


def git(repo, *args):
    return subprocess.run(
        ["git", "-C", repo, *args], capture_output=True, text=True, check=False
    ).stdout.strip()


def toolchain_metadata(binary):
    """Facts about *this* run, re-derived rather than copied from the corpus.

    Falsifying benchmark metadata (PART 23, item 9) means editing a file the
    evaluator trusts. Everything here is read from the environment at run
    time, so the results carry what actually ran.
    """
    return {
        "rustc": subprocess.run(
            ["rustc", "--version"], capture_output=True, text=True, check=False
        ).stdout.strip(),
        "cartograph_version": subprocess.run(
            [binary, "version"], capture_output=True, text=True, check=False
        ).stdout.strip(),
        "cartograph_commit": git(ROOT, "rev-parse", "HEAD"),
        "cartograph_tree_clean": git(ROOT, "status", "--porcelain") == "",
        "binary_sha256": sha256_file(binary),
        "os": f"{platform.system()} {platform.release()} {platform.machine()}",
        "python": platform.python_version(),
    }


def capture_normalize(binary, target, raw_dir, name):
    """Records every canonicalised observation the analyser made.

    `match --json` reports decisions and the graph, but a route that took part
    in no decision never appears in it — so route-extraction recall cannot be
    measured from it. `normalize --json` lists every observation with its
    file, line and handler, which is what a ground-truth route record has to
    be compared against.
    """
    proc = subprocess.run(
        [binary, "normalize", "--json", target],
        capture_output=True, text=True, check=False,
    )
    if proc.returncode != 0:
        return None, {"status": "NORMALIZE_FAILED", "stderr_tail": proc.stderr[-1000:]}
    try:
        report = json.loads(proc.stdout)
    except json.JSONDecodeError as exc:
        return None, {"status": "NORMALIZE_BAD_OUTPUT", "error": str(exc)}

    path = os.path.join(raw_dir, f"{name}.normalize.json")
    with open(path, "w", encoding="utf-8") as handle:
        json.dump(report, handle, indent=1, sort_keys=True)
    return path, {
        "status": "OK",
        "raw_path": os.path.relpath(path, ROOT),
        "raw_sha256": sha256_file(path),
        "route_declarations": report["totals"]["route_declarations"],
        "client_calls": report["totals"]["client_calls"],
    }


def measure(binary, target):
    """Runs one analysis, returning its output, wall time and peak RSS.

    `/usr/bin/time -l` reports peak resident set size on macOS; on a platform
    where it does not, memory is recorded as null rather than guessed.
    """
    started = time.monotonic()
    proc = subprocess.run(
        ["/usr/bin/time", "-l", binary, "match", "--json", target],
        capture_output=True,
        text=True,
        check=False,
    )
    elapsed = time.monotonic() - started

    peak = None
    match = re.search(r"(\d+)\s+maximum resident set size", proc.stderr)
    if match:
        peak = int(match.group(1))

    return proc, elapsed, peak


def analyse(entry, args, metadata):
    name = entry["name"]
    mirror = os.path.join(args.mirror, name)

    if not os.path.isdir(os.path.join(mirror, ".git")):
        return {"name": name, "status": "MISSING", "error": f"no checkout at {mirror}"}

    # PART 23, item 10: evaluation against the wrong commit is the easiest
    # mistake to make and the hardest to notice, so the checkout is verified
    # against the corpus rather than assumed.
    head = git(mirror, "rev-parse", "HEAD")
    if head != entry["commit"]:
        return {
            "name": name,
            "status": "WRONG_COMMIT",
            "expected": entry["commit"],
            "actual": head,
        }

    target = mirror if entry["analysis_root"] == "." else os.path.join(
        mirror, entry["analysis_root"]
    )
    if not os.path.exists(target):
        return {"name": name, "status": "MISSING_ROOT", "error": target}

    proc, elapsed, peak = measure(args.binary, target)
    if proc.returncode != 0:
        # A crash is a result, not a reason to drop the repository (PART 17).
        return {
            "name": name,
            "status": "ANALYSIS_FAILED",
            "returncode": proc.returncode,
            "stderr_tail": proc.stderr[-2000:],
            "wall_seconds": round(elapsed, 3),
        }

    try:
        report = json.loads(proc.stdout)
    except json.JSONDecodeError as exc:
        return {"name": name, "status": "BAD_OUTPUT", "error": str(exc)}

    os.makedirs(args.raw, exist_ok=True)
    raw_path = os.path.join(args.raw, f"{name}.json")
    with open(raw_path, "w", encoding="utf-8") as handle:
        json.dump(report, handle, indent=1, sort_keys=True)

    _, normalize = capture_normalize(args.binary, target, args.raw, name)

    totals = report["totals"]
    graph = report["graph"]
    by_kind = {}
    for edge in graph["edges"]:
        by_kind[edge["kind"]] = by_kind.get(edge["kind"], 0) + 1

    return {
        "name": name,
        "status": "OK",
        "repository": entry["repository"],
        "commit": head,
        "analysis_root": entry["analysis_root"],
        # PART 23, item 2: the evaluator records the digest of the output it
        # scored, so output edited after the fact no longer matches.
        "raw_sha256": sha256_file(raw_path),
        "raw_path": os.path.relpath(raw_path, ROOT),
        "normalize": normalize,
        "measurements": {
            "files_analysed": totals["files"],
            "backend_routes": totals["backend_routes"],
            "client_calls": totals["client_calls"],
            "decisions": {
                "exact": totals["exact"],
                "strong": totals["strong"],
                "ambiguous": totals["ambiguous"],
                "no_match": totals["no_match"],
                "unsupported": totals["unsupported"],
            },
            "dynamic_urls": {
                "fully_resolved": totals["dynamic_resolved"],
                "partially_resolved": totals["dynamic_partial"],
            },
            "orm": {
                "models": totals["orm_models"],
                "tables_resolved": totals["orm_tables"],
                "ambiguous_model_names": totals["orm_ambiguous"],
                "edges": totals["orm_edges"],
            },
            "graph": {
                "nodes": graph["node_count"],
                "edges": graph["edge_count"],
                "edges_by_kind": by_kind,
            },
        },
        "performance": {
            "analysis_ms": report["elapsed_ms"],
            "wall_seconds": round(elapsed, 3),
            "peak_rss_bytes": peak,
        },
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--corpus", default=os.path.join(ROOT, "benchmarks/corpus.json"))
    parser.add_argument("--binary", default=os.path.join(ROOT, "target/release/cartograph"))
    parser.add_argument("--mirror", required=True, help="directory holding the pinned checkouts")
    parser.add_argument("--raw", required=True, help="where raw analyser output is written")
    parser.add_argument("--pass-number", type=int, required=True)
    parser.add_argument("--out", required=True)
    args = parser.parse_args()

    with open(args.corpus, encoding="utf-8") as handle:
        corpus = json.load(handle)

    metadata = toolchain_metadata(args.binary)
    results = [analyse(entry, args, metadata) for entry in corpus["repositories"]]

    for row in results:
        print(f"{row['status']:<16} {row['name']}", file=sys.stderr)

    output = {
        "schema": "cartograph.benchmark-measurement/1",
        "milestone": "M07",
        "status": "INTERNAL",
        "pass": args.pass_number,
        "corpus_sha256": sha256_file(args.corpus),
        "toolchain": metadata,
        "repositories": results,
    }
    os.makedirs(os.path.dirname(args.out), exist_ok=True)
    with open(args.out, "w", encoding="utf-8") as handle:
        json.dump(output, handle, indent=2, sort_keys=True)
    print(f"wrote {args.out}", file=sys.stderr)

    failures = [r for r in results if r["status"] != "OK"]
    return 1 if failures else 0


if __name__ == "__main__":
    sys.exit(main())
