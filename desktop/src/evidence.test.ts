/**
 * Evidence presentation: faithful wording, and no invented positions.
 *
 * The panel's job is to say what Rust found. These tests pin the two ways that
 * can go wrong quietly: an unrecognised value being replaced by a friendlier
 * guess, and a missing column being defaulted to one that was never observed.
 */

import { describe, expect, it } from "vitest";

import { formatLocation, kindWording, provenanceWording } from "./evidence";

describe("wording", () => {
  it("reads known kinds and provenances as prose", () => {
    expect(kindWording("http-call")).toBe("calls over HTTP");
    expect(kindWording("orm-access")).toBe("accesses via ORM");
    expect(provenanceWording("route-matcher")).toBe("Route matcher");
    expect(provenanceWording("orm-resolution")).toBe("ORM resolution");
  });

  // RULE 009 applied to wording: an unknown value is shown, not guessed at.
  // A new EdgeKind in Rust must surface as itself rather than silently
  // becoming a neighbouring phrase.
  it("shows an unrecognised value rather than inventing wording for it", () => {
    expect(kindWording("teleports-to")).toBe("teleports-to");
    expect(provenanceWording("quantum-inference")).toBe("quantum-inference");
  });

  it("never returns an empty string for a value it was given", () => {
    for (const value of ["import", "call", "queries", "unknown-thing"]) {
      expect(kindWording(value).length).toBeGreaterThan(0);
    }
  });
});

describe("source location", () => {
  it("renders file and line", () => {
    expect(
      formatLocation({ file: "api/routes.py", line: 8, column: null }),
    ).toBe("api/routes.py:8");
  });

  it("includes the column when the analysis recorded one", () => {
    expect(
      formatLocation({ file: "web/lib/api.ts", line: 42, column: 11 }),
    ).toBe("web/lib/api.ts:42:11");
  });

  // A fabricated column is a fabricated source position — it would point the
  // reader at a place nothing was observed.
  it("does not default a missing column to 1", () => {
    const rendered = formatLocation({
      file: "api/models.py",
      line: 5,
      column: null,
    });

    expect(rendered).toBe("api/models.py:5");
    expect(rendered).not.toContain(":5:1");
  });

  it("treats an absent column the same as a null one", () => {
    expect(
      formatLocation({
        file: "a.py",
        line: 1,
      } as unknown as Parameters<typeof formatLocation>[0]),
    ).toBe("a.py:1");
  });

  it("keeps the repository-relative path exactly as received", () => {
    // The panel must not prettify a path: what Rust recorded is what the
    // reader needs in order to find the line.
    const file = "packages/web/src/features/checkout/api.ts";
    expect(formatLocation({ file, line: 120, column: null })).toBe(
      `${file}:120`,
    );
  });
});
