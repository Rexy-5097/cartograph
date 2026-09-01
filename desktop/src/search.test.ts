/**
 * Finding a node by name.
 *
 * These pin the behaviour the desktop was unblocked by: the Airflow graph holds
 * three artefacts whose labels differ only by case and kind, and picking the
 * wrong one silently answers a different question.
 */

import { describe, expect, it } from "vitest";

import { findNodes } from "./search";
import type { SceneNode } from "./session";

function node(over: Partial<SceneNode> = {}): SceneNode {
  return { id: 0, x: 0, y: 0, cluster: 0, label: "n", kind: "function", ...over };
}

/** The real Airflow collision, reduced to the part that matters. */
function airflow(): SceneNode[] {
  return [
    node({ id: 1177, label: "connection", kind: "table" }),
    node({ id: 1176, label: "Connection", kind: "class" }),
    node({ id: 2832, label: "connection", kind: "function" }),
    node({ id: 3001, label: "to_connection", kind: "function" }),
    node({ id: 4002, label: "Widget", kind: "class" }),
  ];
}

describe("findNodes", () => {
  it("finds a node by its exact label", () => {
    const found = findNodes(airflow(), "Widget");
    expect(found).toEqual([{ id: 4002, label: "Widget", kind: "class" }]);
  });

  it("returns every namesake, each distinguishable by kind", () => {
    const found = findNodes(airflow(), "Connection");

    expect(found.map((m) => `${m.label} · ${m.kind}`)).toEqual([
      "Connection · class",
      // Same label, so these tie-break by id: 1177 before 2832.
      "connection · table",
      "connection · function",
      "to_connection · function",
    ]);
  });

  // The whole point: the exactly-typed one must not be buried.
  it("puts the case-exact match first", () => {
    expect(findNodes(airflow(), "Connection")[0]).toEqual({
      id: 1176,
      label: "Connection",
      kind: "class",
    });
  });

  it("still finds the class when the query is lowercase", () => {
    const found = findNodes(airflow(), "connection");
    expect(found.some((m) => m.id === 1176)).toBe(true);
  });

  it("distinguishes the three namesakes by id", () => {
    const found = findNodes(airflow(), "connection").filter(
      (m) => m.label.toLowerCase() === "connection",
    );
    expect(found.map((m) => m.id).sort()).toEqual([1176, 1177, 2832]);
  });

  it("says nothing matched rather than guessing", () => {
    expect(findNodes(airflow(), "NoSuchArtefact")).toEqual([]);
  });

  // An empty box is not a request for the whole graph.
  it("matches nothing on empty or whitespace input", () => {
    expect(findNodes(airflow(), "")).toEqual([]);
    expect(findNodes(airflow(), "   ")).toEqual([]);
  });

  it("ignores surrounding whitespace in the query", () => {
    expect(findNodes(airflow(), "  Widget  ")).toHaveLength(1);
  });

  it("is deterministic: the same scene and query give the same order", () => {
    expect(findNodes(airflow(), "connection")).toEqual(
      findNodes(airflow(), "connection"),
    );
  });

  /**
   * Every id it offers must be a node of the scene it was given. The caller
   * feeds these straight into the ordinary selection path, so an id from
   * anywhere else would be a stale selection the user could not have made by
   * clicking.
   */
  it("only ever returns ids present in the scene", () => {
    const scene = airflow();
    const ids = new Set(scene.map((n) => n.id));

    for (const query of ["connection", "e", "W", "to_"]) {
      for (const match of findNodes(scene, query)) {
        expect(ids.has(match.id)).toBe(true);
      }
    }
  });

  it("finds nothing in an empty scene", () => {
    expect(findNodes([], "Connection")).toEqual([]);
  });
});
