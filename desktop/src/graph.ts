/**
 * Turning a Rust scene into a Graphology graph.
 *
 * # The one rule
 *
 * **Coordinates and clusters are used exactly as received.** No arithmetic, no
 * normalisation, no fallback layout. ADR-0001 places layout in the Rust core
 * and ADR-0006 keeps the renderer swappable *because* the frontend only ever
 * draws `{id, x, y, cluster}`; a scaling factor applied here would quietly
 * falsify both. `graph.test.ts` asserts bit-equality rather than approximate
 * equality for exactly that reason.
 *
 * What this file *may* decide is presentation — colour, size, label text.
 * Those are drawing choices and carry no claim about the system.
 *
 * # Why it returns diagnostics instead of throwing
 *
 * A malformed scene should not blank the window. Sigma is unforgiving in a
 * specific and nasty way: a node whose `x` is `NaN` does not raise, it
 * corrupts the camera's bounding-box reduction, and *every* node disappears.
 * One bad record would therefore cost the whole map with no error anywhere.
 *
 * So each record is checked before it is added, and anything rejected is
 * counted and explained rather than silently dropped. The window can then say
 * "9,998 of 10,000 nodes drawn" — which is honest — instead of showing an
 * empty canvas or, worse, a plausible-looking partial map.
 */

import Graph from "graphology";

import type { Cluster, EdgeKind, NodeKind, Scene } from "./session";

/** What could not be drawn, and why. */
export interface BuildDiagnostics {
  /** Nodes rejected before reaching the graph. */
  skippedNodes: number;
  /** Edges rejected before reaching the graph. */
  skippedEdges: number;
  /** One human-readable line per distinct reason, for the status area. */
  reasons: string[];
}

/** A built graph and an account of anything that did not make it. */
export interface BuildResult {
  graph: Graph;
  diagnostics: BuildDiagnostics;
}

/**
 * Cluster colours.
 *
 * A fixed, ordered palette indexed by cluster id. Deterministic by
 * construction: the same scene always produces the same colours, because the
 * cluster ids themselves are deterministic in Rust (ADR-0015). Chosen to stay
 * distinguishable against the dark canvas and to keep adjacent indices apart.
 */
const CLUSTER_PALETTE = [
  "#4f9dde",
  "#e0803c",
  "#5fb87a",
  "#c26ac4",
  "#d4b23f",
  "#5bc0be",
  "#e06a6a",
  "#8f8fd6",
  "#7fa650",
  "#c98aa0",
] as const;

/** The colour for a cluster. Presentation only. */
export function clusterColor(cluster: number): string {
  if (!Number.isFinite(cluster) || cluster < 0) {
    return "#8a8f98";
  }
  // `noUncheckedIndexedAccess` is on, so the modulo result is still an
  // `| undefined` to the compiler. The fallback is unreachable and is written
  // as a value rather than a `!` assertion.
  return CLUSTER_PALETTE[Math.floor(cluster) % CLUSTER_PALETTE.length] ?? "#8a8f98";
}

/**
 * Node size by kind.
 *
 * The kinds that answer "what is this system?" — a route, a table, an external
 * service — are drawn larger than the functions between them. This is a
 * legibility choice, not a claim about importance.
 */
const SIZE_BY_KIND: Partial<Record<NodeKind, number>> = {
  repository: 10,
  package: 8,
  route: 6,
  table: 6,
  "external-service": 6,
  class: 4,
  module: 4,
  file: 3,
};

function sizeFor(kind: NodeKind): number {
  return SIZE_BY_KIND[kind] ?? 2.5;
}

/**
 * Edge colours.
 *
 * The three cross-stack kinds are deliberately brighter than the rest: an HTTP
 * call from TypeScript to Python, an ORM access and a table query are the
 * claims Cartograph exists to make, and a reader should find them without
 * hunting.
 */
const EDGE_COLORS: Record<EdgeKind, string> = {
  "http-call": "#e0803c",
  "orm-access": "#5fb87a",
  queries: "#5bc0be",
  call: "#454b55",
  import: "#3a3f47",
  inherits: "#5a5f6a",
  implements: "#5a5f6a",
  references: "#3a3f47",
};

