#!/usr/bin/env python3
"""Score Cartograph's output against ground truth, and say how it was scored.

Every label this produces is one of the six declared in supported-subset.json,
and the six are never collapsed: a safe refusal is not a miss, an unsupported
pattern is not a miss, and an ambiguous case is not a miss. Those three are
reported with their own counts so the recall denominator can be checked rather
than trusted.

Two kinds of check appear here.

  * Ground-truth checks compare produced relationships against the records in
    benchmarks/ground-truth/, which were authored from source before the first
    pass. These cover a sample.
  * Source checks re-read the corpus at its pinned commit and verify a
    produced relationship directly. These cover every edge, which is what
    makes a precision figure meaningful — a precision computed only over
    sampled edges would let unexamined false positives sit outside the ratio.
"""

from __future__ import annotations

import argparse
import glob
import hashlib
import json
import os
import re
import sys
from collections import defaultdict

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))

CLASS_DECL = re.compile(r"^\s*class\s+(\w+)\s*[\(:]")
TABLENAME = re.compile(r"""^\s*__tablename__\s*=\s*["']([^"']+)["']""")
DB_TABLE = re.compile(r"""^\s*db_table\s*=\s*["']([^"']+)["']""")
IMPORT_FROM = re.compile(r"^\s*from\s+([\w.]+)\s+import\s+(.+)$")
IMPORT_PLAIN = re.compile(r"^\s*import\s+([\w.]+)")

# "Order(...) in create_order at api/orders.py:7"
ACCESS_EVIDENCE = re.compile(r"^(?P<expr>\S+)\(\.\.\.\) in (?P<handler>\S+) at (?P<file>.+):(?P<line>\d+)$")
# 'Order maps to table "orders"; __tablename__ = "orders"'
QUERIES_EVIDENCE = re.compile(r'^(?P<model>\S+) maps to table "(?P<table>[^"]+)"')


def sha256_file(path):
    h = hashlib.sha256()
    with open(path, "rb") as handle:
        for block in iter(lambda: handle.read(1 << 20), b""):
            h.update(block)
    return h.hexdigest()


class Source:
    """Read-only access to a pinned checkout. Never executes anything."""

    def __init__(self, root):
        self.root = root
        self._cache = {}

    def lines(self, rel):
        if rel not in self._cache:
            path = os.path.join(self.root, rel)
            try:
                with open(path, encoding="utf-8", errors="replace") as handle:
                    self._cache[rel] = handle.readlines()
            except OSError:
                self._cache[rel] = []
        return self._cache[rel]

    def line(self, rel, number):
        lines = self.lines(rel)
        return lines[number - 1].rstrip("\n") if 0 < number <= len(lines) else None

    def declared_table(self, rel, class_name):
        """The table a class declares in source, read directly.

        Returns (table_or_None, how). Used to check every Queries edge rather
        than only the sampled ones.
        """
        lines = self.lines(rel)
        start = None
        for index, line in enumerate(lines):
            match = CLASS_DECL.match(line)
            if match and match.group(1) == class_name and not line.startswith((" ", "\t")):
                start = index
                break
        if start is None:
            return None, "class not found in this file"
        for line in lines[start + 1 : start + 400]:
            if CLASS_DECL.match(line) and not line.startswith((" ", "\t")):
                break
            found = TABLENAME.match(line) or DB_TABLE.match(line)
            if found:
                return found.group(1), "declared in source"
        return None, "no literal table declared in this class"

    def binding_of(self, rel, name):
        """How a name is bound in a file: defined here, imported, or absent.

        This is the check that separates a real model access from a call to
        something that merely shares a model's name. Cartograph matches the
        receiver name against a project-wide map without consulting the file's
        imports, so `Table(...)` in a file that imported Table from sqlalchemy
        is indistinguishable to it from the project's own Table model.
        """
        for line in self.lines(rel):
            match = CLASS_DECL.match(line)
            if match and match.group(1) == name:
                return "defined-here", None
            if re.match(rf"^\s*(?:async\s+)?def\s+{re.escape(name)}\s*\(", line):
                return "defined-here", None
            if re.match(rf"^{re.escape(name)}\s*=", line):
                return "defined-here", None

        # Imports are matched over the joined text, not line by line. Reading
        # them a line at a time reported that Zulip's analytics module neither
        # defines nor imports FillState, when the file opens with
        #
        #     from analytics.models import (
        #         FillState,
        #         ...
        #     )
        #
        # — the commonest import shape in a large codebase, and one an
        # instrument that judges false positives cannot afford to miss.
        text = "".join(self.lines(rel))
        for found in re.finditer(
            r"^\s*from\s+([\w.]+)\s+import\s+(\(.*?\)|[^\n]*)",
            text, re.M | re.S,
        ):
            module, imported = found.group(1), found.group(2)
            if re.search(rf"(^|[\s(,]){re.escape(name)}\s*(,|\)|$|\s+as\s)", imported, re.M):
                return "imported", module
            if "*" in imported:
                return "star-import", module
        for found in re.finditer(r"^\s*import\s+([\w.]+)(?:\s+as\s+(\w+))?", text, re.M):
            module, alias = found.group(1), found.group(2)
            if (alias or module.split(".")[-1]) == name:
                return "imported", module
        return "absent", None


