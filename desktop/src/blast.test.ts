/**
 * Blast radius highlighting: a derived view that adds nothing.
 *
 * Graphology runs headless, so these drive the real graph the renderer draws
 * rather than a stand-in. What they check is that highlighting is *only*
 * presentation: it paints what Rust listed, it never invents impact, and above
 * all it never moves a node — the layout belongs to Rust (ADR-0001), and a
 * highlight that nudged a coordinate would break that quietly.
 */

import { describe, expect, it } from "vitest";

import {
  applyBlastHighlight,
  clearBlastHighlight,
  describeBlast,
  edgeKey,
  impactedEdgeKeys,
  impactedNodeKeys,
  nodeKey,
} from "./blast";
import { buildGraph, clusterColor, edgeColor } from "./graph";
import type { BlastResult, Scene, SceneEdge, SceneNode } from "./session";

function node(over: Partial<SceneNode> = {}): SceneNode {
  return { id: 0, x: 0, y: 0, cluster: 0, label: "n", kind: "function", ...over };
}

function edge(over: Partial<SceneEdge> = {}): SceneEdge {
  return { id: 0, source: 0, target: 1, kind: "call", ...over };
}

/** `a -> b -> c`: c is the interesting target. */
function scene(): Scene {
  return {
    nodes: [
      node({ id: 0, x: 1, y: 2, cluster: 0, label: "a" }),
      node({ id: 1, x: 3, y: 4, cluster: 1, label: "b" }),
      node({ id: 2, x: 5, y: 6, cluster: 2, label: "c", kind: "class" }),
      node({ id: 3, x: 7, y: 8, cluster: 0, label: "unrelated" }),
    ],
    edges: [
      edge({ id: 0, source: 0, target: 1, kind: "call" }),
      edge({ id: 1, source: 1, target: 2, kind: "orm-access" }),
      edge({ id: 2, source: 3, target: 3, kind: "import" }),
    ],
    clusters: [
      { id: 0, label: "web" },
      { id: 1, label: "api" },
      { id: 2, label: "models" },
    ],
  };
}

/** What Rust would return for target `c`. Not recomputed here. */
function result(over: Partial<BlastResult> = {}): BlastResult {
  return {
    analysis: 1,
    target: 2,
    reached: [
      { node: 1, depth: 1, confidence: 0.8, via: 1 },
      { node: 0, depth: 2, confidence: 0.8, via: 0 },
    ],
    calibrated: false,
    routes: "representative",
    ...over,
  };
}

describe("keys", () => {
  it("maps Rust ids to the graph's keys", () => {
    expect(nodeKey(7)).toBe("7");
    expect(edgeKey(7)).toBe("e7");
  });

  it("reads the impacted sets straight from the result", () => {
    expect(impactedNodeKeys(result())).toEqual(new Set(["1", "0"]));
    expect(impactedEdgeKeys(result())).toEqual(new Set(["e1", "e0"]));
  });

  it("treats no result as nothing impacted", () => {
    expect(impactedNodeKeys(null).size).toBe(0);
    expect(impactedEdgeKeys(null).size).toBe(0);
  });
});

