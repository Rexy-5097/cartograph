/**
 * The frontend half of the layout boundary.
 *
 * The Rust half lives in `crates/cartograph-desktop/tests/scene.rs`. Together
 * they establish one property end to end: **a coordinate computed in Rust
 * reaches the renderer unchanged.**
 *
 * Positions are compared with `toBe` — strict equality on the number — rather
 * than `toBeCloseTo`. That is the whole point. A tolerance would pass a
 * frontend transform that scaled everything by 1.0000001, and a frontend
 * transform is precisely what ADR-0001 and ADR-0006 forbid: the moment layout
 * can be adjusted here, "layout is computed in the Rust core" stops being true
 * of what is actually drawn, and swapping Sigma stops being contained.
 *
 * The second theme is that a malformed scene must not blank the window. Sigma
 * fails silently on a `NaN` coordinate — it does not raise, it collapses the
 * camera's bounding box and every node vanishes — so the checks happen before
 * anything reaches it, and the loss is reported rather than absorbed.
 */

import { describe, expect, it } from "vitest";

import { buildGraph, clusterColor, clusterSummary } from "./graph";
import type { Scene, SceneEdge, SceneNode } from "./session";

function node(over: Partial<SceneNode> = {}): SceneNode {
  return {
    id: 0,
    x: 0,
    y: 0,
    cluster: 0,
    label: "n",
    kind: "function",
    ...over,
  };
}

function edge(over: Partial<SceneEdge> = {}): SceneEdge {
  return { id: 0, source: 0, target: 1, kind: "call", ...over };
}

function scene(nodes: SceneNode[], edges: SceneEdge[] = []): Scene {
  return { nodes, edges, clusters: [{ id: 0, label: "api" }] };
}

describe("coordinates come from Rust", () => {
  it("uses the exact coordinate it was given", () => {
    // The case PART 17 names: Rust says x=10, y=20.
    const { graph } = buildGraph(scene([node({ id: 7, x: 10, y: 20 })]));

    expect(graph.getNodeAttribute("7", "x")).toBe(10);
    expect(graph.getNodeAttribute("7", "y")).toBe(20);
  });

  it("does not rescale, recentre or normalise a large layout", () => {
    const built = buildGraph(
      scene([
        node({ id: 0, x: -4823.5, y: 991.25 }),
        node({ id: 1, x: 7710.125, y: -3.5 }),
      ]),
    );

    expect(built.graph.getNodeAttribute("0", "x")).toBe(-4823.5);
    expect(built.graph.getNodeAttribute("0", "y")).toBe(991.25);
    expect(built.graph.getNodeAttribute("1", "x")).toBe(7710.125);
    expect(built.graph.getNodeAttribute("1", "y")).toBe(-3.5);
  });

  it("preserves coordinates that a normalising step would round away", () => {
    // Chosen so any multiply-then-divide would lose the last bits.
    const x = 0.1 + 0.2;
    const { graph } = buildGraph(scene([node({ id: 3, x, y: -x })]));

    expect(graph.getNodeAttribute("3", "x")).toBe(x);
    expect(graph.getNodeAttribute("3", "y")).toBe(-x);
  });

  it("takes cluster membership from Rust without reassigning it", () => {
    const { graph } = buildGraph(
      scene([node({ id: 0, cluster: 4 }), node({ id: 1, cluster: 9 })]),
    );

    expect(graph.getNodeAttribute("0", "cluster")).toBe(4);
    expect(graph.getNodeAttribute("1", "cluster")).toBe(9);
  });
});