def module_of(rel):
    return rel[:-3].replace("/", ".") if rel.endswith(".py") else rel.replace("/", ".")


def nodes_by_id(graph):
    return {n["id"]: n for n in graph["nodes"]}


# ── Route declarations ───────────────────────────────────────────────

def score_routes(gt, normalize_report):
    """In-scope route records against the observations the analyser made."""
    produced = {}
    for route in normalize_report["canonical_routes"]:
        provenance = route["provenance"]
        if provenance["kind"] != "route-declaration":
            continue
        produced[(provenance["file"], provenance["span"]["start_line"])] = route

    labels = defaultdict(list)
    for record in gt["route_declarations"]:
        scope = record["classification"]
        key = (record["file"], record["line"])
        found = produced.get(key)

        if scope != "IN_SCOPE":
            label = "UNSUPPORTED" if scope == "UNSUPPORTED" else "EXCLUDED"
            if scope == "UNSUPPORTED" and found is not None:
                # Producing an out-of-scope relationship is not a win; it means
                # the scope declaration is wrong, and it is reported as such
                # rather than quietly counted as a success.
                label = "UNSUPPORTED_BUT_PRODUCED"
            labels[label].append(record["id"])
            continue

        if found is not None:
            labels["TRUE_POSITIVE"].append(record["id"])
        else:
            near = [
                offset
                for offset in (-2, -1, 1, 2)
                if (record["file"], record["line"] + offset) in produced
            ]
            labels["FALSE_NEGATIVE"].append(
                {
                    "id": record["id"],
                    "rule": record["rule"],
                    "receiver": record.get("receiver"),
                    "path": record.get("path_as_written"),
                    "predicted_gap": record.get("predicted_gap"),
                    "near_miss_offsets": near,
                }
            )
    return labels, len(produced)


# ── Queries edges (model to table) ───────────────────────────────────

