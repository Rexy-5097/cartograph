/**
 * Edge interaction: the settings and the id path the evidence panel depends on.
 *
 * # Why this file exists
 *
 * M11 made an edge interrogable — click a relationship, see the evidence behind
 * it. That interaction was silently dead: `SIGMA_SETTINGS` never set
 * `enableEdgeEvents`, and Sigma 3.0.3 gates its whole edge branch on it:
 *
 *     const nodeAtPosition = this.getNodeAtPosition(event);
 *     if (nodeAtPosition) return this.emit(eventType + "Node", ...);
 *     if (this.settings.enableEdgeEvents) {
 *       const edge = this.getEdgeAtPoint(event.x, event.y);
 *       if (edge) return this.emit(eventType + "Edge", ...);
 *     }
 *     return this.emit(eventType + "Stage", baseEvent);
 *
 * With the flag off, every click that is not on a node falls through to
 * `clickStage` — which the window treats as "clear the selection". So clicking
 * an edge did the opposite of selecting it, and no test caught it because the
 * setting was never asserted.
 *
 * # What can and cannot be tested here
 *
 * Sigma needs WebGL and this suite runs in Node, so dispatching a real click
 * through a renderer is not possible in a unit test, and the project has no
 * component-testing library. What is pinned here is everything that does not
 * need a GPU: the setting itself, and the key/id translation that carries a
 * click to `edge_evidence`. The click-to-panel path is verified in the real
 * Windows window instead, and that is recorded in the M11 remediation report
 * rather than implied by these tests.
 */

import { describe, expect, it } from "vitest";

import { SIGMA_SETTINGS } from "./sigmaSettings";
import { edgeKey, nodeKey } from "./blast";
import { buildGraph } from "./graph";
import type { Scene, SceneEdge, SceneNode } from "./session";

function node(over: Partial<SceneNode> = {}): SceneNode {
  return { id: 0, x: 0, y: 0, cluster: 0, label: "n", kind: "function", ...over };
}

function edge(over: Partial<SceneEdge> = {}): SceneEdge {
  return { id: 0, source: 0, target: 1, kind: "call", ...over };
}

function scene(): Scene {
  return {
    nodes: [
      node({ id: 0, x: 1, y: 2, cluster: 0, label: "caller" }),
      node({ id: 1, x: 3, y: 4, cluster: 1, label: "callee" }),
      node({ id: 2, x: 5, y: 6, cluster: 1, label: "model", kind: "class" }),
    ],
    edges: [
      edge({ id: 7, source: 0, target: 1, kind: "call" }),
      edge({ id: 42, source: 1, target: 2, kind: "orm-access" }),
    ],
    clusters: [
      { id: 0, label: "web" },
      { id: 1, label: "api" },
    ],
  };
}

/** Sigma's own dispatch, reproduced from 3.0.3, over the settings we ship. */
function dispatch(
  settings: { enableEdgeEvents?: boolean },
  hit: { node?: string; edge?: string },
): string {
  if (hit.node !== undefined) {
    return "clickNode";
  }
  if (settings.enableEdgeEvents === true && hit.edge !== undefined) {
    return "clickEdge";
  }
  return "clickStage";
}

describe("Sigma settings", () => {
  // The regression itself. If this is ever false again, edge evidence dies.
  it("enables edge events, without which clickEdge never fires", () => {
    expect(SIGMA_SETTINGS.enableEdgeEvents).toBe(true);
  });

  it("keeps the settings the benchmark and the window share", () => {
    // The benchmark imports this same object; a divergence would mean the
    // measured renderer is not the shipped one.
    expect(SIGMA_SETTINGS.minCameraRatio).toBe(0.02);
    expect(SIGMA_SETTINGS.zIndex).toBe(true);
  });
});

describe("what a click resolves to", () => {
  it("routes a click on an edge to clickEdge once events are enabled", () => {
    expect(dispatch(SIGMA_SETTINGS, { edge: "e42" })).toBe("clickEdge");
  });

  // The defect, pinned so it cannot come back silently.
  it("would fall through to clickStage with edge events off", () => {
    expect(dispatch({ enableEdgeEvents: false }, { edge: "e42" })).toBe(
      "clickStage",
    );
  });

  it("still prefers a node when one is under the cursor", () => {
    expect(dispatch(SIGMA_SETTINGS, { node: "1", edge: "e42" })).toBe(
      "clickNode",
    );
  });

  it("still reports empty space as clickStage", () => {
    expect(dispatch(SIGMA_SETTINGS, {})).toBe("clickStage");
  });
});

describe("the id a click hands to edge_evidence", () => {
  /** Exactly the translation GraphView performs in its clickEdge handler. */
  const idFromKey = (key: string) => Number.parseInt(key.replace(/^e/, ""), 10);

  it("recovers the Rust EdgeId from the key Sigma reports", () => {
    expect(idFromKey("e42")).toBe(42);
    expect(idFromKey(edgeKey(7))).toBe(7);
  });

  it("names an edge that is actually in the rendered graph", () => {
    const { graph } = buildGraph(scene());

    for (const key of graph.edges()) {
      const id = idFromKey(key);
      expect(Number.isFinite(id)).toBe(true);
      expect(graph.hasEdge(edgeKey(id))).toBe(true);
    }
  });

  it("resolves to the exact edge, with its own endpoints and kind", () => {
    const { graph } = buildGraph(scene());
    const key = edgeKey(42);

    expect(graph.source(key)).toBe(nodeKey(1));
    expect(graph.target(key)).toBe(nodeKey(2));
    expect(graph.getEdgeAttribute(key, "kind")).toBe("orm-access");
    // and not the other edge's identity
    expect(graph.getEdgeAttribute(edgeKey(7), "kind")).toBe("call");
  });

  it("does not confuse an edge id with a node id of the same number", () => {
    const { graph } = buildGraph(scene());
    expect(graph.hasNode(nodeKey(0))).toBe(true);
    expect(graph.hasEdge(edgeKey(0))).toBe(false);
  });
});

describe("selecting an edge is not a graph mutation", () => {
  /**
   * Selection lives entirely in React state — it calls `edge_evidence` and
   * stores the record. Nothing touches the graph. These assert the invariant a
   * future "highlight the selected edge" change would have to preserve.
   */
  it("leaves the element counts alone", () => {
    const { graph } = buildGraph(scene());
    const order = graph.order;
    const size = graph.size;

    const key = edgeKey(42);
    graph.getEdgeAttribute(key, "kind");

    expect(graph.order).toBe(order);
    expect(graph.size).toBe(size);
  });

  it("leaves every coordinate and cluster untouched", () => {
    const { graph } = buildGraph(scene());
    const before = graph.mapNodes((k) => [
      k,
      graph.getNodeAttribute(k, "x"),
      graph.getNodeAttribute(k, "y"),
      graph.getNodeAttribute(k, "cluster"),
    ]);

    graph.source(edgeKey(42));
    graph.target(edgeKey(42));

    expect(
      graph.mapNodes((k) => [
        k,
        graph.getNodeAttribute(k, "x"),
        graph.getNodeAttribute(k, "y"),
        graph.getNodeAttribute(k, "cluster"),
      ]),
    ).toEqual(before);
  });
});
