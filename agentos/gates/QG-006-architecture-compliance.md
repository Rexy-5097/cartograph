# QG-006 — Architecture compliance

- **Prohibited-for-v1 dependency scan** over all `Cargo.toml` files: neo4j,
  postgres/sqlx/diesel, redis, aws-sdk/s3, kafka, kubernetes, langchain,
  vector-db crates, docker requirements.
- **petgraph containment**: `petgraph` appears only in `cartograph-graph`'s
  dependencies and never in any `pub` signature (checked by grep over public
  API surface + enforced by outside-in integration tests).
- **Dependency direction**: core depends on no Cartograph crate; nothing
  depends on cli.
- **Frozen-stack conformance**: new dependencies not named in the frozen stack
  need an ADR reference in the PR.
