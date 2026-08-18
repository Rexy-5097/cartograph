# Quality gates — Cartograph

Cartograph-specific gates, numbered to match the AgentOS framework's checklist
gates but defined concretely for a Rust static-analysis engine. Executable via
`make gates` → [`../tools/scripts/run_gates.py`](../tools/scripts/run_gates.py).
CI runs the same script.

| Gate | Checks | Mechanism |
|---|---|---|
| QG-001 | Repository integrity | run_gates.py |
| QG-002 | Formatting | `cargo fmt --all -- --check` |
| QG-003 | Lints | `cargo clippy --workspace --all-targets --all-features -- -D warnings` |
| QG-004 | Tests | `cargo test --workspace` |
| QG-005 | Security & secrets | run_gates.py scans |
| QG-006 | Architecture compliance | run_gates.py scans |
| QG-007 | Documentation & change tracking | run_gates.py |
| QG-008 | Milestone acceptance | run_gates.py + milestone file |

A FAIL on any gate blocks milestone completion (RULE 017). Later milestones
add: benchmark correctness, cross-stack precision/recall, calibration,
performance, incremental latency, rendering performance, MCP correctness,
privacy verification, release verification.
