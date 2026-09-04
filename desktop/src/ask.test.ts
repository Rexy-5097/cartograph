/**
 * Degraded ASK presentation: faithful wording, faithful order, no network.
 *
 * The panel's job is to say what Rust found. These tests pin the ways that can
 * go wrong quietly: an entry dropped, an order improved on, evidence tidied,
 * or a state described from an assumption rather than from the answer.
 */

import { afterEach, describe, expect, it, vi } from "vitest";

import { aiWording, askSummary, isDegraded, orderedEntries } from "./ask";
import type { AskAnswer, EvidenceRecord } from "./session";

/** Evidence with spacing and punctuation a "tidy-up" would damage. */
const AWKWARD =
  "GET /api/orders  matched  GET /api/orders (list_orders); neither side declared a method.";

function entry(edge: number, evidence: string): EvidenceRecord {
  return {
    analysis: 1,
    edge,
    kind: "http-call",
    source: { id: 0, label: "caller", kind: "function" },
    target: { id: 1, label: "handler", kind: "function" },
    confidence: 0.784,
    calibrated: false,
    provenance: "route-matcher",
    deterministic: true,
    evidence,
    location: { file: "api/routes.py", line: 6, column: null },
  };
}

function answerOf(entries: EvidenceRecord[]): AskAnswer {
  return { analysis: 1, target: 0, ai: "disabled", scope: 42, entries };
}

afterEach(() => {
  vi.unstubAllGlobals();
});

describe("the AI state", () => {
  it("describes a disabled model from the answer", () => {
    expect(aiWording("disabled")).toBe(
      "No model was consulted — this is the derived evidence.",
    );
    expect(isDegraded(answerOf([]))).toBe(true);
  });

  // RULE 009 applied to wording, as `kindWording` already does it: a state
  // this build has never heard of must show as itself rather than silently
  // reading as "no model was consulted", which would misdescribe the source
  // of an answer.
  it("shows an unrecognised state rather than inventing wording for it", () => {
    expect(aiWording("enabled")).toBe("enabled");
    expect(aiWording("degraded-by-quota")).toBe("degraded-by-quota");
  });

  it("does not call an answer degraded merely because it is empty", () => {
    const enabled = { ...answerOf([]), ai: "enabled" as unknown as "disabled" };
    expect(isDegraded(enabled)).toBe(false);
  });
});

describe("entries", () => {
  it("renders every entry it was given", () => {
    const answer = answerOf([entry(1, "one"), entry(2, "two"), entry(3, "three")]);
    expect(orderedEntries(answer)).toHaveLength(3);
  });

  it("keeps Rust's order rather than sorting", () => {
    // Deliberately not in any order this file could be tempted to "fix".
    const answer = answerOf([entry(9, "c"), entry(2, "a"), entry(5, "b")]);
    expect(orderedEntries(answer).map((e) => e.edge)).toEqual([9, 2, 5]);
  });

  it("leaves evidence text exactly as received", () => {
    const answer = answerOf([entry(1, AWKWARD)]);
    const only = orderedEntries(answer)[0];
    expect(only?.evidence).toBe(AWKWARD);
    expect(only?.evidence).toContain("  matched  ");
    expect(only?.evidence.endsWith(".")).toBe(true);
  });

  it("adds no entry of its own", () => {
    const answer = answerOf([]);
    expect(orderedEntries(answer)).toEqual([]);
  });
});

describe("summary", () => {
  it("says so when nothing leads away from the artefact", () => {
    expect(askSummary(answerOf([]))).toBe(
      "No derived relationships lead away from this artefact.",
    );
  });

  it("counts one and many", () => {
    expect(askSummary(answerOf([entry(1, "a")]))).toBe("1 derived relationship.");
    expect(askSummary(answerOf([entry(1, "a"), entry(2, "b")]))).toBe(
      "2 derived relationships.",
    );
  });

  it("never quotes the evidence in the summary", () => {
    const summary = askSummary(answerOf([entry(1, AWKWARD)]));
    expect(summary).not.toContain("/api/orders");
  });
});

describe("offline", () => {
  it("issues no network request", () => {
    const fetched = vi.fn();
    vi.stubGlobal("fetch", fetched);

    const answer = answerOf([entry(1, AWKWARD), entry(2, "second")]);
    aiWording(answer.ai);
    askSummary(answer);
    isDegraded(answer);
    orderedEntries(answer);

    expect(fetched).not.toHaveBeenCalled();
  });

  it("renders no filesystem root and invents no source text", () => {
    const answer = answerOf([entry(1, AWKWARD)]);
    const shown = [
      aiWording(answer.ai),
      askSummary(answer),
      ...orderedEntries(answer).map((e) => `${e.evidence}|${e.location.file}`),
    ].join("\n");

    for (const marker of ["/home/", "/Users/", "AppData", ":\\", "runner/work"]) {
      expect(shown).not.toContain(marker);
    }
    // The location is repository-relative, which Rust guarantees at
    // construction; this pins that the UI does not prepend anything to it.
    expect(shown).toContain("api/routes.py");
    expect(shown.startsWith("/")).toBe(false);
  });
});
