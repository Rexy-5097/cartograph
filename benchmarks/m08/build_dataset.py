#!/usr/bin/env python3
"""Build the M08 labelled evaluation dataset.

Labels every edge Cartograph produced, from repository source, using
instruments that share no code with the analyser. Writes a dataset bound to
the digests of everything it depended on, so a result cannot outlive the
inputs that justify it.

Standard library only.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import subprocess
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
sys.path.insert(0, os.path.join(ROOT, "benchmarks", "m08"))
import label_edges as L  # noqa: E402


def sha256(path):
    h = hashlib.sha256()
    with open(path, "rb") as handle:
        for block in iter(lambda: handle.read(1 << 20), b""):
            h.update(block)
    return h.hexdigest()


def records_digest(records):
    """A digest over the labelled records themselves.

    The `bound_to` digests cover the dataset's *inputs*. They do not notice a
    record being deleted, relabelled, moved between splits, or given a
    different confidence — four attacks that all succeeded against the first
    version of the integrity checks. This closes them: any edit to the records
    changes this digest, and the only honest way to change the records is to
    rebuild the dataset, which re-derives every label from source.
    """
    canonical = json.dumps(
        [
            [r["repository"], r["kind"], r["at"], r["edge"],
             r["confidence"], r["provenance"], r["label"], r["split"]]
            for r in records
        ],
        sort_keys=True, separators=(",", ":"),
    )
    return hashlib.sha256(canonical.encode()).hexdigest()


def holdout_bucket(record, salt):
    """Deterministic development/holdout split.

    Split on a digest of the edge's own identity, not on its label, its
    confidence, or its position in the file. A split that can see the outcome
    is not a split.
    """
    key = f"{salt}|{record['repository']}|{record['kind']}|{record['at']}|{record['edge']}"
    digest = hashlib.sha256(key.encode()).hexdigest()
    return "holdout" if int(digest[:8], 16) % 4 == 0 else "development"


def main():
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--corpus", default=os.path.join(ROOT, "benchmarks/corpus.json"))
    ap.add_argument("--raw", required=True, help="directory of raw analyser output")
    ap.add_argument("--mirror", required=True)
    ap.add_argument("--measurement", required=True, help="the run these graphs came from")
    ap.add_argument("--salt", default="m08-holdout-v1")
    ap.add_argument("--out", required=True)
    args = ap.parse_args()

    corpus = json.load(open(args.corpus, encoding="utf-8"))
    measurement = json.load(open(args.measurement, encoding="utf-8"))

    records = []
    per_repo = {}
    for entry in corpus["repositories"]:
        name = entry["name"]
        graph_path = os.path.join(args.raw, f"{name}.json")
        if not os.path.exists(graph_path):
            continue
        checkout = os.path.join(args.mirror, name)
        head = subprocess.run(["git", "-C", checkout, "rev-parse", "HEAD"],
                              capture_output=True, text=True, check=False).stdout.strip()
        if head != entry["commit"]:
            raise SystemExit(f"{name} is at {head}, corpus pins {entry['commit']}")

        graph = json.load(open(graph_path, encoding="utf-8"))["graph"]
        source = L.Source(checkout)
        routers = L.RouterIndex(source, L.python_files(checkout))
        labelled = L.label_graph(graph, source, routers)
        for r in labelled:
            r["repository"] = name
            r["split"] = holdout_bucket(r, args.salt)
        records.extend(labelled)
        per_repo[name] = len(labelled)
        print(f"  {name:<20} {len(labelled):>6} edges labelled", file=sys.stderr)

    document = {
        "schema": "cartograph.m08-dataset/1",
        "milestone": "M08",
        "status": "INTERNAL",
        "independence": (
            "Every label is re-derived from repository source by benchmarks/m08/label_edges.py, "
            "which shares no code with the analyser and implements route composition a second "
            "time from the frameworks' semantics. No label is taken from Cartograph output."
        ),
        "split": {
            "method": "sha256 of (salt, repository, kind, location, edge id) modulo 4; bucket 0 is holdout",
            "salt": args.salt,
            "rationale": "Split on edge identity, never on label or confidence.",
        },
        "bound_to": {
            "cartograph_commit": measurement["toolchain"]["cartograph_commit"],
            "cartograph_tree_clean": measurement["toolchain"]["cartograph_tree_clean"],
            "corpus_sha256": sha256(args.corpus),
            "supported_subset_sha256": sha256(os.path.join(ROOT, "benchmarks/supported-subset.json")),
            "measurement_sha256": sha256(args.measurement),
            "labeller_sha256": sha256(os.path.join(ROOT, "benchmarks/m08/label_edges.py")),
            "repository_commits": {e["name"]: e["commit"] for e in corpus["repositories"]},
        },
        "records_sha256": records_digest(records),
        "counts": {
            "edges": len(records),
            "per_repository": per_repo,
            "per_label": {
                lab: sum(1 for r in records if r["label"] == lab)
                for lab in sorted({r["label"] for r in records})
            },
            "per_split": {
                sp: sum(1 for r in records if r["split"] == sp)
                for sp in sorted({r["split"] for r in records})
            },
        },
        "records": records,
    }
    os.makedirs(os.path.dirname(args.out), exist_ok=True)
    json.dump(document, open(args.out, "w", encoding="utf-8"), indent=1, sort_keys=True)
    print(f"wrote {args.out} ({len(records)} records)", file=sys.stderr)
    return 0


if __name__ == "__main__":
    sys.exit(main())