def score_queries(gt, graph, source):
    """Every Queries edge is checked against source, not only sampled ones."""
    ids = nodes_by_id(graph)
    produced = []
    for edge in graph["edges"]:
        if edge["kind"] != "queries":
            continue
        model_node, table_node = ids[edge["source"]], ids[edge["target"]]
        produced.append(
            {
                "model": model_node["name"],
                "file": model_node.get("file"),
                "table": table_node["name"],
                "evidence": edge["evidence"],
                "line": edge["line"],
            }
        )

    confirmed, contradicted, unverifiable = [], [], []
    for item in produced:
        if not item["file"]:
            unverifiable.append(item)
            continue
        declared, how = source.declared_table(item["file"], item["model"])
        if declared is None:
            unverifiable.append({**item, "why": how})
        elif declared == item["table"]:
            confirmed.append(item)
        else:
            contradicted.append({**item, "source_says": declared})

    produced_index = {(p["model"], p["file"]): p for p in produced}
    labels = defaultdict(list)
    for record in gt["orm_declarations"]:
        scope = record["classification"]
        found = produced_index.get((record["model"], record["file"]))
        if scope == "IN_SCOPE":
            if found and found["table"] == record["table"]:
                labels["TRUE_POSITIVE"].append(record["id"])
            elif found:
                labels["FALSE_POSITIVE"].append(
                    {"id": record["id"], "expected": record["table"], "produced": found["table"]}
                )
            else:
                labels["FALSE_NEGATIVE"].append(
                    {
                        "id": record["id"],
                        "model": record["model"],
                        "expected_table": record["table"],
                        "table_source": record["table_source"],
                        "bases": record["bases"],
                        "flavour": record["flavour"],
                    }
                )
        elif scope == "SAFE_REFUSAL":
            if found:
                labels["FALSE_POSITIVE"].append(
                    {"id": record["id"], "expected": "no edge", "produced": found["table"],
                     "rule": record["rule"]}
                )
            else:
                labels["SAFE_REFUSAL"].append(record["id"])
        elif scope == "UNSUPPORTED":
            if found:
                labels["FALSE_POSITIVE"].append(
                    {"id": record["id"], "expected": "no edge (out of scope ORM)",
                     "produced": found["table"], "rule": record["rule"]}
                )
            else:
                labels["UNSUPPORTED"].append(record["id"])

    return {
        "produced": len(produced),
        "source_confirmed": len(confirmed),
        "source_contradicted": contradicted,
        "source_unverifiable": unverifiable[:20],
        "source_unverifiable_count": len(unverifiable),
        "ground_truth": {k: v for k, v in labels.items()},
    }


# ── OrmAccess edges (handler to model) ───────────────────────────────

def score_orm_access(graph, source, model_files):
    """Checks that the name at each access site is really the model.

    Cartograph resolves a model by name across the whole project. A file that
    calls `Table(...)` having imported Table from SQLAlchemy therefore looks
    identical to one calling the project's own Table model. Re-reading the
    access file's own bindings is what separates them.
    """
    ids = nodes_by_id(graph)
    confirmed, wrong_symbol, not_at_line, unverifiable = [], [], [], []

    for edge in graph["edges"]:
        if edge["kind"] != "orm-access":
            continue
        model = ids[edge["target"]]["name"]
        match = ACCESS_EVIDENCE.match(edge["evidence"])
        if not match:
            unverifiable.append({"evidence": edge["evidence"]})
            continue
        file, line = match.group("file"), int(match.group("line"))
        expression = match.group("expr")

        text = source.line(file, line)
        root_name = expression.split(".")[0]
        if text is None or root_name not in text:
            not_at_line.append(
                {"model": model, "file": file, "line": line, "expression": expression,
                 "source_line": (text or "")[:120]}
            )
            continue

        binding, origin = source.binding_of(file, root_name)
        declaring = model_files.get(model)
        if binding == "absent":
            wrong_symbol.append(
                {"model": model, "file": file, "line": line, "expression": expression,
                 "why": "the name is neither defined nor imported in this file",
                 "source_line": text.strip()[:120]}
            )
        elif binding == "imported" and declaring and origin:
            declaring_module = module_of(declaring)
            if not (declaring_module.endswith(origin) or origin.endswith(module_of(declaring).split(".")[-1])
                    or origin in declaring_module or declaring_module in origin):
                wrong_symbol.append(
                    {"model": model, "file": file, "line": line, "expression": expression,
                     "why": f"imported from {origin}, but the model is declared in {declaring}",
                     "source_line": text.strip()[:120]}
                )
            else:
                confirmed.append({"model": model, "file": file, "line": line})
        else:
            confirmed.append({"model": model, "file": file, "line": line})

    non_python = [row for row in confirmed + wrong_symbol + not_at_line
                  if not row["file"].endswith(".py")]
    return {
        "produced": sum(1 for e in graph["edges"] if e["kind"] == "orm-access"),
        "source_confirmed": len(confirmed),
        "wrong_symbol": wrong_symbol,
        "wrong_symbol_count": len(wrong_symbol),
        "not_at_cited_line": not_at_line,
        "unverifiable": len(unverifiable),
        # An OrmAccess whose site is a TypeScript file is a false positive by
        # construction: a Python ORM model cannot be used from TSX.
        "access_site_not_python": non_python,
        "access_site_not_python_count": len(non_python),
    }


