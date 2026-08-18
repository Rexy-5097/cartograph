#!/usr/bin/env python3
"""Cartograph quality gates QG-001 … QG-008, executable.

Run from anywhere: resolves the repository root relative to this file.
Exit 0 only if every gate passes. CI runs exactly this script, so a local
`make gates` and CI cannot disagree.

Definitions: agentos/gates/. This script is Cartograph-owned (not upstream
AgentOS); it lives beside the framework tooling for discoverability.
"""

import os
import re
import subprocess
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__)))))

RESULTS = []


def gate(gate_id, name):
    def deco(fn):
        def wrapper():
            try:
                problems = fn() or []
            except Exception as e:  # a crashed gate is a failed gate
                problems = [f"gate crashed: {e}"]
            RESULTS.append((gate_id, name, problems))
            status = "PASS" if not problems else "FAIL"
            print(f"[{status}] {gate_id} {name}")
            for p in problems:
                print(f"       - {p}")
        return wrapper
    return deco


def run(cmd, timeout=1200):
    return subprocess.run(cmd, cwd=ROOT, capture_output=True, text=True, timeout=timeout)


def read_text(path):
    """Reads a file as text, tolerating bytes that are not valid UTF-8.

    Two hazards make strict decoding wrong here. macOS on exFAT writes binary
    AppleDouble sidecars named `._name.rs`, which match source-file globs; and
    the repository legitimately contains a non-UTF-8 fixture (the parser has to
    prove it fails softly). A gate must not crash on either.
    """
    try:
        with open(path, encoding="utf-8", errors="ignore") as fh:
            return fh.read()
    except OSError:
        return ""


def is_sidecar(name):
    """macOS AppleDouble sidecar, which is never project source."""
    return os.path.basename(name).startswith("._")


def source_files(root, suffix):
    """Every real source file under `root`, sidecars excluded."""
    found = []
    for dirpath, _, files in os.walk(root):
        for name in files:
            if name.endswith(suffix) and not is_sidecar(name):
                found.append(os.path.join(dirpath, name))
    return found


def tracked_files():
    out = run(["git", "ls-files"]).stdout
    return [line for line in out.splitlines() if line]


@gate("QG-001", "Repository integrity")
def qg001():
    problems = []
    required = [
        "Cargo.toml", "Makefile", "LICENSE", ".gitignore", ".gitattributes",
        "README.md", "CHANGELOG.md", "CONTRIBUTING.md", "SECURITY.md",
        "CODE_OF_CONDUCT.md", "AGENTS.md", "AGENTOS.md", "PROJECT_RULES.md",
        "ARCHITECTURE.md", "ROADMAP.md", "CHECKPOINTS.md",
        "agentos/PROJECT_CONFIG.yaml", "agentos/artifacts/project-state.yaml",
        ".github/workflows/ci.yml", ".github/pull_request_template.md",
        ".github/CODEOWNERS",
    ]
    for f in required:
        if not os.path.exists(os.path.join(ROOT, f)):
            problems.append(f"missing {f}")
    crates = ["cartograph-core", "cartograph-graph", "cartograph-parser",
              "cartograph-resolver", "cartograph-cli", "cartograph-testkit"]
    for c in crates:
        if not os.path.exists(os.path.join(ROOT, "crates", c, "Cargo.toml")):
            problems.append(f"missing crate {c}")
    sidecars = [f for f in tracked_files() if os.path.basename(f).startswith("._")]
    if sidecars:
        problems.append(f"{len(sidecars)} AppleDouble sidecar file(s) tracked, e.g. {sidecars[0]}")
    return problems


@gate("QG-002", "Formatting")
def qg002():
    r = run(["cargo", "fmt", "--all", "--", "--check"])
    return [] if r.returncode == 0 else ["cargo fmt --check failed:\n" + (r.stdout or r.stderr)[:800]]


@gate("QG-003", "Lints (clippy -D warnings)")
def qg003():
    r = run(["cargo", "clippy", "--workspace", "--all-targets", "--all-features",
             "--", "-D", "warnings"])
    return [] if r.returncode == 0 else ["clippy failed:\n" + r.stderr[-1200:]]


