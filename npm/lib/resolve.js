"use strict";

// Where the native binary is, and nothing else.
//
// This package is a launcher. The engine is the Rust binary; no analysis
// happens in JavaScript, and none ever should — two implementations of the
// resolver would be two answers to the same question.

const os = require("os");
const path = require("path");
const fs = require("fs");

/** Target triples, keyed by what Node reports. */
const TARGETS = {
  "darwin:arm64": "aarch64-apple-darwin",
  "darwin:x64": "x86_64-apple-darwin",
  "linux:x64": "x86_64-unknown-linux-gnu",
  "win32:x64": "x86_64-pc-windows-msvc",
};

/** The target triple for the running machine, or null if unsupported. */
function target(platform = process.platform, arch = process.arch) {
  return TARGETS[`${platform}:${arch}`] || null;
}

/** The binary's file name on this platform. */
function binaryName(platform = process.platform) {
  return platform === "win32" ? "cartograph.exe" : "cartograph";
}

/**
 * Finds the binary to run.
 *
 * The order is deliberate. An explicit override wins, so a developer working
 * on the engine can point the wrapper at their own build without reinstalling
 * anything — which is what keeps local development independent of the network
 * (PART 16).
 *
 * Returns `{ path }` on success, or `{ error, target }` describing what is
 * missing. It never downloads anything at call time: a CLI that reaches for
 * the network when you run it is a CLI that fails on a train.
 */
function resolveBinary(options = {}) {
  const platform = options.platform || process.platform;
  const arch = options.arch || process.arch;
  const env = options.env || process.env;
  const root = options.root || path.join(__dirname, "..");

  // 1. Explicit override.
  if (env.CARTOGRAPH_BINARY) {
    if (fs.existsSync(env.CARTOGRAPH_BINARY)) {
      return { path: env.CARTOGRAPH_BINARY, source: "CARTOGRAPH_BINARY" };
    }
    return {
      error:
        `CARTOGRAPH_BINARY is set to ${env.CARTOGRAPH_BINARY}, but no file is there.`,
      target: target(platform, arch),
    };
  }

  const triple = target(platform, arch);
  if (!triple) {
    return {
      error:
        `Cartograph has no prebuilt binary for ${platform}/${arch}.\n` +
        `Supported: ${Object.keys(TARGETS).join(", ")}.\n` +
        `Build from source instead: cargo install --git https://github.com/Rexy-5097/cartograph cartograph-cli`,
      target: null,
    };
  }

  // 2. A binary vendored beside this package.
  const vendored = path.join(root, "vendor", triple, binaryName(platform));
  if (fs.existsSync(vendored)) {
    return { path: vendored, source: "vendored", target: triple };
  }

  // 3. A cargo build in the repository this package lives in — the layout a
  //    contributor has after `cargo build --release`.
  for (const candidate of [
    path.join(root, "..", "target", "release", binaryName(platform)),
    path.join(root, "..", "target", "debug", binaryName(platform)),
  ]) {
    if (fs.existsSync(candidate)) {
      return { path: candidate, source: "cargo build", target: triple };
    }
  }

  return {
    error:
      `No Cartograph binary was found for ${triple}.\n\n` +
      `Fix it in one of these ways:\n` +
      `  • download a release binary and set CARTOGRAPH_BINARY to its path\n` +
      `  • build from source:  cargo build --release -p cartograph-cli\n` +
      `  • install from source: cargo install --path crates/cartograph-cli\n\n` +
      `Releases: https://github.com/Rexy-5097/cartograph/releases`,
    target: triple,
  };
}

module.exports = { resolveBinary, target, binaryName, TARGETS };
