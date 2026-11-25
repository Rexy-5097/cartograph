"""Integration check: the browser API against the CLI's own output.

EXPERIMENTAL (M18). Run against a live `cartograph-serve` and the JSON that
`cartograph <repo> --json` printed for the same repository at the same commit:

    python -B verify_api.py http://127.0.0.1:PORT TOKEN path/to/summary.json

Checks, each printed PASS or FAIL:

1. Counts: /api/report totals and /api/analysis summary equal the CLI JSON.
2. No invented relationship: every edge the browser draws (scene) is an edge
   of the CLI's graph with the same id, endpoints and kind, and none is
   missing.
3. Node identity: every scene node is a CLI node with the same id, kind and
   name.
4. Evidence: a sample of edges' evidence equals the CLI edge's evidence,
   provenance, confidence, file and line.
5. Stale selection: a lookup with an earlier analysis id is refused (409).
6. Invalid requests: missing parameters (400), unknown edge/node (404), wrong
   method (405), unknown endpoint (404), no token (401), foreign Host (421),
   foreign Origin (403), source outside the analysis (404).
7. Latency: median and p95 of evidence and node lookups.

Uses only the standard library; it reads the repository through the API,
never directly.
"""

import http.client
import json
import statistics
import sys
import time
import urllib.parse


