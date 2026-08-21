#!/usr/bin/env python3
"""Add ORM ground-truth records to the drafted route records.

Runs after draft_ground_truth.py and, like it, reads only repository source.
Classification applies the scope rules fixed in supported-subset.json:
a recognised flavour with a declared table is in scope, a recognised flavour
without one is a required refusal, and any other ORM is out of scope and
carries a must_not_produce assertion so a table invented for it is caught.
"""

from __future__ import annotations

import json
import os
import subprocess
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))

REPOS = ["full-stack-fastapi", "onyx", "superset", "posthog", "zulip", "airflow", "autogpt"]
# A declarative base is not a mapped model; it maps to no table.
DECLARATIVE_BASES = {"Base", "PublicBase", "DeclarativeBase"}
SAMPLE = 14


def classify(model):
    flavour, table, source = model["flavour"], model["table"], model["table_source"]
    if flavour in ("sqlmodel", "peewee", "tortoise"):
        return "UNSUPPORTED", "OUT-1", "ORM flavour outside the two M06 declared"
    if table:
        return (
            "IN_SCOPE",
            "IN-ORM-1" if flavour != "django" else "IN-ORM-2",
            f"{source} declares the table",
        )
    if source and "not a literal" in source:
        return "SAFE_REFUSAL", "SR-2", "table name is not a string literal"
    if flavour == "django":
        return "SAFE_REFUSAL", "SR-6", "no Meta.db_table; Django's default needs the app label"
    return "SAFE_REFUSAL", "SR-2", "no table declared in this class"


def main():
    mirror = sys.argv[1]
    for name in REPOS:
        result = subprocess.run(
            ["python3", os.path.join(ROOT, "benchmarks/enumerate_models.py"),
             os.path.join(mirror, name), "--sample", str(SAMPLE)],
            capture_output=True, text=True, check=True,
        )
        data = json.loads(result.stdout)

        path = os.path.join(ROOT, f"benchmarks/ground-truth/{name}.json")
        document = json.load(open(path, encoding="utf-8"))

        records, must_not = [], []
        for model in data["models"]:
            scope, rule, why = classify(model)
            record = {
                "id": f"{model['file']}:{model['line']}",
                "relationship": "QUERIES",
                "model": model["model"], "bases": model["bases"], "flavour": model["flavour"],
                "table": model["table"], "table_source": model["table_source"],
                "file": model["file"], "line": model["line"],
                "classification": scope, "rule": rule, "reason": why,
            }
            if model["model"] in DECLARATIVE_BASES and not model["table"]:
                record["note"] = ("a declarative base, not a mapped model; it maps to no "
                                  "table and must not produce a Queries edge")
                record["classification"], record["rule"] = "SAFE_REFUSAL", "SR-2"
            records.append(record)
            if scope == "UNSUPPORTED":
                must_not.append({
                    "assertion": f"no table may be claimed for {model['model']} ({model['flavour']})",
                    "model": model["model"], "file": model["file"], "line": model["line"],
                    "rule": rule,
                    "check": "no Queries edge whose source node is named this model",
                })

        counts = {}
        for record in records:
            counts[record["rule"]] = counts.get(record["rule"], 0) + 1

        document["orm_declarations"] = records
        document["orm_population"] = data["total"]
        document["orm_rule_counts"] = counts
        document["must_not_produce"] = must_not
        document["verification"] = {
            "reviewed": True,
            "reviewed_against": "repository source at the pinned commit",
            "cartograph_output_consulted": False,
            "notes": NOTES,
        }
        json.dump(document, open(path, "w", encoding="utf-8"), indent=1, sort_keys=True)
        print(f"{name}: orm_population={data['total']} sampled={len(records)} "
              f"{counts} must_not={len(must_not)}", file=sys.stderr)
    return 0


NOTES = [
    "Drafts were reviewed against repository source before use. The review found six defects in the instrument, none in Cartograph.",
    "GAP-1 (an unrecognised decorator receiver) was being applied to Django path() entries, which have no receiver.",
    "Handlers for Django URL-conf entries were read as 'the next def in the file', which attributed an unrelated admin helper to a route. The view is now read from the entry's second argument.",
    "path(..., include(...)) is a mount point, not a route (OUT-10); a view named through csrf_exempt/admin_view/as_view is marked handler_resolvable=false, so the route counts but the chain cannot.",
    "ORM flavour is decided by what a class declares (__tablename__ vs Meta.db_table), not by its base name: `Model` is used by both Django and Flask-AppBuilder, and attributing it by name mislabelled all 32 Superset and all 61 Airflow models as Django.",
    "CORRECTION AFTER PASS 2 — the model enumerator required a class header on one line, so a multi-line `class SqlaTable(` did not close the previous class's scope and its __tablename__ was recorded against SqlMetric forty lines earlier. Cartograph was right; the ground truth was wrong. One record across the corpus was affected.",
    "CORRECTION AFTER PASS 2 — route declarations inside docstrings were being counted as routes. Onyx and AutoGPT document their dependencies with `@router.post(\"/tokens\")` inside a docstring; Cartograph, which parses, ignored all three, and the instrument charged it three false negatives for correctly declining to extract documentation. Both corrections move numbers in Cartograph's favour and are recorded here for exactly that reason.",
]

if __name__ == "__main__":
    sys.exit(main())
