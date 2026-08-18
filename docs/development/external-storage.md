# External storage setup

The repository, cargo build artifacts, benchmark corpora, Git mirrors and
profiling data live on an external SSD; toolchains and the cargo registry stay
on the internal disk (ADR-0008). **No machine-specific path is ever committed**
— everything below is per-developer, user-level configuration.

## Cargo target directory

Put build artifacts on the external volume via your **user-level**
`~/.cargo/config.toml` (never the repository):

```toml
# ~/.cargo/config.toml — adjust the volume name to your machine
[build]
target-dir = "/Volumes/<your-external-volume>/cargo-target"

[profile.dev]
debug = 1            # line tables only — halves target size

[profile.dev.package."*"]
opt-level = 2        # dependencies optimized once

[profile.release]
lto = "thin"
codegen-units = 4
```

Alternatively `export CARGO_TARGET_DIR=...` in your shell profile. The
repository's committed `Cargo.toml` already applies the profile settings;
duplicating them user-level is harmless.

Find your volume: `ls /Volumes` / `diskutil info "/Volumes/<name>"`.

`sccache` is a worthwhile addition (`cargo install sccache`, then
`build.rustc-wrapper = "sccache"` user-level).

## exFAT caveats (T7-class drives ship exFAT)

- **No hard links** → cargo prints "hard linking files in the incremental
  compilation cache failed. copying files instead" and copies. Harmless,
  slightly slower.
- **No extended attributes** → macOS materialises xattrs as AppleDouble `._*`
  sidecar files next to everything it touches, including inside `.git`, where
  git prints `error: non-monotonic index ... ._pack-*.idx`. The sidecars are
  junk metadata: `.gitignore` excludes them, `make clean-sidecars` deletes
  them, and QG-001 fails if one is ever tracked.
- **Permanent fix (recommended, user decision):** reformat the volume APFS in
  Disk Utility — this erases it — or add an APFS partition for development.
  Everything works correctly on exFAT with the mitigations above; APFS removes
  the noise and enables hard links.

## What lives where

| Internal disk | External SSD |
|---|---|
| macOS, toolchains, rustup | repository |
| cargo registry (`~/.cargo/registry`) | cargo target dir |
| IDE caches | benchmark corpora (M08), Git mirrors, profiling output |
