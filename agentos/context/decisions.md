# ADR index — Cartograph

> Cartograph's own decisions live in [../../docs/adr/](../../docs/adr/).
> `artifacts/decisions/` holds upstream AgentOS framework history (ADR-0001…0056) — do not mix the two.

| ADR | Title | Status |
|---|---|---|
| [0001](../../docs/adr/ADR-0001-rust-core-is-the-product.md) | Rust core is the product; CLI/desktop/MCP are clients | Accepted |
| [0002](../../docs/adr/ADR-0002-two-tier-analysis.md) | Two-tier analysis: tree-sitter + LSP | Accepted |
| [0003](../../docs/adr/ADR-0003-cartograph-owns-the-graph.md) | Cartograph owns the domain graph; petgraph underneath | Accepted |
| [0004](../../docs/adr/ADR-0004-evidence-first-class.md) | Evidence, provenance, confidence are first-class edge fields | Accepted |
| [0005](../../docs/adr/ADR-0005-local-first-privacy.md) | Local-first privacy model | Accepted |
| [0006](../../docs/adr/ADR-0006-sigma-before-custom-renderer.md) | Sigma.js before any custom renderer | Accepted |
| [0007](../../docs/adr/ADR-0007-no-llm-graph-construction.md) | No LLM graph construction, ever | Accepted |
| [0008](../../docs/adr/ADR-0008-external-ssd-workspace.md) | Repository and build artifacts on external SSD | Accepted |
| [0009](../../docs/adr/ADR-0009-vendored-agentos.md) | AgentOS vendored under `agentos/` with minimal modification | Accepted |
| [0010](../../docs/adr/ADR-0010-checkpoint-tag-policy.md) | Provisional vs accepted checkpoint tags | Accepted |
| [0011](../../docs/adr/ADR-0011-node-identity-within-a-graph.md) | Node identity within a graph: (kind, name, file) | Accepted · partly overtaken by 0014 |
| [0012](../../docs/adr/ADR-0012-the-client-wrapper-is-a-node.md) | The client wrapper is a node | Accepted |
| [0013](../../docs/adr/ADR-0013-m10-resolver-semantic-compatibility.md) | M10 may change the resolver, but not M09's semantics | Accepted |
| [0014](../../docs/adr/ADR-0014-m10-scope-reconciliation.md) | M10 delivers incremental Stage C; three scope items deferred | Accepted |
| [0015](../../docs/adr/ADR-0015-layout-contract.md) | The layout contract: directory clusters, per-cluster relaxation, determinism without stability | Accepted |
| [0016](../../docs/adr/ADR-0016-desktop-boundary.md) | The desktop boundary: shared pipeline crate, testable session, thin shell outside the workspace | Accepted |
| [0017](../../docs/adr/ADR-0017-render-scene.md) | The render scene: layout plus the minimum a reader needs | Accepted |
| [0018](../../docs/adr/ADR-0018-blast-radius-confidence.md) | Blast radius confidence: weakest link along a path, best route to a node | Accepted |
| [0019](../../docs/adr/ADR-0019-trace-belongs-to-the-graph.md) | The trace traversal belongs to the graph, not to a client | Accepted |
| [0020](../../docs/adr/ADR-0020-mcp-boundary-and-authorization.md) | MCP boundary and authorization model: map is an aggregated view, authorization is one canonical repository identity per session, transport is stdio | Accepted · Amendments 1-2: identity is grant-supplied, granted in argv at launch |
