#!/usr/bin/env python3
"""Enumerate ORM model declarations from source, independently of Cartograph.

Recognises more ORMs than Cartograph does — SQLModel, Peewee, Tortoise and
Prisma as well as SQLAlchemy and Django — because a model the analyser cannot
see must still be visible here. Whether a flavour is in scope is decided by
supported-subset.json, not by this script.
"""

from __future__ import annotations

import argparse
import json
import os
import re
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import enumerate_source as es  # noqa: E402

CLASS = re.compile(r"^\s*class\s+(\w+)\s*\(([^)]*)\)\s*:")
# Any class header at all, including one whose bases run over several lines.
# The strict pattern above needs `(...)` and the colon on one line, so
# `class SqlaTable(` did not register as a new class — and Superset's
# SqlaTable.__tablename__ = "tables" was recorded against the previous class,
# SqlMetric, forty lines earlier. Detecting the boundary and reading the bases
# are two different jobs.
CLASS_START = re.compile(r"^class\s+(\w+)\s*[\(:]")
TABLENAME = re.compile(r"""^\s*__tablename__\s*=\s*["']([^"']+)["']""")
TABLENAME_DYNAMIC = re.compile(r"^\s*__tablename__\s*=\s*(?!['\"])")
DB_TABLE = re.compile(r"""^\s*db_table\s*=\s*["']([^"']+)["']""")
DB_TABLE_DYNAMIC = re.compile(r"^\s*db_table\s*=\s*(?!['\"])")

FLAVOURS = [
    ("django", ("models.Model", "Model")),
    ("sqlalchemy", ("Base", "DeclarativeBase", "db.Model")),
    ("sqlmodel", ("SQLModel",)),
    ("peewee", ("peewee.Model",)),
    ("tortoise", ("tortoise.Model", "models.Model")),
]


def flavour_of(bases, decl):
    text = [b.strip() for b in bases.split(",")]
    if "table=True" in decl or any(b.startswith("table=") for b in text):
        if "SQLModel" in bases:
            return "sqlmodel"
    for name, markers in FLAVOURS:
        for marker in markers:
            if marker in text:
                return name
    return None


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("root")
    parser.add_argument("--sample", type=int, default=0)
    parser.add_argument("--summary", action="store_true")
    args = parser.parse_args()

    models = []
    for path in sorted(es.walk(args.root, {".py"})):
        try:
            with open(path, encoding="utf-8", errors="replace") as handle:
                lines = handle.readlines()
        except OSError:
            continue
        rel = os.path.relpath(path, args.root).replace(os.sep, "/")
        inert = es.inert_lines(lines)
        current = None
        for number, line in enumerate(lines, start=1):
            if number in inert:
                continue
            # A new top-level class ends the previous class's scope, whether
            # or not its bases can be read from this line.
            if CLASS_START.match(line) and not CLASS.match(line):
                current = None
                continue
            match = CLASS.match(line)
            if match:
                name, bases = match.group(1), match.group(2)
                flavour = flavour_of(bases, line)
                if flavour and not line.startswith(" "):
                    current = {
                        "model": name,
                        "flavour": flavour,
                        "bases": [b.strip() for b in bases.split(",") if b.strip()],
                        "file": rel,
                        "line": number,
                        "table": None,
                        "table_source": None,
                    }
                    models.append(current)
                elif not line.startswith(" "):
                    # A top-level class with an unrecognised base. Recorded so
                    # a model inheriting a project-local abstract base (GAP-3)
                    # is visible rather than invisible.
                    current = {
                        "model": name,
                        "flavour": "unrecognised-base",
                        "bases": [b.strip() for b in bases.split(",") if b.strip()],
                        "file": rel,
                        "line": number,
                        "table": None,
                        "table_source": None,
                    }
                continue
            if current is None:
                continue
            for pattern, source in (
                (TABLENAME, "__tablename__"),
                (DB_TABLE, "db_table"),
            ):
                found = pattern.match(line)
                if found:
                    current["table"] = found.group(1)
                    current["table_source"] = source
            for pattern, source in (
                (TABLENAME_DYNAMIC, "__tablename__"),
                (DB_TABLE_DYNAMIC, "db_table"),
            ):
                if current["table"] is None and pattern.match(line):
                    current["table_source"] = f"{source} (not a literal)"

    # `Model` alone is declared by Django and by Flask-SQLAlchemy /
    # Flask-AppBuilder alike, so the base name cannot decide the flavour. What
    # the class declares can: `__tablename__` is SQLAlchemy's, `Meta.db_table`
    # is Django's. Resolving it here keeps the ground truth accurate about
    # Superset and Airflow, whose models inherit a bare `Model`.
    for model in models:
        if "models.Model" in model["bases"]:
            model["flavour"] = "django"
        elif model["table_source"] and model["table_source"].startswith("__tablename__"):
            model["flavour"] = "sqlalchemy"
        elif model["table_source"] and model["table_source"].startswith("db_table"):
            model["flavour"] = "django"
        elif model["bases"] == ["Model"]:
            model["flavour"] = "ambiguous-bare-Model"

    declared = [m for m in models if m["flavour"] != "unrecognised-base"]
    if args.summary:
        counts = {}
        for m in declared:
            counts[m["flavour"]] = counts.get(m["flavour"], 0) + 1
        with_table = sum(1 for m in declared if m["table"])
        json.dump(
            {"models": len(declared), "by_flavour": counts, "with_literal_table": with_table},
            sys.stdout, indent=2, sort_keys=True,
        )
        print()
        return 0

    selected = es.stride_sample(declared, args.sample) if args.sample else declared
    json.dump({"total": len(declared), "selected": len(selected), "models": selected},
              sys.stdout, indent=1)
    print()
    return 0


if __name__ == "__main__":
    sys.exit(main())
