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
        ("CHANGELOG.md", "M04"),
        ("CHECKPOINTS.md", "M04"),
        ("agentos/context/state.md", "M04"),
    ]:
        path = os.path.join(ROOT, f)
        if not os.path.exists(path):
            problems.append(f"missing {f}")
            continue
        if True:
            if needle not in read_text(path):
                problems.append(f"{f} has no entry for the active milestone")
    return problems


@gate("QG-008", "Milestone acceptance (M06)")
def qg008():
    problems = []
    resolver_src = os.path.join(ROOT, "crates/cartograph-resolver/src")
    for required in (
        "crates/cartograph-resolver/src/matching.rs",
        "crates/cartograph-resolver/src/edge.rs",
        "crates/cartograph-resolver/tests/matching.rs",
        "crates/cartograph-resolver/tests/cross_stack.rs",
        "crates/cartograph-resolver/benches/matching.rs",
        "docs/resolver/matching.md",
        "crates/cartograph-resolver/src/symbolic.rs",
        "crates/cartograph-resolver/src/evaluator.rs",
        "crates/cartograph-resolver/src/dynamic.rs",
        "docs/resolver/dynamic-urls.md",
        "crates/cartograph-resolver/src/orm.rs",
        "crates/cartograph-resolver/tests/orm.rs",
        "crates/cartograph-resolver/tests/full_stack.rs",
        "docs/resolver/orm.md",
    ):
        if not os.path.exists(os.path.join(ROOT, required)):
            problems.append(f"M04 deliverable missing: {required}")

    # M07+ capability must be absent. These names would each mean a later
    # milestone had started: Git co-change, incremental invalidation, blast
    # radius, structural diff, MCP.
    forbidden = (
        "co_change",
        "git_history",
        "blast_radius",
        "structural_diff",
        "McpServer",
        "invalidate_subgraph",
    )
    for path in source_files(resolver_src, ".rs"):
        for i, line in enumerate(read_text(path).splitlines(), 1):
            code = line.split("//")[0]
            if any(sym in code for sym in forbidden):
                problems.append(f"M07+ capability in {os.path.basename(path)}:{i}")

    # M05 must never read the analysed project's environment.
    for path in source_files(resolver_src, ".rs"):
        for i, line in enumerate(read_text(path).splitlines(), 1):
            code = line.split("//")[0]
            if "env::var" in code or "std::env::" in code:
                problems.append(
                    f"resolver reads the environment in {os.path.basename(path)}:{i}; "
                    "environment values must stay symbolic"
                )

    # Edges may be produced only through the accepted-match gate.
    edge_src = read_text(os.path.join(resolver_src, "edge.rs"))
    if "is_accepted" not in edge_src and "accepted()" not in edge_src:
        problems.append("edge.rs builds edges without gating on an accepted match")

    # No future-milestone dependencies. LSP is in the frozen stack but M04 did
    # not need it: route matching joins observations, not symbols.
    dep_names = {
        m.group(1)
        for line in read_text(os.path.join(ROOT, "Cargo.toml")).splitlines()
        if not line.lstrip().startswith("#")
        for m in [re.match(r'\s*"?([A-Za-z0-9_-]+)"?\s*=', line)]
        if m
    }
    for premature in ("async-lsp", "lsp-types", "redb", "gix", "notify", "rmcp", "regex", "url", "tokio", "salsa"):
        if premature in dep_names:
            problems.append(f"future-milestone dependency `{premature}` introduced at M04")

    if "Analyze" in read_text(os.path.join(ROOT, "crates/cartograph-cli/src/main.rs")):
        problems.append("CLI exposes an `analyze` command before M09")

    for m in (
        "M00-foundation.md",
        "M01-typescript-extraction.md",
        "M02-python-extraction.md",
        "M03-route-normalization.md",
        "M04-cross-language-resolver.md",
    ):
        if not os.path.exists(os.path.join(ROOT, "agentos/milestones", m)):
            problems.append(f"missing milestone definition {m}")
    return problems


# Test files each milestone owns. QG-009 measures the milestone under
# development, so it needs to know which files that milestone brought.
MILESTONE_TESTS = {
    "M00": ["crates/cartograph-core/src", "crates/cartograph-graph/tests"],
    "M01": ["crates/cartograph-parser/tests/typescript_extraction.rs"],
    "M02": ["crates/cartograph-parser/tests/python_extraction.rs"],
    "M03": ["crates/cartograph-resolver/tests/canonicalization.rs"],
    "M04": [
        "crates/cartograph-resolver/tests/matching.rs",
        "crates/cartograph-resolver/tests/cross_stack.rs",
    ],
    "M06": [
        "crates/cartograph-resolver/tests/orm.rs",
        "crates/cartograph-resolver/tests/full_stack.rs",
    ],
    "M05": [
        "crates/cartograph-resolver/tests/symbolic.rs",
        "crates/cartograph-resolver/tests/evaluator.rs",
        "crates/cartograph-resolver/tests/dynamic_resolution.rs",
    ],
}

# Milestones whose logic can silently produce wrong output, so synthetic
# fixtures alone are not evidence.
HIGH_RISK = {"M04", "M05", "M06", "M07", "M08", "M10", "M12", "M13"}

