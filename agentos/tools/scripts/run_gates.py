#!/usr/bin/env python3
"""Cartograph quality gates QG-001 … QG-008, executable.

Run from anywhere: resolves the repository root relative to this file.
Exit 0 only if every gate passes. CI runs exactly this script, so a local
`make gates` and CI cannot disagree.

Definitions: agentos/gates/. This script is Cartograph-owned (not upstream
AgentOS); it lives beside the framework tooling for discoverability.
"""

import hashlib
import json
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


@gate("QG-008", "Milestone acceptance (M07)")
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
        # M07: the validation milestone's own artefacts. A benchmark whose
        # corpus, scope or results are missing is not a benchmark.
        "benchmarks/corpus.json",
        "benchmarks/supported-subset.json",
        "benchmarks/run_benchmark.py",
        "benchmarks/evaluate.py",
        "benchmarks/results/m07-pass1-evaluation.json",
        "benchmarks/results/m07-pass2-evaluation.json",
        "docs/benchmarks/m07-report.md",
    ):
        if not os.path.exists(os.path.join(ROOT, required)):
            problems.append(f"milestone deliverable missing: {required}")

    # M08+ capability must be absent. These names would each mean a later
    # milestone had started: Git co-change, incremental invalidation, blast
    # radius, structural diff, MCP. M07 adds no product capability at all —
    # it measures the engine M06 accepted.
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
                problems.append(f"M08+ capability in {os.path.basename(path)}:{i}")

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
    #
    # `rmcp` and `tokio` left this list at M15, which is the milestone that
    # needs them: rmcp is the MCP SDK the frozen stack names for M15, and tokio
    # is the runtime it requires. The list says "not yet", so an entry leaves it
    # when its milestone arrives -- the same move `diff` made out of the CLI's
    # forbidden-subcommand list when M13 built it. The remaining eight are still
    # unbuilt.
    dep_names = {
        m.group(1)
        for line in read_text(os.path.join(ROOT, "Cargo.toml")).splitlines()
        if not line.lstrip().startswith("#")
        for m in [re.match(r'\s*"?([A-Za-z0-9_-]+)"?\s*=', line)]
        if m
    }
    for premature in ("async-lsp", "lsp-types", "redb", "gix", "notify", "regex", "url", "salsa"):
        if premature in dep_names:
            problems.append(f"future-milestone dependency `{premature}` introduced early")

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
    # M07 is a validation milestone. Its regression fixtures live beside the
    # tests of the milestones whose defects they pin, so the files it owns are
    # the CLI's graph export and the benchmark harness itself.
    "M07": ["crates/cartograph-cli/tests/cli.rs"],
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


def benchmark_problems():
    """Checks the real-repository benchmark artifacts, when a milestone has them.

    A benchmark is only evidence if its parts agree with each other. These
    checks are the ones an outside reader would run before believing a number:
    that the corpus is pinned, that scope was declared rather than fitted, that
    ground truth records refusals and not only successes, and that the results
    on disk describe the corpus and ground truth now in the tree.

    Runs whenever benchmarks/corpus.json exists, so it keeps applying after the
    milestone that introduced it.
    """
    corpus_path = os.path.join(ROOT, "benchmarks/corpus.json")
    if not os.path.exists(corpus_path):
        return []

    problems = []
    try:
        corpus = json.loads(read_text(corpus_path))
        subset = json.loads(read_text(os.path.join(ROOT, "benchmarks/supported-subset.json")))
    except (json.JSONDecodeError, OSError) as exc:
        return [f"benchmark artifacts are not readable: {exc}"]

    repositories = corpus.get("repositories", [])
    if len(repositories) < 7:
        problems.append(
            f"the corpus declares {len(repositories)} repositories; M07 requires at least 7"
        )
    for entry in repositories:
        if not re.fullmatch(r"[0-9a-f]{40}", entry.get("commit", "")):
            problems.append(f"corpus entry {entry.get('name')} is not pinned to an exact commit")

    # Scope has to be declared, and the gaps predicted in advance have to still
    # be there — deleting one after a pass is how a miss becomes "out of scope".
    for section in ("in_scope", "out_of_scope", "safe_refusal", "known_implementation_gaps"):
        if not subset.get(section):
            problems.append(f"supported-subset.json has no {section} section")
    if len(subset.get("known_implementation_gaps", {}).get("entries", [])) < 1:
        problems.append("supported-subset.json predicts no implementation gaps")

    # Ground truth exists for every corpus entry, was reviewed, and was not
    # authored from the analyser's own output.
    for entry in repositories:
        name = entry.get("name")
        path = os.path.join(ROOT, f"benchmarks/ground-truth/{name}.json")
        if not os.path.exists(path):
            problems.append(f"no ground truth for corpus entry {name}")
            continue
        gt = json.loads(read_text(path))
        if gt.get("commit") != entry.get("commit"):
            problems.append(f"ground truth for {name} names a different commit than the corpus")
        if gt.get("verification", {}).get("cartograph_output_consulted") is not False:
            problems.append(f"ground truth for {name} does not assert independence from output")
        records = gt.get("route_declarations", []) + gt.get("orm_declarations", [])
        if not any(r.get("classification") in ("UNSUPPORTED", "SAFE_REFUSAL") for r in records):
            problems.append(
                f"ground truth for {name} records no refusal and no unsupported case; "
                "a corpus entry that only contains what works is not evidence"
            )

    # Results describe the corpus that is in the tree, not an earlier one.
    for pass_number in (1, 2):
        path = os.path.join(ROOT, f"benchmarks/results/m07-pass{pass_number}-evaluation.json")
        if not os.path.exists(path):
            problems.append(f"no evaluation recorded for pass {pass_number}")
            continue
        result = json.loads(read_text(path))
        if result.get("corpus_sha256") != sha256_of(corpus_path):
            problems.append(
                f"pass {pass_number} results were produced against a different corpus "
                "than the one committed"
            )
        if not result.get("adversarial_checks"):
            problems.append(f"pass {pass_number} results record no adversarial checks")
        # Deleting a ground-truth record shrinks both sides of the recall
        # ratio, so the denominator check alone cannot see it — that attack
        # was the one this suite initially missed. Binding the results to the
        # digest of the ground truth they were computed from closes it: an
        # edited record forces a re-evaluation, which is the point.
        for name, recorded in (result.get("ground_truth_sha256") or {}).items():
            path = os.path.join(ROOT, f"benchmarks/ground-truth/{name}.json")
            if not os.path.exists(path):
                problems.append(f"pass {pass_number} scored {name}, whose ground truth is gone")
            elif sha256_of(path) != recorded:
                problems.append(
                    f"pass {pass_number} was scored against a different version of "
                    f"{name}'s ground truth than the one committed"
                )
        # The recall denominator must equal the in-scope record count, which is
        # what stops a denominator being trimmed after the fact.
        for name, repo in result.get("repositories", {}).items():
            if repo.get("status") != "OK":
                continue
            gt = json.loads(read_text(os.path.join(ROOT, f"benchmarks/ground-truth/{name}.json")))
            declared = sum(
                1 for r in gt["route_declarations"] if r["classification"] == "IN_SCOPE"
            )
            scored = repo["routes"]["metrics"]["recall_denominator"]
            if declared != scored:
                problems.append(
                    f"pass {pass_number} {name}: recall denominator {scored} does not "
                    f"match {declared} in-scope ground-truth records"
                )
    return problems


