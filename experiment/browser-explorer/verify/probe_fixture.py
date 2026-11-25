"""Edge-case check: start cartograph-serve on one repository and report what it serves.

EXPERIMENTAL (M18). Used for the empty-graph and partial-analysis cases:

    python -B probe_fixture.py path/to/cartograph-serve.exe ASSETS_DIR REPO PORT

Starts the server, reads the session URL it prints, waits for the first
analysis, prints /api/status, the report totals and diagnostics, checks that
a request after shutdown is refused, and stops the server.
"""

import http.client
import json
import re
import subprocess
import sys
import time


def main() -> int:
    exe, assets, repo, port = sys.argv[1], sys.argv[2], sys.argv[3], int(sys.argv[4])
    proc = subprocess.Popen(
        [exe, "--port", str(port), "--assets", assets, repo],
        stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True,
    )
    token = None
    for _ in range(5):
        line = proc.stdout.readline()
        print("server:", line.rstrip())
        m = re.search(r"#token=([0-9a-f]+)", line)
        if m:
            token = m.group(1)
            break
    if token is None:
        print("FAIL no token printed")
        proc.kill()
        return 1

    def get(path):
        conn = http.client.HTTPConnection("127.0.0.1", port, timeout=60)
        conn.request("GET", path, headers={"Host": f"127.0.0.1:{port}", "X-Cartograph-Session": token})
        r = conn.getresponse()
        body = r.read()
        conn.close()
        return r.status, json.loads(body) if body else None

    status = None
    for _ in range(120):
        _, status = get("/api/status")
        if status["phase"] in ("ready", "failed"):
            break
        time.sleep(0.25)
    print("status:", json.dumps(status))
    code, report = get("/api/report")
    print("report:", code)
    if code == 200:
        print("totals:", json.dumps(report["totals"]))
        print("diagnostics:", json.dumps(report["diagnostics"]))
        code, payload = get("/api/analysis")
        print("scene:", len(payload["scene"]["nodes"]), "nodes,", len(payload["scene"]["edges"]), "edges")
    else:
        print("body:", json.dumps(report))
    proc.terminate()
    proc.wait(timeout=10)
    try:
        get("/api/status")
        print("FAIL server still answering after stop")
        return 1
    except OSError as err:
        print("after stop: refused as expected:", type(err).__name__)
    return 0


if __name__ == "__main__":
    sys.exit(main())
