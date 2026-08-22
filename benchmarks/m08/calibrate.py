#!/usr/bin/env python3
"""Calibration analysis over the M08 labelled dataset.

Reports what the confidence numbers are actually worth, with the denominator
and sample size attached to every figure, and refuses to dress a small sample
as an estimate.

Standard library only — `math` is enough for a Wilson interval, and a plotting
dependency would buy nothing a table does not already say.
"""

from __future__ import annotations

import argparse
import json
import math
import os
from collections import defaultdict

TP, FP, UNVERIFIABLE = "TRUE_POSITIVE", "FALSE_POSITIVE", "UNVERIFIABLE"

# Below this many verified observations a proportion is reported but flagged;
# the Wilson interval on fewer is wider than any claim worth making.
WEAK_SAMPLE = 30


def wilson(successes, total, z=1.96):
    """Wilson score interval — behaves at the extremes, unlike normal approx.

    A normal-approximation interval on 290/290 is [1.0, 1.0], which asserts
    certainty no sample of 290 can support.
    """
    if total == 0:
        return None, None
    phat = successes / total
    denom = 1 + z * z / total
    centre = (phat + z * z / (2 * total)) / denom
    margin = z * math.sqrt(phat * (1 - phat) / total + z * z / (4 * total * total)) / denom
    return max(0.0, centre - margin), min(1.0, centre + margin)


def summarise(records):
    """Verified counts for a group of records."""
    tp = sum(1 for r in records if r["label"] == TP)
    fp = sum(1 for r in records if r["label"] == FP)
    un = sum(1 for r in records if r["label"] == UNVERIFIABLE)
    verified = tp + fp
    accuracy = tp / verified if verified else None
    low, high = wilson(tp, verified) if verified else (None, None)
    return {
        "produced": len(records),
        "verified": verified,
        "unverifiable": un,
        "unverifiable_share": round(un / len(records), 4) if records else None,
        "true_positive": tp,
        "false_positive": fp,
        "observed_accuracy": round(accuracy, 4) if accuracy is not None else None,
        "ci95": [round(low, 4), round(high, 4)] if low is not None else None,
        "sample_adequate": verified >= WEAK_SAMPLE,
    }


def by_confidence(records):
    """Observed accuracy at each distinct confidence value.

    Cartograph emits a small set of discrete priors, so this is the natural
    grouping; ten equal-width bins would leave six of them empty and hide the
    fact that one value carries most of the corpus.
    """
    groups = defaultdict(list)
    for r in records:
        groups[r["confidence"]].append(r)
    return {str(c): summarise(rs) for c, rs in sorted(groups.items())}


def reliability_bins(records, width=0.1):
    """The conventional equal-width view, reported for comparability."""
    groups = defaultdict(list)
    for r in records:
        idx = min(int(r["confidence"] / width), int(1 / width) - 1)
        groups[idx].append(r)
    out = {}
    for idx in range(int(1 / width)):
        lo, hi = idx * width, (idx + 1) * width
        rs = groups.get(idx, [])
        entry = summarise(rs)
        entry["mean_confidence"] = (
            round(sum(r["confidence"] for r in rs) / len(rs), 4) if rs else None
        )
        out[f"{lo:.1f}-{hi:.1f}"] = entry
    return out


def calibration_error(records):
    """ECE and MCE over verified observations, weighted by bin population.

    Only verified observations can contribute: an edge whose correctness is
    unknown cannot testify for or against a confidence value. The share that
    had to be excluded is reported beside the number, because a calibration
    error computed on a filtered subset is only as representative as that
    filter.
    """
    verified = [r for r in records if r["label"] in (TP, FP)]
    if not verified:
        return {"ece": None, "mce": None, "verified": 0, "excluded_unverifiable": len(records)}
    groups = defaultdict(list)
    for r in verified:
        groups[r["confidence"]].append(r)
    total = len(verified)
    ece = 0.0
    mce = 0.0
    terms = []
    for conf, rs in sorted(groups.items()):
        acc = sum(1 for r in rs if r["label"] == TP) / len(rs)
        gap = abs(acc - conf)
        ece += (len(rs) / total) * gap
        mce = max(mce, gap)
        terms.append({"confidence": conf, "observed": round(acc, 4),
                      "gap": round(gap, 4), "n": len(rs),
                      "sample_adequate": len(rs) >= WEAK_SAMPLE})
    return {
        "ece": round(ece, 4),
        "mce": round(mce, 4),
        "verified": total,
        "excluded_unverifiable": len(records) - total,
        "excluded_share": round((len(records) - total) / len(records), 4),
        "terms": terms,
    }


def thresholds(records, steps=None):
    """Precision, recall and coverage as an acceptance threshold moves.

    Recall here is recall *among produced edges* — the share of correct edges
    retained. It is not recall against an independent denominator; that is
    measured separately and must not be confused with this.
    """
    steps = steps or [0.0, 0.6, 0.65, 0.7, 0.75, 0.784, 0.8, 0.85, 0.9, 0.95, 0.98, 0.99]
    verified = [r for r in records if r["label"] in (TP, FP)]
    all_tp = sum(1 for r in verified if r["label"] == TP)
    rows = []
    for t in steps:
        kept = [r for r in verified if r["confidence"] >= t]
        tp = sum(1 for r in kept if r["label"] == TP)
        fp = sum(1 for r in kept if r["label"] == FP)
        rows.append({
            "threshold": t,
            "kept": len(kept),
            "coverage_of_verified": round(len(kept) / len(verified), 4) if verified else None,
            "precision": round(tp / (tp + fp), 4) if tp + fp else None,
            "retained_true_positives": round(tp / all_tp, 4) if all_tp else None,
            "false_positives_admitted": fp,
        })
    return rows


def main():
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--dataset", required=True)
    ap.add_argument("--split", default="development",
                    choices=["development", "holdout", "all"])
    ap.add_argument("--out", required=True)
    args = ap.parse_args()

    data = json.load(open(args.dataset, encoding="utf-8"))
    records = data["records"]
    if args.split != "all":
        records = [r for r in records if r["split"] == args.split]

    by_class = defaultdict(list)
    by_prov = defaultdict(list)
    for r in records:
        by_class[r["kind"]].append(r)
        by_prov[r["provenance"]].append(r)

    document = {
        "schema": "cartograph.m08-calibration/1",
        "milestone": "M08",
        "status": "INTERNAL",
        "split": args.split,
        "bound_to": data["bound_to"],
        "records_considered": len(records),
        "weak_sample_threshold": WEAK_SAMPLE,
        "overall": summarise(records),
        "calibration_error": calibration_error(records),
        "by_confidence_value": by_confidence(records),
        "reliability_bins": reliability_bins(records),
        "by_relationship_class": {k: summarise(v) for k, v in sorted(by_class.items())},
        "by_relationship_class_calibration": {
            k: calibration_error(v) for k, v in sorted(by_class.items())
        },
        "by_provenance": {k: summarise(v) for k, v in sorted(by_prov.items())},
        "by_provenance_calibration": {
            k: calibration_error(v) for k, v in sorted(by_prov.items())
        },
        "thresholds": thresholds(records),
    }
    os.makedirs(os.path.dirname(args.out), exist_ok=True)
    json.dump(document, open(args.out, "w", encoding="utf-8"), indent=2, sort_keys=True)
    print(f"wrote {args.out}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