def calibration_problems():
    """Checks the M08 calibration artefacts, when a milestone has them.

    Runs whenever benchmarks/m08/results/dataset.json exists, so it keeps
    applying after the milestone that introduced it. The substance lives in
    benchmarks/m08/verify_integrity.py; this gate refuses to let a result be
    committed whose own integrity checks do not pass.
    """
    dataset = os.path.join(ROOT, "benchmarks/m08/results/dataset.json")
    if not os.path.exists(dataset):
        return []

    problems = []
    try:
        data = json.loads(read_text(dataset))
    except json.JSONDecodeError as exc:
        return [f"the M08 dataset is not readable: {exc}"]

    if not data.get("records"):
        problems.append("the M08 dataset holds no labelled records")
    if not data.get("records_sha256"):
        problems.append("the M08 dataset is not bound to a digest of its own records")

    labels = {r.get("label") for r in data.get("records", [])}
    if "UNVERIFIABLE" not in labels:
        problems.append(
            "the M08 dataset records no unverifiable observations; a dataset "
            "containing only what could be checked scores itself on the easy "
            "half of its own corpus"
        )

    # Every calibration must be bound to the dataset it was computed from, and
    # must not report an accuracy for a group it never verified.
    for name in ("calibration-development", "calibration-holdout"):
        path = os.path.join(ROOT, f"benchmarks/m08/results/{name}.json")
        if not os.path.exists(path):
            problems.append(f"no M08 {name} result recorded")
            continue
        cal = json.loads(read_text(path))
        if cal.get("bound_to") != data.get("bound_to"):
            problems.append(f"{name} was computed against different inputs than the dataset")
        for value, entry in cal.get("by_confidence_value", {}).items():
            if entry.get("verified") == 0 and entry.get("observed_accuracy") is not None:
                problems.append(
                    f"{name} reports an accuracy at confidence {value} with no verified observations"
                )
            if 0 < entry.get("verified", 0) < cal.get("weak_sample_threshold", 30) \
                    and entry.get("sample_adequate"):
                problems.append(
                    f"{name} presents {entry['verified']} observations at confidence "
                    f"{value} as an adequate sample"
                )

    integrity = os.path.join(ROOT, "benchmarks/m08/results/integrity.json")
    if not os.path.exists(integrity):
        problems.append("no M08 integrity result recorded")
    else:
        failed = [c["check"] for c in json.loads(read_text(integrity)).get("checks", [])
                  if not c.get("passed")]
        if failed:
            problems.append(f"M08 integrity checks failing: {', '.join(failed[:3])}")
    return problems


def sha256_of(path):
    digest = hashlib.sha256()
    with open(path, "rb") as handle:
        for block in iter(lambda: handle.read(1 << 20), b""):
            digest.update(block)
    return digest.hexdigest()


@gate("QG-009", "Continuous verification")
def qg009():
    problems = []
    milestone = current_milestone()
    if not milestone:
        return ["cannot determine the current milestone from project-state.yaml"]

    sources = milestone_test_sources(milestone)
    entry = checkpoint_entry(milestone)
    problems += benchmark_problems()
    problems += calibration_problems()

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