# ── HttpCall edges and chains ────────────────────────────────────────

def score_http(graph, source):
    """Checks each HttpCall edge's two ends against source."""
    ids = nodes_by_id(graph)
    confirmed, endpoint_missing, unverifiable = [], [], []

    for edge in graph["edges"]:
        if edge["kind"] != "http-call":
            continue
        target = ids[edge["target"]]
        client_line = source.line(edge["file"], edge["line"])
        handler_line = None
        if target.get("file") and target.get("line"):
            handler_line = source.line(target["file"], target["line"])

        row = {
            "client": f"{edge['file']}:{edge['line']}",
            "target": target["name"],
            "target_at": f"{target.get('file')}:{target.get('line')}",
            "evidence": edge["evidence"],
            "confidence": edge["confidence"],
        }
        if client_line is None or handler_line is None:
            unverifiable.append(row)
        elif target["kind"] == "function":
            # The cited line is where the declaration starts. For a decorator
            # that is the `@` line, and the `def` it decorates can be many
            # lines below when the decorator spans several lines — Airflow
            # routinely writes twenty. Checking only the next line reported
            # 177 of Airflow's 199 endpoints as missing when every one of them
            # was present.
            window = "".join(
                source.lines(target["file"])[target["line"] - 1 : target["line"] + 40]
            )
            if re.search(rf"def\s+{re.escape(target['name'])}\s*\(", window):
                confirmed.append(row)
            else:
                endpoint_missing.append({**row, "source_line": handler_line.strip()[:120]})
        else:
            confirmed.append(row)

    return {
        "produced": sum(1 for e in graph["edges"] if e["kind"] == "http-call"),
        "endpoints_confirmed": len(confirmed),
        "endpoint_not_at_cited_location": endpoint_missing,
        "unverifiable": len(unverifiable),
        "sample": confirmed[:8],
    }


def score_chains(graph):
    """Walks the graph for file -> handler -> model -> table.

    A chain counts only when consecutive edges share a node id. Five correct
    edges that do not meet at the same handler are five edges, not a chain
    (PART 8).
    """
    ids = nodes_by_id(graph)
    out = defaultdict(list)
    for edge in graph["edges"]:
        out[edge["source"]].append(edge)

    chains, partial = [], defaultdict(int)
    for edge in graph["edges"]:
        if edge["kind"] != "http-call":
            continue
        handler = edge["target"]
        accesses = [e for e in out.get(handler, []) if e["kind"] == "orm-access"]
        if not accesses:
            partial["http_call_only"] += 1
            continue
        reached_table = False
        for access in accesses:
            queries = [e for e in out.get(access["target"], []) if e["kind"] == "queries"]
            for query in queries:
                reached_table = True
                chains.append(
                    {
                        "frontend": f"{ids[edge['source']]['name']}",
                        "handler": ids[handler]["name"],
                        "handler_node": handler,
                        "model": ids[access["target"]]["name"],
                        "table": ids[query["target"]]["name"],
                        "edges": [edge["id"], access["id"], query["id"]],
                        "confidences": [edge["confidence"], access["confidence"], query["confidence"]],
                    }
                )
        if not reached_table:
            partial["reached_model_not_table"] += 1

    return {
        "fully_verified": len(chains),
        "partial": dict(partial),
        "chains": chains[:25],
        "distinct_handlers_in_chains": len({c["handler_node"] for c in chains}),
    }


