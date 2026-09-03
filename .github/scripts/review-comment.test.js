#!/usr/bin/env node
"use strict";

// Tests for the review-comment upsert.
//
// The helper's entire contract is: find Cartograph's own comment, fit the
// review inside GitHub's limit without breaking Markdown, and issue one
// request. These check exactly that, and nothing about analysis — analysis is
// the engine's job and is tested in Rust.
//
// The GitHub API is stubbed at `fetch`, which is the boundary this code owns.
// That is deliberately *not* a substitute for the real check: whether the
// Action runs on a Cartograph pull request and updates its own comment is
// established on a real pull request, because a stub cannot tell you that a
// token had the right scope or that an event carried the right SHA.

const assert = require("assert");
const fs = require("fs");
const os = require("os");
const path = require("path");

const {
  MARKER,
  GITHUB_COMMENT_LIMIT,
  findReviewComment,
  fitToLimit,
  redact,
  upsert,
} = require("./review-comment.js");

let failures = 0;
function test(name, fn) {
  try {
    const result = fn();
    if (result && typeof result.then === "function") {
      return result.then(
        () => console.log(`ok   ${name}`),
        (error) => {
          failures += 1;
          console.error(`FAIL ${name}\n     ${error.message}`);
        },
      );
    }
    console.log(`ok   ${name}`);
  } catch (error) {
    failures += 1;
    console.error(`FAIL ${name}\n     ${error.message}`);
  }
  return Promise.resolve();
}

/// Records every request instead of making one.
function stubFetch(responses) {
  const calls = [];
  global.fetch = async (url, options) => {
    calls.push({ url, method: options.method, body: options.body, headers: options.headers });
    const next = responses.shift();
    if (!next) {
      throw new Error(`unexpected request: ${options.method} ${url}`);
    }
    return {
      ok: next.status < 400,
      status: next.status,
      json: async () => next.body,
      text: async () => JSON.stringify(next.body ?? {}),
    };
  };
  return calls;
}

const REVIEW = "## Cartograph — architecture review\n\n**1 added · 0 removed · 0 changed**, with 5 unchanged.\n";

