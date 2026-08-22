# cartograph-cli

**Compute the architecture. Prove the relationships.**

This package is a **launcher**. The engine is a native binary written in Rust;
no analysis happens in JavaScript, and none ever will — two implementations of
the resolver would be two answers to the same question.

```bash
npx cartograph-cli .
```

## How the binary is found

In this order, stopping at the first that exists:

1. `$CARTOGRAPH_BINARY`, if set. A path that is set but missing is an error,
   not a reason to fall through — otherwise you would silently run a different
   build from the one you meant.
2. A binary vendored at `vendor/<target-triple>/cartograph`.
3. `target/release/cartograph` or `target/debug/cartograph`, relative to the
   repository — the layout a contributor has after `cargo build --release`.

Resolution never touches the network. A CLI that reaches for the internet when
you run it is a CLI that fails on a train.

If nothing is found, the error names all three ways to fix it and exits `3`.

## What the launcher guarantees

- `argv` is forwarded unchanged.
- stdio is **inherited**, so the child owns the terminal: JSON stays on stdout,
  progress and diagnostics stay on stderr, and neither is buffered by Node.
- The exit code is reproduced exactly — `2` usage, `3` input, `5` symbol not
  found, and the rest of [the documented set](../docs/development/cli.md#exit-codes).
  A wrapper that collapsed those into `1` would break every script.
- A signal is re-raised rather than translated, so Ctrl-C reports as Ctrl-C.

## Platforms

The release workflow is configured to build and smoke-test a native binary on
each of these, on that platform's own operating system:

| Platform | Target triple | Binary produced so far |
|---|---|---|
| macOS Apple Silicon | `aarch64-apple-darwin` | yes — built and exercised |
| Linux x64 | `x86_64-unknown-linux-gnu` | yes — built and smoke-tested in CI |
| macOS Intel | `x86_64-apple-darwin` | not yet — no release tag has been cut |
| Windows x64 | `x86_64-pc-windows-msvc` | not yet — no release tag has been cut |

The last two are **configured, not produced**. This project does not claim a
platform binary exists until CI has actually built and smoke-tested one, so
until a `v*` tag is pushed, build from source there:

```bash
cargo install --path crates/cartograph-cli
```

Anything not listed: build from source as well.