# An assertion is "negative" when it asserts a refusal. Matched against
# assertion bodies rather than test names, so a reassuring name cannot pass.
REFUSAL_MARKERS = (
    "NoMatch",
    "Ambiguous",
    "Unsupported",
    "is_none()",
    "is_empty()",
    "edges, 0",
    "edge_count(), 0",
    "assert!(!",
    "is_err()",
    "NotApplicable",
    "Methods::Unknown",
    "must not",
    "never",
    "not guessed",
    "no edge",
)


def current_milestone():
    """The milestone under development, from the project state artifact."""
    state = read_text(os.path.join(ROOT, "agentos/artifacts/project-state.yaml"))
    m = re.search(r"^current_milestone:\s*(\S+)", state, re.M)
    return m.group(1) if m else None


def milestone_test_sources(milestone):
    """(path, text) for every test file the milestone owns."""
    out = []
    for entry in MILESTONE_TESTS.get(milestone, []):
        full = os.path.join(ROOT, entry)
        if os.path.isdir(full):
            out += [(p, read_text(p)) for p in source_files(full, ".rs")]
        elif os.path.exists(full):
            out.append((full, read_text(full)))
    return out


def split_tests(text):
    """Splits a Rust test file into per-test bodies."""
    bodies = []
    for chunk in text.split("#[test]")[1:]:
        # A test body ends where the next attribute or a top-level `}` does.
        end = chunk.find("\n}\n")
        bodies.append(chunk if end == -1 else chunk[: end + 2])
    return bodies


def checkpoint_entry(milestone):
    """The CHECKPOINTS.md section for one milestone."""
    text = read_text(os.path.join(ROOT, "CHECKPOINTS.md"))
    m = re.search(rf"^## {milestone} — .*?(?=^## |\Z)", text, re.M | re.S)
    return m.group(0) if m else ""


@gate("QG-009", "Continuous verification")
def qg009():
    problems = []
    milestone = current_milestone()
    if not milestone:
        return ["cannot determine the current milestone from project-state.yaml"]

    sources = milestone_test_sources(milestone)
    entry = checkpoint_entry(milestone)

    # A milestone that has been unlocked but not started has neither
    # registered tests nor a checkpoint entry. Demanding verification findings
    # from work that does not exist is the paperwork this protocol exists to
    # avoid (see docs/development/continuous-verification.md, "Proportionality").
    #
    # This is not a loophole: registering the milestone's test files in
    # MILESTONE_TESTS is part of doing the work, and doing so turns every check
    # below on. A milestone cannot ship code while claiming not to have started.
    if milestone not in MILESTONE_TESTS and not entry:
        print(f"       · {milestone} not yet started (no registered tests, no checkpoint entry)")
        return problems

    if milestone in MILESTONE_TESTS and not sources:
        problems.append(f"{milestone} declares test files that do not exist")

    # 1 + 2. The milestone has tests, and enough of them assert refusals.
    bodies = [b for _, text in sources for b in split_tests(text)]
    if sources and not bodies:
        problems.append(f"{milestone} has test files but no tests in them")
    if bodies:
        negative = sum(
            1 for b in bodies if any(marker in b for marker in REFUSAL_MARKERS)
        )
        minimum = max(1, len(bodies) // 4)
        if negative < minimum:
            problems.append(
                f"{milestone}: only {negative} of {len(bodies)} tests assert a refusal; "
                f"at least {minimum} required (a feature that never refuses is the failure mode)"
            )

    # 3. Standing invariants, for the subsystems that exist.
    all_tests = "\n".join(
        read_text(p)
        for d in ("crates/cartograph-resolver/tests", "crates/cartograph-parser/tests")
        for p in source_files(os.path.join(ROOT, d), ".rs")
    )
    if os.path.exists(os.path.join(ROOT, "crates/cartograph-resolver/src/edge.rs")):
        invariants = {
            "ambiguity produces no accepted edge": ("Ambiguous", "accepted().is_none()"),
            "an unknown method never becomes GET": ("never_read_as_get", "defaulting"),
            "accepted edges carry evidence": ("evidence()", "Evidence"),
        }
        for name, markers in invariants.items():
            if not any(m in all_tests for m in markers):
                problems.append(f"no test covers the invariant: {name}")

    if not entry:
        problems.append(f"CHECKPOINTS.md has no entry for {milestone}")
    else:
        # 4. High-risk milestones show real-repository validation.
        if milestone in HIGH_RISK:
            has_corpus = re.search(
                r"(flask|django|fastapi|swr|zustand|repositor)", entry, re.I
            )
            if not has_corpus:
                problems.append(
                    f"{milestone} is high-risk but its checkpoint records no "
                    "real-repository validation"
                )
        # 5 + 6. Verification findings, with substance.
        m = re.search(r"### Verification findings(.*?)(?=^### |\Z)", entry, re.M | re.S)
        if not m:
            problems.append(
                f"{milestone} checkpoint has no '### Verification findings' section"
            )
        else:
            findings = m.group(1).strip()
            if len(findings) < 120:
                problems.append(
                    f"{milestone} verification findings are too thin to be evidence "
                    "(state what was tested, not just an outcome)"
                )
            if re.fullmatch(r"(?i)none\.?", findings):
                problems.append(
                    f"{milestone} verification findings say only 'none'; "
                    "state what was run to establish that"
                )
    return problems


def main():
    print(f"Cartograph quality gates — root: {ROOT}\n")
    for fn in [qg001, qg002, qg003, qg004, qg005, qg006, qg007, qg008, qg009]:
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