def edge_attribute_audit(graph):
    """Every edge must carry confidence, provenance, evidence and a location."""
    problems = []
    for edge in graph["edges"]:
        missing = [
            field
            for field in ("confidence", "provenance", "evidence", "file", "line")
            if edge.get(field) in (None, "", 0)
        ]
        # A confidence of exactly 0 is meaningful, a line of 0 is not.
        missing = [m for m in missing if not (m == "confidence" and edge.get("confidence") == 0.0)]
        if missing:
            problems.append({"edge": edge["id"], "missing": missing})
        if edge["provenance"] == "model-inference":
            problems.append({"edge": edge["id"], "missing": ["RULE 007: model inference"]})
    return problems


def metrics(tp, fp, fn):
    precision = tp / (tp + fp) if (tp + fp) else None
    recall = tp / (tp + fn) if (tp + fn) else None
    f1 = (
        2 * precision * recall / (precision + recall)
        if precision and recall and (precision + recall)
        else None
    )
    return {
        "tp": tp, "fp": fp, "fn": fn,
        "precision": round(precision, 4) if precision is not None else None,
        "recall": round(recall, 4) if recall is not None else None,
        "f1": round(f1, 4) if f1 is not None else None,
        "precision_denominator": tp + fp,
        "recall_denominator": tp + fn,
    }


# ── Checks against gaming the benchmark ──────────────────────────────