function edgeColor(kind: EdgeKind): string {
  return EDGE_COLORS[kind] ?? "#454b55";
}

/** A coordinate Sigma can survive: a real, finite number. */
function usable(value: unknown): value is number {
  return typeof value === "number" && Number.isFinite(value);
}

/**
 * Builds the Graphology graph a Sigma instance renders.
 *
 * Never throws on malformed input. A scene that is not an object, or whose
 * `nodes` is not an array, yields an empty graph and a diagnostic — the window
 * shows "nothing to draw" rather than a stack trace.
 */
export function buildGraph(scene: Scene | null | undefined): BuildResult {
  const graph = new Graph({ multi: true, type: "directed" });
  const reasons: string[] = [];
  let skippedNodes = 0;
  let skippedEdges = 0;

  const note = (reason: string) => {
    if (!reasons.includes(reason)) {
      reasons.push(reason);
    }
  };

  const nodes = Array.isArray(scene?.nodes) ? scene.nodes : [];
  const edges = Array.isArray(scene?.edges) ? scene.edges : [];
  if (!Array.isArray(scene?.nodes)) {
    if (scene != null) {
      note("the scene carried no node list");
    }
    return { graph, diagnostics: { skippedNodes, skippedEdges, reasons } };
  }

  for (const node of nodes) {
    const id = node?.id;
    if (typeof id !== "number" || !Number.isFinite(id)) {
      skippedNodes += 1;
      note("a node had no usable id");
      continue;
    }
    const key = String(id);
    if (graph.hasNode(key)) {
      skippedNodes += 1;
      note("a node id appeared more than once");
      continue;
    }
    // A non-finite coordinate does not fail loudly in Sigma — it collapses the
    // camera and hides every node. Rejecting one record is the cheaper loss.
    if (!usable(node.x) || !usable(node.y)) {
      skippedNodes += 1;
      note("a node arrived without a usable position");
      continue;
    }

    graph.addNode(key, {
      // Straight from Rust. Nothing between the wire and the renderer.
      x: node.x,
      y: node.y,
      cluster: node.cluster,
      label: typeof node.label === "string" ? node.label : key,
      kind: node.kind,
      size: sizeFor(node.kind),
      color: clusterColor(node.cluster),
    });
  }

  for (const edge of edges) {
    const id = edge?.id;
    const source = edge?.source;
    const target = edge?.target;
    if (typeof source !== "number" || typeof target !== "number") {
      skippedEdges += 1;
      note("an edge had no usable endpoints");
      continue;
    }
    const from = String(source);
    const to = String(target);
    // An endpoint the renderer never received would be a dangling reference.
    if (!graph.hasNode(from) || !graph.hasNode(to)) {
      skippedEdges += 1;
      note("an edge referenced a node that is not in the scene");
      continue;
    }
    const key = typeof id === "number" && Number.isFinite(id) ? `e${id}` : undefined;
    if (key !== undefined && graph.hasEdge(key)) {
      skippedEdges += 1;
      note("an edge id appeared more than once");
      continue;
    }

    const attributes = {
      kind: edge.kind,
      color: edgeColor(edge.kind),
      size: edge.kind === "http-call" || edge.kind === "orm-access" ? 1.4 : 0.7,
    };
    if (key === undefined) {
      graph.addEdge(from, to, attributes);
    } else {
      graph.addEdgeWithKey(key, from, to, attributes);
    }
  }

  return { graph, diagnostics: { skippedNodes, skippedEdges, reasons } };
}

/** The clusters present in a scene, largest first, for the legend. */
export function clusterSummary(
  scene: Scene | null | undefined,
): Array<Cluster & { count: number }> {
  const nodes = Array.isArray(scene?.nodes) ? scene.nodes : [];
  const clusters = Array.isArray(scene?.clusters) ? scene.clusters : [];
  return clusters
    .map((cluster) => ({
      ...cluster,
      count: nodes.filter((node) => node.cluster === cluster.id).length,
    }))
    .sort((a, b) => b.count - a.count || a.label.localeCompare(b.label));
}
