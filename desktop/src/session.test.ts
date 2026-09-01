/**
 * The session state machine.
 *
 * The Rust suite pins the *names* on both sides of the boundary. What it
 * cannot check is the part that only exists here: which transitions are legal.
 * Without these cases the frontend would be verified by `tsc` alone, and `tsc`
 * is happy with a reducer that returns the wrong state for every event.
 *
 * The invariants being defended:
 *
 * - a second analysis cannot start while one is running, because two results
 *   racing to populate one window is a bug the user sees as flicker and then
 *   as stale data;
 * - cancelling is not failing, and must not be shown as one;
 * - a late result from an abandoned run cannot revive itself.
 */

import { describe, expect, it } from "vitest";

import {
  type AnalysisPayload,
  type SessionState,
  initialState,
  isUserCorrectable,
  reduce,
} from "./session";

const payload: AnalysisPayload = {
  repository: "acme",
  summary: { files: 3, failed: 0, nodes: 4, edges: 3, clusters: 2 },
  scene: {
    nodes: [
      { id: 0, x: 0, y: 0, cluster: 0, label: "create_order", kind: "function" },
    ],
    edges: [],
    clusters: [{ id: 0, label: "api" }],
  },
};

/** Drives the reducer through a sequence, returning the final state. */
function run(...events: Parameters<typeof reduce>[1][]): SessionState {
  return events.reduce(reduce, initialState);
}

describe("the happy path", () => {
  it("starts idle", () => {
    expect(initialState.phase).toBe("idle");
  });

  it("goes idle → selecting → analyzing → ready", () => {
    const selecting = reduce(initialState, { type: "selectRequested" });
    expect(selecting.phase).toBe("selecting");

    const analyzing = reduce(selecting, {
      type: "analysisStarted",
      repository: "acme",
    });
    expect(analyzing).toEqual({ phase: "analyzing", repository: "acme" });

    const ready = reduce(analyzing, { type: "analysisSucceeded", payload });
    expect(ready).toEqual({ phase: "ready", payload });
  });

  it("can analyse a second repository after the first", () => {
    const ready = run(
      { type: "selectRequested" },
      { type: "analysisStarted", repository: "first" },
      { type: "analysisSucceeded", payload },
      { type: "selectRequested" },
    );
    expect(ready.phase).toBe("selecting");
  });
});

describe("refusing a second analysis", () => {
  it("ignores a selection while analysing", () => {
    const analyzing = run(
      { type: "selectRequested" },
      { type: "analysisStarted", repository: "acme" },
    );
    expect(reduce(analyzing, { type: "selectRequested" })).toBe(analyzing);
  });

  it("accepts a selection from every other state", () => {
    const states: SessionState[] = [
      { phase: "idle" },
      { phase: "selecting" },
      { phase: "ready", payload },
      { phase: "failed", error: { kind: "notFound", message: "gone" } },
      { phase: "cancelled" },
    ];
    for (const state of states) {
      expect(reduce(state, { type: "selectRequested" }).phase).toBe(
        "selecting",
      );
    }
  });
});

describe("cancelling", () => {
  it("is not a failure", () => {
    const cancelled = run(
      { type: "selectRequested" },
      { type: "selectionCancelled" },
    );
    expect(cancelled.phase).toBe("cancelled");
    expect(cancelled.phase).not.toBe("failed");
  });

  it("only applies while the dialog is open", () => {
    const ready: SessionState = { phase: "ready", payload };
    expect(reduce(ready, { type: "selectionCancelled" })).toBe(ready);
  });

  it("recovers to idle", () => {
    const cancelled = run(
      { type: "selectRequested" },
      { type: "selectionCancelled" },
    );
    expect(reduce(cancelled, { type: "reset" }).phase).toBe("idle");
  });
});

describe("late results", () => {
  /**
   * The reason `analysisSucceeded` checks the current phase. Without it, a
   * result from a run the user walked away from would populate the window
   * minutes later, and it would look like the application had drawn the wrong
   * repository.
   */
  it("cannot revive a session that moved on", () => {
    const cancelled: SessionState = { phase: "cancelled" };
    expect(reduce(cancelled, { type: "analysisSucceeded", payload })).toBe(
      cancelled,
    );

    const idle: SessionState = { phase: "idle" };
    expect(reduce(idle, { type: "analysisSucceeded", payload })).toBe(idle);
  });

  it("still reports a failure, because the user needs to know", () => {
    const error = { kind: "analysisFailed" as const, message: "broke" };
    const failed = reduce({ phase: "idle" }, { type: "analysisFailed", error });
    expect(failed).toEqual({ phase: "failed", error });
  });
});

describe("error classification", () => {
  it("marks the kinds a different folder could fix", () => {
    for (const kind of [
      "notFound",
      "notADirectory",
      "permissionDenied",
      "noSupportedSources",
    ] as const) {
      expect(isUserCorrectable(kind)).toBe(true);
    }
  });

  it("does not blame the user for a defect", () => {
    expect(isUserCorrectable("internal")).toBe(false);
    expect(isUserCorrectable("analysisFailed")).toBe(false);
  });
});
