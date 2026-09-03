#!/usr/bin/env node
"use strict";

// Posts Cartograph's architecture review onto a pull request.
//
// The binary produced the review; this only delivers it. Analysis and
// rendering live in Rust (RULE 002, and M14's Action "runs the binary in the
// user's own CI" per ROADMAP.md), so nothing here reads a graph, computes a
// diff, or formats a table. It reads text on disk, decides whether a comment
// already exists, and issues one HTTP request.
//
// Dependency-free on purpose, exactly like `npm/bin/cartograph.js`: a comment
// upsert does not justify a framework, and a workflow with no lockfile cannot
// have one poisoned. Node 20 supplies `fetch`.

const fs = require("fs");

/// Hidden in rendered Markdown, stable across runs, and specific enough that
/// it cannot collide with something a person wrote. This is the whole basis of
/// idempotency: find this, update that comment; otherwise create one.
const MARKER = "<!-- cartograph-architecture-review -->";

/// GitHub refuses a comment body over 65,536 characters. Posting one is a hard
/// failure, so the review is trimmed to fit rather than the run breaking.
const GITHUB_COMMENT_LIMIT = 65536;

/// Room kept for the marker and the truncation notice, so trimming can never
/// itself push the body back over the limit.
const RESERVE = 1024;

/// Finds Cartograph's own comment among a page of issue comments.
///
/// Matching is on the marker alone, never on the author or on prose: a bot
/// name can change, and a person quoting the review must not be mistaken for
/// it. The lowest id wins so that a duplicate created by some earlier mishap
/// resolves to the same comment on every subsequent run rather than
/// alternating between them.
function findReviewComment(comments) {
  const mine = comments.filter(
    (c) => typeof c.body === "string" && c.body.includes(MARKER),
  );
  if (mine.length === 0) {
    return null;
  }
  return mine.reduce((lowest, c) => (c.id < lowest.id ? c : lowest));
}

/// Fits a review inside GitHub's comment limit without breaking Markdown.
///
/// Whole lines are kept or dropped, never split. Every table row is one line,
/// so a row can never be cut in half and silently misalign the columns after
/// it. What is dropped is always a suffix, which keeps the result deterministic
/// for a given review.
///
/// A truncated comment says so. Silently showing a shorter review would let a
/// reader conclude a relationship was not reported when it merely did not fit.
function fitToLimit(review, limit = GITHUB_COMMENT_LIMIT) {
  const body = `${MARKER}\n${review}`;
  if (body.length <= limit) {
    return { body, truncated: false, omitted: 0 };
  }

  const lines = review.split("\n");
  const budget = limit - MARKER.length - RESERVE;
  const kept = [];
  let used = 0;

  for (const line of lines) {
    // +1 for the newline this line will be joined with.
    if (used + line.length + 1 > budget) {
      break;
    }
    kept.push(line);
    used += line.length + 1;
  }

  const omitted = lines.length - kept.length;
  const notice =
    `\n> **This review was shortened.** ${omitted} more ` +
    `${omitted === 1 ? "line was" : "lines were"} omitted because GitHub ` +
    `limits a comment to ${GITHUB_COMMENT_LIMIT} characters. Run ` +
    "`cartograph diff <before> <after> --markdown` locally for the whole review.\n";

  return {
    body: `${MARKER}\n${kept.join("\n")}\n${notice}`,
    truncated: true,
    omitted,
  };
}

/// One GitHub REST call, with the token supplied by the caller.
///
/// The token is read from the environment and placed in a header. It is never
/// logged, never interpolated into a message, and never written to a file
/// (RULE 015). A failing request reports its status and GitHub's message, both
/// of which are safe to print; the request body is not echoed.
async function request(method, url, token, body) {
  const response = await fetch(url, {
    method,
    headers: {
      accept: "application/vnd.github+json",
      authorization: `Bearer ${token}`,
      "content-type": "application/json",
      "user-agent": "cartograph-architecture-review",
      "x-github-api-version": "2022-11-28",
    },
    body: body === undefined ? undefined : JSON.stringify(body),
  });

  if (!response.ok) {
    const detail = await response.text().catch(() => "");
    let message = "";
    try {
      message = JSON.parse(detail).message ?? "";
    } catch {
      message = "";
    }
    throw new Error(
      `GitHub API ${method} ${redact(url)} failed: ${response.status}` +
        (message ? ` — ${message}` : ""),
    );
  }

  return response.status === 204 ? null : response.json();
}

/// Removes anything credential-shaped from a URL before it reaches a log.
///
/// The URLs used here carry none, but a URL is exactly the kind of value that
/// acquires a query parameter later, and a redaction added afterwards is one
/// that was missing when it mattered.
function redact(url) {
  return String(url).replace(/([?&](?:access_token|token)=)[^&]*/gi, "$1REDACTED");
}

/// Creates or updates the review comment.
async function upsert({ api, repository, pull, token, review }) {
  const { body, truncated, omitted } = fitToLimit(review);
  const base = `${api}/repos/${repository}`;

  // One page of 100 is enough in practice, and asking for more pages would
  // mean more calls against a rate limit for a comment that is nearly always
  // recent. If it is ever not enough, a second comment appears rather than
  // silent corruption of someone else's.
  const comments = await request(
    "GET",
    `${base}/issues/${pull}/comments?per_page=100`,
    token,
  );
  const existing = findReviewComment(comments);

  if (existing) {
    await request("PATCH", `${base}/issues/comments/${existing.id}`, token, {
      body,
    });
    return { action: "updated", id: existing.id, truncated, omitted };
  }

  const created = await request(
    "POST",
    `${base}/issues/${pull}/comments`,
    token,
    { body },
  );
  return { action: "created", id: created.id, truncated, omitted };
}

async function main() {
  const token = process.env.GITHUB_TOKEN;
  const repository = process.env.GITHUB_REPOSITORY;
  const pull = process.env.PR_NUMBER;
  const file = process.env.REVIEW_FILE;
  const api = process.env.GITHUB_API_URL || "https://api.github.com";

  for (const [name, value] of [
    ["GITHUB_TOKEN", token],
    ["GITHUB_REPOSITORY", repository],
    ["PR_NUMBER", pull],
    ["REVIEW_FILE", file],
  ]) {
    // Reports which variable is missing, never what any of them contain.
    if (!value) {
      throw new Error(`${name} is not set`);
    }
  }

  const review = fs.readFileSync(file, "utf8");
  const result = await upsert({ api, repository, pull, token, review });

  console.log(
    `${result.action} comment ${result.id}` +
      (result.truncated ? ` (shortened; ${result.omitted} lines omitted)` : ""),
  );
}

module.exports = {
  MARKER,
  GITHUB_COMMENT_LIMIT,
  findReviewComment,
  fitToLimit,
  redact,
  upsert,
};

if (require.main === module) {
  main().catch((error) => {
    console.error(error.message);
    process.exit(1);
  });
}