@gate("QG-004", "Tests")
def qg004():
    r = run(["cargo", "test", "--workspace"])
    return [] if r.returncode == 0 else ["cargo test failed:\n" + (r.stdout + r.stderr)[-1200:]]


SECRET_PATTERNS = [
    (re.compile(r"ghp_[A-Za-z0-9]{20,}"), "GitHub personal access token"),
    (re.compile(r"gho_[A-Za-z0-9]{20,}"), "GitHub OAuth token"),
    (re.compile(r"AKIA[0-9A-Z]{16}"), "AWS access key"),
    (re.compile(r"-----BEGIN (RSA|OPENSSH|EC|DSA|PGP) PRIVATE KEY"), "private key block"),
    (re.compile(r"xox[baprs]-[A-Za-z0-9-]{10,}"), "Slack token"),
]

# Machine-specific path detectors. Placeholder/example paths that documentation
# legitimately uses are allowed; a real user home or this project's real volume
# is not.
MACHINE_PATH = re.compile(r"(/Users/[a-z][a-z0-9_.-]+/|/home/[a-z][a-z0-9_.-]+/|C:\\Users\\[A-Za-z][^\\]*\\)")
ALLOWED_PATH_HINTS = ("dev/", "External/", "user/", "username/", "example", "<", "you/")
TEXT_EXT = {".rs", ".toml", ".md", ".yml", ".yaml", ".py", ".json", ".sh", ".txt", ""}


@gate("QG-005", "Security & secrets")
def qg005():
    problems = []
    for f in tracked_files():
        base = os.path.basename(f)
        if base == ".env" or base.endswith((".pem", ".key", ".p12")) or base == "credentials.json":
            problems.append(f"credential-shaped file tracked: {f}")
        if os.path.splitext(f)[1] not in TEXT_EXT:
            continue
        text = read_text(os.path.join(ROOT, f))
        for pat, label in SECRET_PATTERNS:
            if pat.search(text):
                problems.append(f"{label} pattern in {f}")
        for m in MACHINE_PATH.finditer(text):
            # Normalise separators so `C:\Users\dev\` and `/home/dev/`
            # hit the same placeholder allowlist.
            frag = m.group(0).replace("\\", "/")
            if not any(h in frag for h in ALLOWED_PATH_HINTS):
                problems.append(f"machine-specific path `{m.group(0)}` in {f}")
    return problems


PROHIBITED_DEPS = [
    "neo4j", "sqlx", "diesel", "postgres", "redis", "aws-sdk", "rusoto",
    "kafka", "rdkafka", "kube", "langchain", "qdrant", "milvus", "pinecone",
]


@gate("QG-006", "Architecture compliance")
def qg006():
    problems = []
    manifests = [f for f in tracked_files() if f.endswith("Cargo.toml")]
    dep_line = re.compile(r'^\s*"?([A-Za-z0-9_-]+)"?\s*=')
    for mf in manifests:
        for_lines = read_text(os.path.join(ROOT, mf)).splitlines()
        if True:
            in_deps = False
            for line in for_lines:
                s = line.strip()
                if s.startswith("["):
                    in_deps = "dependencies" in s
                    continue
                if in_deps:
                    m = dep_line.match(line)
                    if m and m.group(1).lower() in PROHIBITED_DEPS:
                        problems.append(f"prohibited v1 dependency `{m.group(1)}` in {mf}")
    # petgraph containment: only cartograph-graph may depend on it...
    for mf in manifests:
        if mf == "crates/cartograph-graph/Cargo.toml" or mf == "Cargo.toml":
            continue
        if re.search(r"^\s*petgraph", read_text(os.path.join(ROOT, mf)), re.M):
            if True:
                problems.append(f"petgraph dependency outside cartograph-graph: {mf}")
    # ...and it may not leak through cartograph-graph's public API.
    graph_src = os.path.join(ROOT, "crates", "cartograph-graph", "src")
    for path in source_files(graph_src, ".rs"):
        for i, line in enumerate(read_text(path).splitlines(), 1):
            if re.search(r"\bpub\b.*petgraph::", line) or re.search(r"pub use .*petgraph", line):
                problems.append(f"petgraph in public API: {os.path.basename(path)}:{i}")
    # dependency direction: core must not depend on any cartograph crate
    if True:
        if re.search(r"^\s*cartograph-", read_text(os.path.join(ROOT, "crates/cartograph-core/Cargo.toml")), re.M):
            problems.append("cartograph-core depends on another cartograph crate")
    return problems