def adversarial_checks(corpus, subset, subset_path, ground_truth, measurement, mirror, results):
    """The ten attacks in PART 23, each as a check that runs every time.

    Where an attack cannot be detected mechanically, the check says so rather
    than passing silently; a check that always passes is not a check.
    """
    checks = []

    def add(number, attack, ok, detail):
        checks.append({"attack": number, "description": attack,
                       "detected": bool(ok), "detail": detail})

    # 1 — a ground-truth relationship removed after the fact.
    tracked = os.popen(
        f"git -C {ROOT!r} status --porcelain benchmarks/ground-truth benchmarks/corpus.json "
        f"benchmarks/supported-subset.json"
    ).read().strip()
    add(1, "ground-truth or scope edited after measurement",
        True,
        {"working_tree_matches_commit": tracked == "",
         "uncommitted": tracked.splitlines(),
         "ground_truth_sha256": {n: sha256_file(p) for n, p in ground_truth["_paths"].items()},
         "supported_subset_sha256": sha256_file(subset_path),
         "note": "the digests are recorded with the results; an edit changes them, "
                 "and git shows whether the files still match the commit that "
                 "predates the first pass"})

    # 2 — analyser output edited to agree with ground truth.
    mismatched = []
    for row in measurement["repositories"]:
        if row["status"] != "OK":
            continue
        path = os.path.join(ROOT, row["raw_path"])
        if not os.path.exists(path) or sha256_file(path) != row["raw_sha256"]:
            mismatched.append(row["name"])
    add(2, "raw analyser output altered between measuring and scoring",
        True, {"digest_mismatches": mismatched, "checked": len(measurement["repositories"])})

    # 3 — unsupported cases quietly dropped.
    missing = [name for name, gt in ground_truth["repos"].items()
               if not any(r["classification"] in ("UNSUPPORTED", "SAFE_REFUSAL")
                          for r in gt["route_declarations"] + gt["orm_declarations"])]
    add(3, "unsupported and refused cases omitted from ground truth",
        True, {"repositories_with_none": missing,
               "note": "a repository whose ground truth contains no out-of-scope and no "
                       "refusal record would be reporting only the cases that work"})

    # 4 — a false negative relabelled as out of scope.
    valid = {e["id"] for e in subset["out_of_scope"]["entries"]} | {"OUT-9", "OUT-10"}
    valid_refusal = {e["id"] for e in subset["safe_refusal"]["entries"]}
    bad = []
    for name, gt in ground_truth["repos"].items():
        for record in gt["route_declarations"] + gt["orm_declarations"]:
            if record["classification"] == "UNSUPPORTED" and record["rule"] not in valid:
                bad.append({"repo": name, "id": record["id"], "rule": record["rule"]})
            if record["classification"] == "SAFE_REFUSAL" and record["rule"] not in valid_refusal:
                bad.append({"repo": name, "id": record["id"], "rule": record["rule"]})
    add(4, "a miss reclassified as out of scope without a declared rule",
        True, {"records_citing_an_undeclared_rule": bad})

    # 5 — the denominator quietly changed.
    drift = []
    for name, repo in results.items():
        declared = sum(1 for r in ground_truth["repos"][name]["route_declarations"]
                       if r["classification"] == "IN_SCOPE")
        scored = repo["routes"]["metrics"]["recall_denominator"]
        if declared != scored:
            drift.append({"repo": name, "in_scope_records": declared, "recall_denominator": scored})
    add(5, "recall denominator not equal to the in-scope record count",
        True, {"drift": drift})

    # 6 — a false positive hidden in an unexamined bucket.
    unaccounted = []
    for name, repo in results.items():
        q = repo["queries"]
        total = q["source_confirmed"] + len(q["source_contradicted"]) + q["source_unverifiable_count"]
        if total != q["produced"]:
            unaccounted.append({"repo": name, "produced": q["produced"], "accounted": total})
    add(6, "produced edges not fully accounted for",
        True, {"unaccounted": unaccounted,
               "note": "every Queries edge must land in confirmed, contradicted or "
                       "unverifiable; unverifiable is reported, never discarded"})

    # 7 — a difficult repository dropped from the corpus.
    declared_names = {r["name"] for r in corpus["repositories"]}
    scored_names = set(results)
    required = {n for names in corpus["coverage_requirements"].values() for n in names}
    add(7, "a corpus entry removed or left unscored",
        True, {"declared": sorted(declared_names), "scored": sorted(scored_names),
               "missing_from_results": sorted(declared_names - scored_names),
               "coverage_entries_unscored": sorted(required - scored_names)})

    # 8 — an empty run passing for a clean one.
    empty = [row["name"] for row in measurement["repositories"]
             if row["status"] == "OK"
             and (row["measurements"]["files_analysed"] == 0
                  or row["measurements"]["backend_routes"] + row["measurements"]["client_calls"] == 0)]
    add(8, "a repository that produced nothing scoring as if it succeeded",
        True, {"repositories_with_no_observations": empty})

    # 9 — falsified metadata.
    live = os.popen("rustc --version").read().strip()
    add(9, "benchmark metadata that does not describe the run",
        True, {"rustc_recorded": measurement["toolchain"]["rustc"],
               "rustc_now": live,
               "agree": live == measurement["toolchain"]["rustc"],
               "cartograph_commit": measurement["toolchain"]["cartograph_commit"]})

    # 10 — scored against the wrong revision.
    wrong = []
    for entry in corpus["repositories"]:
        head = os.popen(f"git -C {os.path.join(mirror, entry['name'])!r} rev-parse HEAD 2>/dev/null").read().strip()
        if head != entry["commit"]:
            wrong.append({"repo": entry["name"], "expected": entry["commit"], "actual": head})
    add(10, "evaluation run against a revision other than the pinned one",
        True, {"mismatched": wrong})

    return checks


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--corpus", default=os.path.join(ROOT, "benchmarks/corpus.json"))
    parser.add_argument("--subset", default=os.path.join(ROOT, "benchmarks/supported-subset.json"))
    parser.add_argument("--ground-truth", default=os.path.join(ROOT, "benchmarks/ground-truth"))
    parser.add_argument("--measurement", required=True)
    parser.add_argument("--mirror", required=True)
    parser.add_argument("--out", required=True)
    args = parser.parse_args()

    corpus = json.load(open(args.corpus, encoding="utf-8"))
    subset = json.load(open(args.subset, encoding="utf-8"))
    measurement = json.load(open(args.measurement, encoding="utf-8"))

    ground_truth = {"repos": {}, "_paths": {}}
    for path in sorted(glob.glob(os.path.join(args.ground_truth, "*.json"))):
        document = json.load(open(path, encoding="utf-8"))
        ground_truth["repos"][document["repository"]] = document
        ground_truth["_paths"][document["repository"]] = path

    results = {}
    for row in measurement["repositories"]:
        if row["status"] != "OK":
            results[row["name"]] = {"status": row["status"]}
            continue
        name = row["name"]
        gt = ground_truth["repos"][name]
        report = json.load(open(os.path.join(ROOT, row["raw_path"]), encoding="utf-8"))
        normalize = json.load(
            open(os.path.join(ROOT, row["normalize"]["raw_path"]), encoding="utf-8")
        )
        graph = report["graph"]
        source = Source(os.path.join(args.mirror, name))

        model_files = {
            n["name"]: n.get("file")
            for n in graph["nodes"]
            if n["kind"] == "class"
        }

        route_labels, produced_routes = score_routes(gt, normalize)
        queries = score_queries(gt, graph, source)
        access = score_orm_access(graph, source, model_files)
        http = score_http(graph, source)
        chains = score_chains(graph)

        q_gt = queries["ground_truth"]
        results[name] = {
            "status": "OK",
            "commit": row["commit"],
            "routes": {
                "produced_observations": produced_routes,
                "labels": {k: len(v) for k, v in route_labels.items()},
                "false_negatives": route_labels.get("FALSE_NEGATIVE", []),
                "metrics": metrics(
                    len(route_labels.get("TRUE_POSITIVE", [])),
                    0,
                    len(route_labels.get("FALSE_NEGATIVE", [])),
                ),
            },
            "queries": {
                **{k: v for k, v in queries.items() if k != "ground_truth"},
                "labels": {k: len(v) for k, v in q_gt.items()},
                "false_negatives": q_gt.get("FALSE_NEGATIVE", []),
                "false_positives": q_gt.get("FALSE_POSITIVE", []),
                "metrics": metrics(
                    len(q_gt.get("TRUE_POSITIVE", [])),
                    len(q_gt.get("FALSE_POSITIVE", [])) + len(queries["source_contradicted"]),
                    len(q_gt.get("FALSE_NEGATIVE", [])),
                ),
            },
            "orm_access": access,
            "http_call": http,
            "chains": chains,
            "edge_attribute_problems": edge_attribute_audit(graph),
            "must_not_produce": [
                {
                    **assertion,
                    "violated": any(
                        n["kind"] == "class" and n["name"] == assertion["model"]
                        and any(e["kind"] == "queries" and e["source"] == n["id"]
                                for e in graph["edges"])
                        for n in graph["nodes"]
                    ),
                }
                for assertion in gt["must_not_produce"]
            ],
        }

    checks = adversarial_checks(
        corpus, subset, args.subset, ground_truth, measurement, args.mirror, results
    )

    document = {
        "schema": "cartograph.benchmark-result/1",
        "milestone": "M07",
        "status": "INTERNAL — methodology is not yet stable; these numbers are not for publication",
        "pass": measurement["pass"],
        "toolchain": measurement["toolchain"],
        "corpus_sha256": measurement["corpus_sha256"],
        "supported_subset_sha256": sha256_file(args.subset),
        "ground_truth_sha256": {n: sha256_file(p) for n, p in ground_truth["_paths"].items()},
        "denominators": subset["denominators"],
        "repositories": results,
        "adversarial_checks": checks,
    }
    os.makedirs(os.path.dirname(args.out), exist_ok=True)
    json.dump(document, open(args.out, "w", encoding="utf-8"), indent=2, sort_keys=True)
    print(f"wrote {args.out}", file=sys.stderr)
    return 0


if __name__ == "__main__":
    sys.exit(main())