async function run() {
  // ── Marker detection ────────────────────────────────────────────────

  await test("finds the comment carrying the marker", () => {
    const found = findReviewComment([
      { id: 1, body: "looks good to me" },
      { id: 2, body: `${MARKER}\nreview` },
    ]);
    assert.strictEqual(found.id, 2);
  });

  await test("returns null when no review comment exists", () => {
    assert.strictEqual(
      findReviewComment([{ id: 1, body: "please rebase" }]),
      null,
    );
  });

  await test("never selects a human comment that merely mentions Cartograph", () => {
    const found = findReviewComment([
      { id: 1, body: "the cartograph architecture review says otherwise" },
      { id: 2, body: "## Cartograph — architecture review (quoted by hand)" },
    ]);
    assert.strictEqual(found, null, "prose must not be mistaken for the marker");
  });

  await test("selection is deterministic when a duplicate exists", () => {
    const comments = [
      { id: 9, body: `${MARKER}\nsecond` },
      { id: 4, body: `${MARKER}\nfirst` },
    ];
    assert.strictEqual(findReviewComment(comments).id, 4);
    assert.strictEqual(findReviewComment([...comments].reverse()).id, 4);
  });

  await test("tolerates a comment with no body", () => {
    assert.strictEqual(findReviewComment([{ id: 1 }, { id: 2, body: null }]), null);
  });

  // ── Size handling ───────────────────────────────────────────────────

  await test("a short review is posted whole, with the marker", () => {
    const { body, truncated } = fitToLimit(REVIEW);
    assert.ok(body.startsWith(MARKER));
    assert.ok(body.includes("1 added"));
    assert.strictEqual(truncated, false);
  });

  await test("an oversized review is brought under the limit", () => {
    const row = "| a | b | `call` | 0.90 | `x.py:1` |";
    const huge = `${REVIEW}${`${row}\n`.repeat(4000)}`;
    assert.ok(huge.length > GITHUB_COMMENT_LIMIT, "precondition: must be oversized");

    const { body, truncated, omitted } = fitToLimit(huge);
    assert.ok(
      body.length <= GITHUB_COMMENT_LIMIT,
      `body is ${body.length}, over the limit`,
    );
    assert.strictEqual(truncated, true);
    assert.ok(omitted > 0);
  });

  await test("truncation says so, rather than quietly showing less", () => {
    const huge = `${REVIEW}${"| a | b | c | d | e |\n".repeat(5000)}`;
    const { body } = fitToLimit(huge);
    assert.ok(body.includes("This review was shortened"));
    assert.ok(body.includes("more"), "it must say how much was omitted");
  });

  await test("truncation never cuts a table row in half", () => {
    const row = "| alpha | beta | `call` | 0.90 | `path/to/file.py:12` |";
    const huge = `${REVIEW}${`${row}\n`.repeat(4000)}`;
    const { body } = fitToLimit(huge);

    for (const line of body.split("\n")) {
      if (!line.startsWith("| ")) {
        continue;
      }
      assert.ok(
        line.endsWith("|"),
        `a row was cut mid-line, which misaligns the table: ${line}`,
      );
    }
  });

  await test("truncation is deterministic", () => {
    const huge = `${REVIEW}${"| a | b | c | d | e |\n".repeat(5000)}`;
    assert.strictEqual(fitToLimit(huge).body, fitToLimit(huge).body);
  });

  await test("a review that is exactly at the limit is not truncated", () => {
    const room = GITHUB_COMMENT_LIMIT - MARKER.length - 1;
    const exact = "x".repeat(room);
    const { body, truncated } = fitToLimit(exact);
    assert.strictEqual(truncated, false);
    assert.strictEqual(body.length, GITHUB_COMMENT_LIMIT);
  });

  // ── Create and update ───────────────────────────────────────────────

  await test("creates a comment when none exists", async () => {
    const calls = stubFetch([
      { status: 200, body: [{ id: 1, body: "unrelated" }] },
      { status: 201, body: { id: 77 } },
    ]);

    const result = await upsert({
      api: "https://api.github.com",
      repository: "o/r",
      pull: "5",
      token: "t",
      review: REVIEW,
    });

    assert.strictEqual(result.action, "created");
    assert.strictEqual(calls[1].method, "POST");
    assert.ok(calls[1].url.endsWith("/repos/o/r/issues/5/comments"));
  });

  await test("updates the existing comment instead of appending a second", async () => {
    const calls = stubFetch([
      { status: 200, body: [{ id: 3, body: `${MARKER}\nold` }] },
      { status: 200, body: { id: 3 } },
    ]);

    const result = await upsert({
      api: "https://api.github.com",
      repository: "o/r",
      pull: "5",
      token: "t",
      review: REVIEW,
    });

    assert.strictEqual(result.action, "updated");
    assert.strictEqual(result.id, 3);
    assert.strictEqual(calls[1].method, "PATCH");
    assert.ok(calls[1].url.endsWith("/repos/o/r/issues/comments/3"));
    assert.strictEqual(calls.length, 2, "exactly one write");
  });

  await test("leaves unrelated comments untouched", async () => {
    const calls = stubFetch([
      {
        status: 200,
        body: [
          { id: 1, body: "please add a test" },
          { id: 2, body: `${MARKER}\nold` },
          { id: 3, body: "shipping this" },
        ],
      },
      { status: 200, body: { id: 2 } },
    ]);

    await upsert({
      api: "https://api.github.com",
      repository: "o/r",
      pull: "5",
      token: "t",
      review: REVIEW,
    });

    assert.ok(calls[1].url.endsWith("/issues/comments/2"));
    assert.strictEqual(calls.length, 2, "no other comment was written to");
  });

  // ── Failure behaviour ───────────────────────────────────────────────

  await test("an API failure is reported, not swallowed", async () => {
    stubFetch([{ status: 403, body: { message: "Resource not accessible" } }]);

    await assert.rejects(
      upsert({
        api: "https://api.github.com",
        repository: "o/r",
        pull: "5",
        token: "t",
        review: REVIEW,
      }),
      /403/,
    );
  });

  await test("a failure message carries no token", async () => {
    stubFetch([{ status: 401, body: { message: "Bad credentials" } }]);

    try {
      await upsert({
        api: "https://api.github.com",
        repository: "o/r",
        pull: "5",
        token: "ghs_supersecrettokenvalue",
        review: REVIEW,
      });
      assert.fail("should have thrown");
    } catch (error) {
      assert.ok(
        !error.message.includes("ghs_supersecrettokenvalue"),
        `the token leaked into an error: ${error.message}`,
      );
    }
  });

  await test("the token travels in a header, never in the URL", async () => {
    const calls = stubFetch([
      { status: 200, body: [] },
      { status: 201, body: { id: 1 } },
    ]);

    await upsert({
      api: "https://api.github.com",
      repository: "o/r",
      pull: "5",
      token: "ghs_secret",
      review: REVIEW,
    });

    for (const call of calls) {
      assert.ok(!call.url.includes("ghs_secret"), `token in URL: ${call.url}`);
      assert.strictEqual(call.headers.authorization, "Bearer ghs_secret");
    }
  });

  await test("redaction removes a credential-shaped query parameter", () => {
    assert.strictEqual(
      redact("https://api.github.com/x?access_token=abc123&page=2"),
      "https://api.github.com/x?access_token=REDACTED&page=2",
    );
  });

  // ── The posted body ─────────────────────────────────────────────────

  await test("the posted body carries the marker so the next run finds it", async () => {
    const calls = stubFetch([
      { status: 200, body: [] },
      { status: 201, body: { id: 1 } },
    ]);

    await upsert({
      api: "https://api.github.com",
      repository: "o/r",
      pull: "5",
      token: "t",
      review: REVIEW,
    });

    const posted = JSON.parse(calls[1].body).body;
    assert.ok(posted.includes(MARKER));
    assert.strictEqual(
      findReviewComment([{ id: 1, body: posted }]).id,
      1,
      "a comment this helper posts must be findable by this helper",
    );
  });

  await test("reads the review from a file as the workflow passes it", () => {
    const file = path.join(
      os.tmpdir(),
      `cartograph-review-${process.pid}.md`,
    );
    fs.writeFileSync(file, REVIEW, "utf8");
    try {
      assert.strictEqual(fs.readFileSync(file, "utf8"), REVIEW);
    } finally {
      fs.unlinkSync(file);
    }
  });

  if (failures > 0) {
    console.error(`\n${failures} test(s) failed`);
    process.exit(1);
  }
  console.log("\nall review-comment tests passed");
}

run();