describe("highlighting", () => {
  it("marks every impacted node and no others", () => {
    const { graph } = buildGraph(scene());
    applyBlastHighlight(graph, result());

    const impacted = graph.getNodeAttribute("1", "color");
    expect(graph.getNodeAttribute("0", "color")).toBe(impacted);
    // The unrelated node is neither target nor impacted.
    expect(graph.getNodeAttribute("3", "color")).not.toBe(impacted);
    expect(graph.getNodeAttribute("3", "color")).not.toBe(
      graph.getNodeAttribute("2", "color"),
    );
  });

  it("keeps the target visually distinct from the artefacts it impacts", () => {
    const { graph } = buildGraph(scene());
    applyBlastHighlight(graph, result());

    expect(graph.getNodeAttribute("2", "color")).not.toBe(
      graph.getNodeAttribute("1", "color"),
    );
    // And larger, because colour alone will not find one node among hundreds.
    expect(graph.getNodeAttribute("2", "size")).toBeGreaterThan(
      graph.getNodeAttribute("1", "size"),
    );
  });

  it("never places the target among the impacted", () => {
    const built = buildGraph(scene());
    applyBlastHighlight(built.graph, result());

    expect(impactedNodeKeys(result()).has("2")).toBe(false);
  });

  it("highlights the representative route edges and dims the rest", () => {
    const { graph } = buildGraph(scene());
    applyBlastHighlight(graph, result());

    const highlighted = graph.getEdgeAttribute("e0", "color");
    expect(graph.getEdgeAttribute("e1", "color")).toBe(highlighted);
    expect(graph.getEdgeAttribute("e2", "color")).not.toBe(highlighted);
    expect(graph.getEdgeAttribute("e0", "size")).toBeGreaterThan(
      graph.getEdgeAttribute("e2", "size"),
    );
  });

  // The property that matters most: the layout is Rust's.
  it("never moves a node", () => {
    const { graph } = buildGraph(scene());
    const before = graph.mapNodes((key) => [
      key,
      graph.getNodeAttribute(key, "x"),
      graph.getNodeAttribute(key, "y"),
      graph.getNodeAttribute(key, "cluster"),
    ]);

    applyBlastHighlight(graph, result());
    clearBlastHighlight(graph);
    applyBlastHighlight(graph, result());

    const after = graph.mapNodes((key) => [
      key,
      graph.getNodeAttribute(key, "x"),
      graph.getNodeAttribute(key, "y"),
      graph.getNodeAttribute(key, "cluster"),
    ]);
    expect(after).toEqual(before);
  });

  it("adds and removes no nodes or edges", () => {
    const { graph } = buildGraph(scene());
    const order = graph.order;
    const size = graph.size;

    applyBlastHighlight(graph, result());

    expect(graph.order).toBe(order);
    expect(graph.size).toBe(size);
  });

  it("is idempotent, so a re-render cannot accumulate state", () => {
    const { graph } = buildGraph(scene());
    applyBlastHighlight(graph, result());
    const once = graph.getNodeAttribute("2", "size");

    applyBlastHighlight(graph, result());

    expect(graph.getNodeAttribute("2", "size")).toBe(once);
  });

  it("survives a result naming something the scene does not hold", () => {
    const { graph } = buildGraph(scene());

    expect(() =>
      applyBlastHighlight(graph, result({ reached: [{ node: 999, depth: 1, confidence: 0.5, via: 999 }] })),
    ).not.toThrow();
  });

  it("paints an empty result as a lone target", () => {
    const { graph } = buildGraph(scene());
    applyBlastHighlight(graph, result({ reached: [] }));

    const target = graph.getNodeAttribute("2", "color");
    for (const key of ["0", "1", "3"]) {
      expect(graph.getNodeAttribute(key, "color")).not.toBe(target);
    }
  });
});

describe("clearing", () => {
  it("restores exactly what a fresh build produces", () => {
    const input = scene();
    const { graph } = buildGraph(input);
    const fresh = buildGraph(input).graph;

    applyBlastHighlight(graph, result());
    clearBlastHighlight(graph);

    for (const key of graph.nodes()) {
      expect(graph.getNodeAttribute(key, "color")).toBe(
        fresh.getNodeAttribute(key, "color"),
      );
      expect(graph.getNodeAttribute(key, "size")).toBe(
        fresh.getNodeAttribute(key, "size"),
      );
    }
    for (const key of graph.edges()) {
      expect(graph.getEdgeAttribute(key, "color")).toBe(
        fresh.getEdgeAttribute(key, "color"),
      );
    }
  });

  it("restores colours derived from cluster and kind, not from a cache", () => {
    const { graph } = buildGraph(scene());
    applyBlastHighlight(graph, result());
    clearBlastHighlight(graph);

    expect(graph.getNodeAttribute("0", "color")).toBe(clusterColor(0));
    expect(graph.getNodeAttribute("2", "color")).toBe(clusterColor(2));
    expect(graph.getEdgeAttribute("e1", "color")).toBe(edgeColor("orm-access"));
  });

  // A second query must replace the first, not merge with it.
  it("replacing a result leaves no trace of the previous one", () => {
    const { graph } = buildGraph(scene());
    applyBlastHighlight(graph, result());

    // Now target `b` instead: only `a` depends on it.
    applyBlastHighlight(
      graph,
      result({ target: 1, reached: [{ node: 0, depth: 1, confidence: 0.9, via: 0 }] }),
    );

    const target = graph.getNodeAttribute("1", "color");
    const impacted = graph.getNodeAttribute("0", "color");
    // `c`, the previous target, must no longer be highlighted at all.
    expect(graph.getNodeAttribute("2", "color")).not.toBe(target);
    expect(graph.getNodeAttribute("2", "color")).not.toBe(impacted);
    expect(graph.getEdgeAttribute("e1", "color")).not.toBe(
      graph.getEdgeAttribute("e0", "color"),
    );
  });
});

describe("summary", () => {
  it("counts and reports the deepest hop", () => {
    expect(describeBlast(result())).toBe(
      "2 artefacts depend on this, up to 2 hops away.",
    );
  });

  it("says plainly when nothing depends on the artefact", () => {
    expect(describeBlast(result({ reached: [] }))).toBe(
      "Nothing depends on this artefact.",
    );
  });

  it("reads correctly for a single dependent", () => {
    expect(
      describeBlast(result({ reached: [{ node: 1, depth: 1, confidence: 0.8, via: 1 }] })),
    ).toBe("1 artefact depends on this, up to 1 hop away.");
  });

  it("has nothing to say without a result", () => {
    expect(describeBlast(null)).toBeNull();
  });
});
