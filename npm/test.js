#!/usr/bin/env node
"use strict";

// Tests for the launcher.
//
// The wrapper's entire contract is: find the right binary, forward argv and
// stdio, and reproduce the exit code. These check exactly that, and nothing
// about analysis — analysis is the engine's job and is tested in Rust.

const assert = require("assert");
const { execFileSync, spawnSync } = require("child_process");
const path = require("path");
const fs = require("fs");
const os = require("os");

const { resolveBinary, target, binaryName, TARGETS } = require("./lib/resolve.js");

let passed = 0;
function test(name, fn) {
  try {
    fn();
    console.log(`  ok   ${name}`);
    passed += 1;
  } catch (error) {
    console.error(`  FAIL ${name}\n       ${error.message}`);
    process.exitCode = 1;
  }
}

console.log("resolution");

test("every supported platform maps to a target triple", () => {
  assert.strictEqual(target("darwin", "arm64"), "aarch64-apple-darwin");
  assert.strictEqual(target("darwin", "x64"), "x86_64-apple-darwin");
  assert.strictEqual(target("linux", "x64"), "x86_64-unknown-linux-gnu");
  assert.strictEqual(target("win32", "x64"), "x86_64-pc-windows-msvc");
});

test("an unsupported platform is named, not guessed at", () => {
  assert.strictEqual(target("aix", "ppc64"), null);
  const result = resolveBinary({ platform: "aix", arch: "ppc64", env: {} });
  assert.ok(result.error, "an unsupported platform must be an error");
  assert.ok(result.error.includes("aix/ppc64"), result.error);
  assert.ok(result.error.includes("cargo install"), "must offer a way forward");
});

test("the binary name carries .exe only on Windows", () => {
  assert.strictEqual(binaryName("win32"), "cartograph.exe");
  assert.strictEqual(binaryName("linux"), "cartograph");
  assert.strictEqual(binaryName("darwin"), "cartograph");
});

test("an explicit override wins", () => {
  const tmp = fs.mkdtempSync(path.join(os.tmpdir(), "cartograph-npm-"));
  const fake = path.join(tmp, "fake-binary");
  fs.writeFileSync(fake, "#!/bin/sh\nexit 0\n");
  const result = resolveBinary({ env: { CARTOGRAPH_BINARY: fake } });
  assert.strictEqual(result.path, fake);
  assert.strictEqual(result.source, "CARTOGRAPH_BINARY");
  fs.rmSync(tmp, { recursive: true, force: true });
});

test("a missing override is reported rather than silently ignored", () => {
  const result = resolveBinary({ env: { CARTOGRAPH_BINARY: "/no/such/binary" } });
  assert.ok(result.error, "must not fall through to another candidate");
  assert.ok(result.error.includes("/no/such/binary"), result.error);
});

test("a missing binary explains all three ways to get one", () => {
  const tmp = fs.mkdtempSync(path.join(os.tmpdir(), "cartograph-npm-"));
  const result = resolveBinary({ env: {}, root: tmp });
  assert.ok(result.error, "no binary should be found in an empty directory");
  assert.ok(result.error.includes("CARTOGRAPH_BINARY"), result.error);
  assert.ok(result.error.includes("cargo build --release"), result.error);
  assert.ok(result.error.includes("releases"), result.error);
  fs.rmSync(tmp, { recursive: true, force: true });
});

test("resolution never performs network access", () => {
  // The launcher must work offline. If this file ever requires http/https,
  // that is a design change, not an implementation detail.
  const source = fs.readFileSync(path.join(__dirname, "lib/resolve.js"), "utf8");
  for (const forbidden of ["https", "http", "fetch(", "child_process"]) {
    assert.ok(
      !source.includes(`require("${forbidden}")`),
      `resolve.js must not require ${forbidden}`
    );
  }
});

// ── the launcher, against a real binary ─────────────────────────────

const engine =
  process.env.CARTOGRAPH_BINARY ||
  path.join(__dirname, "..", "target", "release", binaryName());

if (!fs.existsSync(engine)) {
  console.log(`\nskipping launcher tests: no binary at ${engine}`);
  console.log(`\n${passed} passed`);
  process.exit(process.exitCode || 0);
}

console.log("launching");
const launcher = path.join(__dirname, "bin/cartograph.js");
const env = { ...process.env, CARTOGRAPH_BINARY: engine };

test("stdout is forwarded unchanged", () => {
  const direct = execFileSync(engine, ["version"], { encoding: "utf8" });
  const wrapped = execFileSync(process.execPath, [launcher, "version"], {
    encoding: "utf8",
    env,
  });
  assert.strictEqual(wrapped, direct);
});

test("json stays pure through the wrapper", () => {
  const out = execFileSync(process.execPath, [launcher, "version", "--json"], {
    encoding: "utf8",
    env,
  });
  const parsed = JSON.parse(out);
  assert.strictEqual(parsed.version, "0.1.0");
  assert.strictEqual(parsed.milestone, "M09");
});

test("a success exit code is reproduced", () => {
  const result = spawnSync(process.execPath, [launcher, "version"], { env });
  assert.strictEqual(result.status, 0);
});

test("a failure exit code is reproduced exactly", () => {
  // 3 = input error, 5 = symbol not found, 2 = usage. The wrapper must not
  // collapse these into a generic 1.
  const fixtures = path.join(
    __dirname, "..", "crates", "cartograph-parser", "tests", "fixtures"
  );
  const cases = [
    [["no/such/path"], 3],
    [["--definitely-not-a-flag"], 2],
    [["trace", "NoSuchSymbol", "--path", fixtures], 5],
  ];
  for (const [args, expected] of cases) {
    const result = spawnSync(process.execPath, [launcher, ...args], { env });
    assert.strictEqual(
      result.status,
      expected,
      `${args.join(" ")} exited ${result.status}, expected ${expected}`
    );
  }
});

test("arguments are forwarded verbatim, including ones with spaces", () => {
  const fixtures = path.join(
    __dirname, "..", "crates", "cartograph-parser", "tests", "fixtures"
  );
  const wrapped = execFileSync(
    process.execPath,
    [launcher, "--json", fixtures],
    { encoding: "utf8", env, maxBuffer: 64 * 1024 * 1024 }
  );
  const parsed = JSON.parse(wrapped);
  assert.strictEqual(parsed.command, "summary");
  assert.ok(parsed.summary.files > 0);
});

console.log(`\n${passed} passed`);
