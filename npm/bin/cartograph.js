#!/usr/bin/env node
"use strict";

// Launch the native binary and get out of the way.
//
// Everything about this file is about *not* interfering: argv is forwarded
// unchanged, stdio is inherited so the child owns the terminal directly
// (progress on stderr, JSON on stdout, both unbuffered), and the child's exit
// code becomes this process's exit code. A wrapper that reinterpreted any of
// those would break the exit-code contract in docs/development/cli.md.

const { spawn } = require("child_process");
const { resolveBinary } = require("../lib/resolve.js");

const resolved = resolveBinary();

if (resolved.error) {
  process.stderr.write(`error: ${resolved.error}\n`);
  // 3 is the documented "input or environment could not be used" code.
  process.exit(3);
}

const child = spawn(resolved.path, process.argv.slice(2), {
  stdio: "inherit",
  windowsHide: true,
});

child.on("error", (error) => {
  process.stderr.write(`error: could not run ${resolved.path}: ${error.message}\n`);
  process.exit(3);
});

child.on("exit", (code, signal) => {
  if (signal) {
    // Reproduce the signal so a shell reports Ctrl-C as Ctrl-C rather than as
    // an ordinary non-zero exit.
    process.kill(process.pid, signal);
    return;
  }
  process.exit(code === null ? 3 : code);
});