describe("totality", () => {
  it("maps every node exactly once", () => {
    const nodes = [0, 1, 2, 3].map((id) => node({ id, x: id, y: id }));
    const { graph, diagnostics } = buildGraph(scene(nodes));

    expect(graph.order).toBe(4);
    expect(diagnostics.skippedNodes).toBe(0);
    for (const n of nodes) {
      expect(graph.hasNode(String(n.id))).toBe(true);
    }
  });

  it("maps every edge exactly once, with its endpoints", () => {
    const built = buildGraph(
      scene(
        [node({ id: 0 }), node({ id: 1 }), node({ id: 2 })],
        [
          edge({ id: 0, source: 0, target: 1 }),
          edge({ id: 1, source: 1, target: 2, kind: "http-call" }),
        ],
      ),
    );

    expect(built.graph.size).toBe(2);
    expect(built.diagnostics.skippedEdges).toBe(0);
    expect(built.graph.source("e0")).toBe("0");
    expect(built.graph.target("e0")).toBe("1");
    expect(built.graph.getEdgeAttribute("e1", "kind")).toBe("http-call");
  });

  it("keeps parallel edges between the same pair", () => {
    const built = buildGraph(
      scene(
        [node({ id: 0 }), node({ id: 1 })],
        [
          edge({ id: 0, source: 0, target: 1, kind: "call" }),
          edge({ id: 1, source: 0, target: 1, kind: "import" }),
        ],
      ),
    );

    // Two distinct claims about one pair are two edges, not one.
    expect(built.graph.size).toBe(2);
    expect(built.diagnostics.skippedEdges).toBe(0);
  });
});

describe("malformed input never blanks the window", () => {
  it("drops a NaN coordinate rather than losing every node to it", () => {
    const built = buildGraph(
      scene([
        node({ id: 0, x: Number.NaN, y: 0 }),
        node({ id: 1, x: 1, y: 1 }),
        node({ id: 2, x: 2, y: 2 }),
      ]),
    );

    // The survivors are what matters: one bad record must not cost the map.
    expect(built.graph.order).toBe(2);
    expect(built.graph.hasNode("0")).toBe(false);
    expect(built.diagnostics.skippedNodes).toBe(1);
    expect(built.diagnostics.reasons.join()).toContain("usable position");
  });

  it("drops infinite coordinates", () => {
    const built = buildGraph(
      scene([
        node({ id: 0, x: Number.POSITIVE_INFINITY, y: 0 }),
        node({ id: 1, x: 0, y: Number.NEGATIVE_INFINITY }),
        node({ id: 2, x: 5, y: 5 }),
      ]),
    );

    expect(built.graph.order).toBe(1);
    expect(built.diagnostics.skippedNodes).toBe(2);
  });

  it("keeps the first of a duplicated node id", () => {
    const built = buildGraph(
      scene([
        node({ id: 4, x: 1, y: 1, label: "first" }),
        node({ id: 4, x: 9, y: 9, label: "second" }),
      ]),
    );

    expect(built.graph.order).toBe(1);
    expect(built.graph.getNodeAttribute("4", "label")).toBe("first");
    expect(built.graph.getNodeAttribute("4", "x")).toBe(1);
    expect(built.diagnostics.skippedNodes).toBe(1);
  });

  it("drops an edge pointing at a node that is not in the scene", () => {
    const built = buildGraph(
      scene([node({ id: 0 })], [edge({ id: 0, source: 0, target: 999 })]),
    );

    expect(built.graph.size).toBe(0);
    expect(built.diagnostics.skippedEdges).toBe(1);
    expect(built.diagnostics.reasons.join()).toContain("not in the scene");
  });

  it("drops an edge whose endpoint was itself dropped", () => {
    // The compound case: the node is rejected for a bad coordinate, so every
    // edge touching it must go too, or Sigma gets a dangling endpoint.
    const built = buildGraph(
      scene(
        [node({ id: 0, x: Number.NaN }), node({ id: 1 })],
        [edge({ id: 0, source: 0, target: 1 })],
      ),
    );

    expect(built.graph.order).toBe(1);
    expect(built.graph.size).toBe(0);
    expect(built.diagnostics.skippedEdges).toBe(1);
  });

  it("keeps the first of a duplicated edge id", () => {
    const built = buildGraph(
      scene(
        [node({ id: 0 }), node({ id: 1 })],
        [
          edge({ id: 5, source: 0, target: 1, kind: "call" }),
          edge({ id: 5, source: 1, target: 0, kind: "import" }),
        ],
      ),
    );

    expect(built.graph.size).toBe(1);
    expect(built.graph.getEdgeAttribute("e5", "kind")).toBe("call");
    expect(built.diagnostics.skippedEdges).toBe(1);
  });

  it("survives an empty scene", () => {
    const built = buildGraph(scene([]));

    expect(built.graph.order).toBe(0);
    expect(built.graph.size).toBe(0);
    expect(built.diagnostics.skippedNodes).toBe(0);
  });

  it("survives a null or undefined scene", () => {
    expect(buildGraph(null).graph.order).toBe(0);
    expect(buildGraph(undefined).graph.order).toBe(0);
  });

  it("survives a payload whose shape is wrong entirely", () => {
    // What a corrupted IPC frame or a version mismatch would look like.
    const malformed = { nodes: "not an array", edges: null } as unknown as Scene;

    const built = buildGraph(malformed);

    expect(built.graph.order).toBe(0);
    expect(built.diagnostics.reasons.length).toBeGreaterThan(0);
  });

  it("survives records missing their fields", () => {
    const malformed = {
      nodes: [{}, { id: "seven" }, { id: 1, x: 1, y: 1 }],
      edges: [{}, { source: 1 }],
      clusters: [],
    } as unknown as Scene;

    const built = buildGraph(malformed);

    expect(built.graph.order).toBe(1);
    expect(built.graph.size).toBe(0);
    expect(built.diagnostics.skippedNodes).toBe(2);
    expect(built.diagnostics.skippedEdges).toBe(2);
  });
});