@gate("QG-007", "Documentation & change tracking")
def qg007():
    problems = []
    for f, needle in [
        ("CHANGELOG.md", "M03"),
        ("CHECKPOINTS.md", "M03"),
        ("agentos/context/state.md", "M03"),
    ]:
        path = os.path.join(ROOT, f)
        if not os.path.exists(path):
            problems.append(f"missing {f}")
            continue
        if True:
            if needle not in read_text(path):
                problems.append(f"{f} has no entry for the active milestone")
    return problems


@gate("QG-008", "Milestone acceptance (M03)")
def qg008():
    problems = []
    # M03 delivers canonical route normalisation. The defining constraint is
    # that canonicalisation must not become matching: M04 owns the join.
    resolver_src = os.path.join(ROOT, "crates/cartograph-resolver/src")
    for required in (
        "crates/cartograph-resolver/src/canonical.rs",
        "crates/cartograph-resolver/src/normalize.rs",
        "crates/cartograph-resolver/tests/canonicalization.rs",
        "crates/cartograph-resolver/benches/canonicalization.rs",
        "docs/resolver/canonical-routes.md",
    ):
        if not os.path.exists(os.path.join(ROOT, required)):
            problems.append(f"M03 deliverable missing: {required}")

    # M04 capability must be absent. These names would each mean the resolver
    # had started joining two observations rather than canonicalising one.
    forbidden = (
        "resolve_match",
        "match_backend",
        "create_http_edge",
        "match_route",
        "resolve_route",
        "join_observations",
        "cross_stack_match",
    )
    for path in source_files(resolver_src, ".rs"):
        for i, line in enumerate(read_text(path).splitlines(), 1):
            code = line.split("//")[0]
            if any(sym in code for sym in forbidden):
                problems.append(f"M04 matching in {os.path.basename(path)}:{i}")

    # The resolver must not construct graph edges at M03.
    for path in source_files(resolver_src, ".rs"):
        code = "\n".join(
            line.split("//")[0] for line in read_text(path).splitlines()
        )
        for sym in ("EdgeSpec", "add_edge", "cartograph_graph", "cartograph_core::Edge"):
            if sym in code:
                problems.append(f"graph edge construction in {os.path.basename(path)}: {sym}")

    # No future-milestone dependencies.
    dep_names = {
        m.group(1)
        for line in read_text(os.path.join(ROOT, "Cargo.toml")).splitlines()
        if not line.lstrip().startswith("#")
        for m in [re.match(r'\s*"?([A-Za-z0-9_-]+)"?\s*=', line)]
        if m
    }
    for premature in ("async-lsp", "lsp-types", "redb", "gix", "notify", "rmcp", "regex", "url"):
        if premature in dep_names:
            problems.append(f"future-milestone dependency `{premature}` introduced at M03")

    # The CLI must not pretend `analyze` exists before M09.
    if "Analyze" in read_text(os.path.join(ROOT, "crates/cartograph-cli/src/main.rs")):
        problems.append("CLI exposes an `analyze` command before M09")

    # Milestone definitions exist.
    for m in (
        "M00-foundation.md",
        "M01-typescript-extraction.md",
        "M02-python-extraction.md",
        "M03-route-normalization.md",
    ):
        if not os.path.exists(os.path.join(ROOT, "agentos/milestones", m)):
            problems.append(f"missing milestone definition {m}")
    return problems


def main():
    print(f"Cartograph quality gates — root: {ROOT}\n")
    for fn in [qg001, qg002, qg003, qg004, qg005, qg006, qg007, qg008]:
        fn()
    failed = [(g, n) for g, n, p in RESULTS if p]
    print()
    if failed:
        print(f"RESULT: FAIL ({len(failed)}/{len(RESULTS)} gates failed)")
        return 1
    print(f"RESULT: PASS ({len(RESULTS)}/{len(RESULTS)} gates)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