def main() -> int:
    base, token, cli_path = sys.argv[1], sys.argv[2], sys.argv[3]
    url = urllib.parse.urlparse(base)
    host, port = url.hostname, url.port
    cli = json.load(open(cli_path, encoding="utf-8"))
    failures = 0

    def request(method, path, headers=None, token_header=True):
        conn = http.client.HTTPConnection(host, port, timeout=60)
        h = {"Host": f"{host}:{port}"}
        if token_header:
            h["X-Cartograph-Session"] = token
        h.update(headers or {})
        conn.request(method, path, headers=h)
        r = conn.getresponse()
        body = r.read()
        conn.close()
        return r.status, (json.loads(body) if body else None)

    def check(name, ok, detail=""):
        nonlocal failures
        print(("PASS " if ok else "FAIL ") + name + (f" — {detail}" if detail else ""))
        if not ok:
            failures += 1

    status, st = request("GET", "/api/status")
    check("status reachable", status == 200, str(st))
    analysis = st["analysis"]

    _, payload = request("GET", "/api/analysis")
    _, report = request("GET", "/api/report")
    check("payload and report belong to one analysis",
          payload["analysis"] == report["analysis"] == analysis,
          f"{payload['analysis']} / {report['analysis']} / {analysis}")

    # 1. Counts.
    s = cli["summary"]
    t = report["totals"]
    pairs = {
        "files": (t["files"], s.get("files")),
        "nodes": (t["nodes"], len(cli["nodes"])),
        "edges": (t["edges"], len(cli["edges"])),
        "scene nodes": (len(payload["scene"]["nodes"]), len(cli["nodes"])),
        "scene edges": (len(payload["scene"]["edges"]), len(cli["edges"])),
        "summary nodes": (payload["summary"]["nodes"], len(cli["nodes"])),
        "summary edges": (payload["summary"]["edges"], len(cli["edges"])),
    }
    for key in ("routes", "http_observations", "orm_models", "orm_tables", "ambiguous",
                "no_match", "unsupported", "http_call_edges", "cross_language_edges"):
        cli_key = {"http_observations": "http_calls"}.get(key, key)
        if cli_key in s:
            pairs[key] = (t[key], s[cli_key])
    for key, (ours, theirs) in pairs.items():
        check(f"count {key}", ours == theirs, f"browser {ours}, CLI {theirs}")

    # 2 and 3. Identity of what is drawn.
    cli_edges = {e["id"]: e for e in cli["edges"]}
    cli_nodes = {n["id"]: n for n in cli["nodes"]}
    bad_edges = [e for e in payload["scene"]["edges"]
                 if e["id"] not in cli_edges
                 or (cli_edges[e["id"]]["source"], cli_edges[e["id"]]["target"], cli_edges[e["id"]]["kind"])
                 != (e["source"], e["target"], e["kind"])]
    check("every drawn edge is a CLI edge (id, endpoints, kind)", not bad_edges,
          f"{len(bad_edges)} mismatched of {len(payload['scene']['edges'])}")
    missing = set(cli_edges) - {e["id"] for e in payload["scene"]["edges"]}
    check("no CLI edge is missing from the drawing", not missing, f"{len(missing)} missing")
    bad_nodes = [n for n in payload["scene"]["nodes"]
                 if n["id"] not in cli_nodes
                 or (cli_nodes[n["id"]]["kind"], cli_nodes[n["id"]]["name"]) != (n["kind"], n["label"])]
    check("every drawn node is a CLI node (id, kind, name)", not bad_nodes,
          f"{len(bad_nodes)} mismatched of {len(payload['scene']['nodes'])}")
    located = {n["id"]: n for n in report["nodes"]}
    bad_loc = [i for i, n in cli_nodes.items()
               if located.get(i, {}).get("file") != n.get("file") or located.get(i, {}).get("line") != n.get("line")]
    check("report node locations equal CLI node file and line", not bad_loc, f"{len(bad_loc)} differ")

    # 4. Evidence equals the CLI's, on a deterministic sample.
    sample = sorted(cli_edges)[:: max(1, len(cli_edges) // 60)]
    latencies, mismatched = [], []
    for edge_id in sample:
        start = time.perf_counter()
        status, record = request("GET", f"/api/evidence?analysis={analysis}&edge={edge_id}")
        latencies.append((time.perf_counter() - start) * 1000)
        c = cli_edges[edge_id]
        if status != 200 or (
            record["evidence"], record["provenance"], round(record["confidence"], 4),
            record["location"]["file"], record["location"]["line"],
        ) != (c["evidence"], c["provenance"], round(c["confidence"], 4), c["file"], c["line"]):
            mismatched.append(edge_id)
    check(f"evidence equals CLI evidence on {len(sample)} sampled edges", not mismatched,
          f"mismatched: {mismatched[:5]}")

    node_lat = []
    for node_id in sorted(cli_nodes)[:: max(1, len(cli_nodes) // 40)]:
        start = time.perf_counter()
        request("GET", f"/api/node?analysis={analysis}&node={node_id}")
        node_lat.append((time.perf_counter() - start) * 1000)

    # 5. Stale selection.
    if analysis > 1:
        status, err = request("GET", f"/api/evidence?analysis={analysis - 1}&edge={sample[0]}")
        check("an earlier analysis id is refused as stale", status == 409 and err.get("kind") == "staleSelection",
              f"{status} {err}")
    else:
        print("SKIP stale check (only one analysis so far; refresh first)")

    # 6. Invalid requests.
    some_file = next(n["file"] for n in cli["nodes"] if n.get("file"))
    cases = [
        ("missing parameters", "GET", "/api/evidence", {}, True, 400),
        ("unknown edge", "GET", f"/api/evidence?analysis={analysis}&edge=999999999", {}, True, 404),
        ("unknown node", "GET", f"/api/node?analysis={analysis}&node=999999999", {}, True, 404),
        ("wrong method", "POST", "/api/evidence", {}, True, 405),
        ("unknown endpoint", "GET", "/api/files", {}, True, 404),
        ("no token", "GET", "/api/status", {}, False, 401),
        ("foreign Host", "GET", "/api/status", {"Host": f"evil.example:{port}"}, True, 421),
        ("foreign Origin", "GET", "/api/status", {"Origin": "http://evil.example"}, True, 403),
        ("source outside the analysis", "GET",
         f"/api/source?analysis={analysis}&file=..%2F..%2Fsecret.txt", {}, True, 404),
        ("source not cited", "GET", f"/api/source?analysis={analysis}&file=README.md", {}, True, 404),
        ("source cited", "GET",
         f"/api/source?analysis={analysis}&file={urllib.parse.quote(some_file, safe='')}&line=1", {}, True, 200),
        ("static traversal", "GET", "/../Cargo.toml", {}, True, 404),
    ]
    for name, method, path, headers, with_token, expected in cases:
        status, _ = request(method, path, headers, with_token)
        check(f"invalid request: {name} -> {expected}", status == expected, f"got {status}")

    def pct(xs, p):
        xs = sorted(xs)
        return xs[min(len(xs) - 1, int(round(p * (len(xs) - 1))))]

    print(f"INFO evidence lookup ms: median {statistics.median(latencies):.1f}, p95 {pct(latencies, 0.95):.1f} (n={len(latencies)})")
    print(f"INFO node lookup ms: median {statistics.median(node_lat):.1f}, p95 {pct(node_lat, 0.95):.1f} (n={len(node_lat)})")
    print(f"RESULT {'PASS' if failures == 0 else 'FAIL'}: {failures} failure(s)")
    return 1 if failures else 0


if __name__ == "__main__":
    sys.exit(main())