describe("determinism", () => {
  it("builds the same graph twice from the same scene", () => {
    const input = scene(
      [node({ id: 0, x: 1.5, y: -2.5 }), node({ id: 1, x: 3, y: 4 })],
      [edge({ id: 0, source: 0, target: 1 })],
    );

    const a = buildGraph(input);
    const b = buildGraph(input);

    expect(a.graph.export()).toEqual(b.graph.export());
  });

  it("gives a cluster the same colour every time", () => {
    expect(clusterColor(3)).toBe(clusterColor(3));
    expect(clusterColor(0)).not.toBe(clusterColor(1));
  });

  it("returns a readable colour for a nonsensical cluster", () => {
    expect(clusterColor(-1)).toMatch(/^#/);
    expect(clusterColor(Number.NaN)).toMatch(/^#/);
  });
});

describe("cluster summary", () => {
  it("counts members and orders by size", () => {
    const input: Scene = {
      nodes: [
        node({ id: 0, cluster: 0 }),
        node({ id: 1, cluster: 1 }),
        node({ id: 2, cluster: 1 }),
      ],
      edges: [],
      clusters: [
        { id: 0, label: "web" },
        { id: 1, label: "api" },
      ],
    };

    expect(clusterSummary(input)).toEqual([
      { id: 1, label: "api", count: 2 },
      { id: 0, label: "web", count: 1 },
    ]);
  });

  it("survives a missing cluster table", () => {
    expect(clusterSummary(null)).toEqual([]);
    expect(clusterSummary({ nodes: [], edges: [] } as unknown as Scene)).toEqual([]);
  });
});

describe("a real repository", () => {
  // Cartograph's own checkout, analysed by the real pipeline and composed by
  // the real `scene::compose`. Regenerate with:
  //   cargo test -p cartograph-desktop --test fixture_10k -- --ignored write_this_repositorys_scene
  //
  // This is the only place the two halves of the count check can meet: Rust
  // produced these records, and this asserts Graphology receives exactly them.
  it("maps every Rust node and edge into Graphology", async () => {
    const scene = (await import("./__fixtures__/scene-repo.json")).default as unknown as Scene;

    const built = buildGraph(scene);

    expect(scene.nodes.length).toBeGreaterThan(0);
    expect(built.graph.order).toBe(scene.nodes.length);
    expect(built.graph.size).toBe(scene.edges.length);
    expect(built.diagnostics.skippedNodes).toBe(0);
    expect(built.diagnostics.skippedEdges).toBe(0);

    // Every coordinate arrives unchanged — sampled across the whole fixture,
    // not just the first record.
    for (const node of scene.nodes) {
      const key = String(node.id);
      expect(built.graph.getNodeAttribute(key, "x")).toBe(node.x);
      expect(built.graph.getNodeAttribute(key, "y")).toBe(node.y);
      expect(built.graph.getNodeAttribute(key, "cluster")).toBe(node.cluster);
    }

    // Every edge keeps the endpoints Rust gave it.
    for (const edge of scene.edges) {
      expect(built.graph.source(`e${edge.id}`)).toBe(String(edge.source));
      expect(built.graph.target(`e${edge.id}`)).toBe(String(edge.target));
    }
  });
});
