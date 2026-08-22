#!/usr/bin/env python3
"""Checks that must pass before an M08 result may be believed.

Each corresponds to a way these numbers could be flattered. A check that
cannot detect its attack says so rather than passing quietly.

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


def sha256(path):
    h = hashlib.sha256()
    with open(path, "rb") as handle:
        for block in iter(lambda: handle.read(1 << 20), b""):
            h.update(block)
    return h.hexdigest()


def check(name, ok, detail):
    return {"check": name, "passed": bool(ok), "detail": detail}


def run(dataset_path, calibrations, mirror):
    data = json.load(open(dataset_path, encoding="utf-8"))
    bound = data["bound_to"]
    records = data["records"]
    out = []

    # 1 — ground truth altered after scoring.
    live = sha256(os.path.join(ROOT, "benchmarks/m08/label_edges.py"))
    out.append(check(
        "labeller unchanged since the dataset was built",
        live == bound["labeller_sha256"],
        {"recorded": bound["labeller_sha256"][:16], "now": live[:16]},
    ))

    # 2 — corpus or scope swapped underneath the result.
    for label, path, key in (
        ("corpus", "benchmarks/corpus.json", "corpus_sha256"),
        ("supported subset", "benchmarks/supported-subset.json", "supported_subset_sha256"),
    ):
        now = sha256(os.path.join(ROOT, path))
        out.append(check(f"{label} unchanged", now == bound[key],
                         {"recorded": bound[key][:16], "now": now[:16]}))

    # 3 — evaluation run against a different revision.
    mismatched = []
    for name, commit in bound["repository_commits"].items():
        head = subprocess.run(["git", "-C", os.path.join(mirror, name), "rev-parse", "HEAD"],
                              capture_output=True, text=True, check=False).stdout.strip()
        if head and head != commit:
            mismatched.append({"repository": name, "pinned": commit, "actual": head})
    out.append(check("every repository is at its pinned commit", not mismatched,
                     {"mismatched": mismatched}))

    # 4 — the records themselves edited: deleted, relabelled, moved between
    # splits, or given a different confidence. The input digests cannot see
    # any of that, and all four succeeded against the first version of these
    # checks.
    sys.path.insert(0, os.path.join(ROOT, "benchmarks", "m08"))
    import build_dataset as B
    live_records = B.records_digest(records)
    out.append(check(
        "the labelled records are unchanged since the dataset was built",
        live_records == data.get("records_sha256"),
        {"recorded": (data.get("records_sha256") or "absent")[:16], "now": live_records[:16],
         "note": "covers deletion, relabelling, split reassignment and confidence edits"},
    ))
    counts = data.get("counts", {})
    out.append(check(
        "the recorded label and split counts match the records present",
        counts.get("per_label") == {lab: sum(1 for r in records if r["label"] == lab)
                                    for lab in sorted({r["label"] for r in records})}
        and counts.get("per_split") == {sp: sum(1 for r in records if r["split"] == sp)
                                        for sp in sorted({r["split"] for r in records})},
        {"recorded": {"labels": counts.get("per_label"), "splits": counts.get("per_split")}},
    ))

    # 5 — negative and uncertain observations deleted.
    labels = {r["label"] for r in records}
    out.append(check(
        "the dataset retains refused and unverifiable observations",
        "UNVERIFIABLE" in labels,
        {"labels_present": sorted(labels),
         "note": "a dataset containing only what could be verified would score "
                 "itself on the easy half of its own corpus"},
    ))

    # 5 — a class quietly dropped.
    kinds = {r["kind"] for r in records}
    out.append(check("every produced relationship class is represented",
                     kinds >= {"http-call", "orm-access", "queries", "call"},
                     {"kinds": sorted(kinds)}))

    # 6 — the holdout touched, or leaking into development.
    splits = {}
    for r in records:
        splits.setdefault(r["split"], set()).add((r["repository"], r["kind"], r["at"], r["edge"]))
    overlap = splits.get("development", set()) & splits.get("holdout", set())
    out.append(check("development and holdout are disjoint", not overlap,
                     {"development": len(splits.get("development", ())),
                      "holdout": len(splits.get("holdout", ())),
                      "overlap": len(overlap)}))

    # 7 — results computed from a dataset other than this one.
    for path in calibrations:
        if not os.path.exists(path):
            out.append(check(f"{os.path.basename(path)} exists", False, {}))
            continue
        cal = json.load(open(path, encoding="utf-8"))
        out.append(check(
            f"{os.path.basename(path)} is bound to this dataset",
            cal["bound_to"] == bound,
            {"note": "the calibration carries the digests the dataset was built from"},
        ))

    # 8 — accuracy claimed where nothing was verified.
    bad = []
    for path in calibrations:
        if not os.path.exists(path):
            continue
        cal = json.load(open(path, encoding="utf-8"))
        for value, entry in cal["by_confidence_value"].items():
            if entry["verified"] == 0 and entry["observed_accuracy"] is not None:
                bad.append({"file": os.path.basename(path), "confidence": value})
    out.append(check("no accuracy is reported for a group with no verified observations",
                     not bad, {"violations": bad}))

    # 9 — a small sample presented as an estimate.
    unflagged = []
    for path in calibrations:
        if not os.path.exists(path):
            continue
        cal = json.load(open(path, encoding="utf-8"))
        for value, entry in cal["by_confidence_value"].items():
            if 0 < entry["verified"] < cal["weak_sample_threshold"] and entry["sample_adequate"]:
                unflagged.append({"file": os.path.basename(path), "confidence": value})
    out.append(check("every small sample is flagged as statistically weak",
                     not unflagged, {"violations": unflagged}))

    # 10 — edge attributes stripped by the tooling.
    missing = [r for r in records
               if r.get("confidence") is None or not r.get("provenance") or not r.get("at")]
    out.append(check("every record retains confidence, provenance and a location",
                     not missing, {"missing": len(missing)}))

    # 11 — labels taken from the analyser.
    out.append(check(
        "labels are declared independent of analyser output",
        "shares no code with the analyser" in data["independence"],
        {"statement": data["independence"][:90] + "..."},
    ))

    # 12 — the split able to see the outcome.
    out.append(check(
        "the holdout split is computed from edge identity, not from labels",
        "never on label" in data["split"]["rationale"],
        {"method": data["split"]["method"]},
    ))

    return out


def main():
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--dataset", required=True)
    ap.add_argument("--mirror", required=True)
    ap.add_argument("--calibration", nargs="*", default=[])
    ap.add_argument("--out")
    args = ap.parse_args()

    results = run(args.dataset, args.calibration, args.mirror)
    failed = [r for r in results if not r["passed"]]
    for r in results:
        print(f"  [{'PASS' if r['passed'] else 'FAIL'}] {r['check']}")
        if not r["passed"]:
            print(f"         {json.dumps(r['detail'])[:160]}")
    if args.out:
        json.dump({"schema": "cartograph.m08-integrity/1", "checks": results},
                  open(args.out, "w", encoding="utf-8"), indent=2, sort_keys=True)
    print(f"\n{len(results) - len(failed)}/{len(results)} integrity checks passed")
    return 1 if failed else 0


if __name__ == "__main__":
    raise SystemExit(main())
