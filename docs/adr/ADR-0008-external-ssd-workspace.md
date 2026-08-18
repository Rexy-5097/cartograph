# ADR-0008 — Repository and heavy artifacts on the external SSD

**Status:** Accepted · 2026-08-18

## Context

Development machine: MacBook Air M4, 16 GB RAM, 256 GB internal SSD, 1 TB
Samsung T7 external SSD. The internal disk cannot hold the repo, cargo target
directories, benchmark corpora (10–15 real repositories at M08), Git mirrors
and profiling data.

## Decision

The repository, cargo build artifacts, benchmark corpora, Git mirrors and
profiling output live on the external volume. Toolchains and the cargo
registry stay on the internal disk. **No machine-specific path is committed**:
the target-dir override lives in the developer's user-level
`~/.cargo/config.toml`, documented (not committed) in
[docs/development/external-storage.md](../development/external-storage.md).

## Alternatives

- **Everything internal** — rejected: 256 GB minus OS and toolchains cannot
  hold the M08 corpus.
- **Committed `.cargo/config.toml` with a `/Volumes/...` path** — rejected:
  breaks every other machine, violates the no-machine-paths rule.

## Consequences

The volume shipped exFAT: no hard links (cargo copies its incremental cache —
harmless, slower), no xattrs (macOS writes AppleDouble `._*` sidecars,
including inside `.git`). Mitigations: `._*` gitignored, `make clean-sidecars`,
QG-001 fails if a sidecar is ever tracked. Reformatting to APFS is the
recommended permanent fix and is documented as a user decision, not performed
unilaterally.
