# QG-003 — Lints

`cargo clippy --workspace --all-targets --all-features -- -D warnings` exits 0.
Workspace lints: `unsafe_code = forbid`, `missing_docs = warn`, clippy
`all` + `pedantic` at warn (denied in this gate). `#[allow]` requires an
adjacent comment explaining why.
