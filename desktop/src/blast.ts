/**
 * Blast radius as a derived visual state over the existing scene.
 *
 * # This file computes nothing about the repository
 *
 * There is no traversal here, no reachability, no confidence arithmetic and no
 * edge-kind policy. Rust decided all of that
 * (`cartograph_graph::blast::blast_radius`, ADR-0018); what arrives is a list,
 * and this file paints it.
 *
 * # Why highlighting mutates attributes instead of rebuilding
 *
 * `GraphView` memoises `buildGraph` on the scene's identity, so a rebuild
 * means re-creating ten thousand nodes. A blast radius is a *view* of the same
 * scene, not a different scene — so it sets attributes on the graph already
 * loaded and Sigma redraws. Nothing here touches `x`, `y` or `cluster`: the
 * layout is Rust's (ADR-0001) and a highlight must not move anything.
 *
 * # Restoring, without remembering
 *
 * Clearing recomputes each base colour from the attributes already on the
 * graph — `cluster` for a node, `kind` for an edge — rather than stashing the
 * previous palette somewhere. A saved copy is a second source of truth that
 * goes stale the moment a scene is replaced mid-highlight.
 */

import type Graph from "graphology";

import { clusterColor, edgeColor, edgeSize, nodeSize } from "./graph";
import type { BlastResult, NodeKind } from "./session";

/** The target itself. Deliberately unlike any cluster colour. */
const TARGET_COLOR = "#ffffff";
/** An artefact that depends on the target. */
const IMPACTED_COLOR = "#e0803c";
/** The route an impacted artefact reaches the target through. */
const IMPACTED_EDGE_COLOR = "#e0803c";
/** Everything outside the blast radius, pushed back rather than hidden. */
const DIMMED_NODE_COLOR = "#2f343c";
const DIMMED_EDGE_COLOR = "#23262b";

/** Graphology's key for a Rust `NodeId`. */
export function nodeKey(id: number): string {
  return String(id);
}

/** Graphology's key for a Rust `EdgeId`. */
export function edgeKey(id: number): string {
  return `e${id}`;
}

/** The nodes a blast result marks as impacted, as Graphology keys. */
export function impactedNodeKeys(result: BlastResult | null): Set<string> {
  if (result === null) {
    return new Set();
  }
  return new Set(result.reached.map((entry) => nodeKey(entry.node)));
}

/**
 * The edges a blast result marks as impacted.
 *
 * One per impacted artefact — the representative route Rust chose. This is
 * *not* every edge between impacted nodes, and the interface says so, because
 * highlighting more would imply the whole subgraph was reported when it was
 * not.
 */
export function impactedEdgeKeys(result: BlastResult | null): Set<string> {
  if (result === null) {
    return new Set();
  }
  return new Set(result.reached.map((entry) => edgeKey(entry.via)));
}

/**
 * Paints a blast result onto the graph already loaded.
 *
 * Idempotent: applying twice leaves the same attributes, so a re-render cannot
 * accumulate state. Safe against a result naming something the scene does not
 * hold — that would be a disagreement between window and graph, and skipping
 * is better than throwing inside a render.
 */
export function applyBlastHighlight(graph: Graph, result: BlastResult): void {
  const impactedNodes = impactedNodeKeys(result);
  const impactedEdges = impactedEdgeKeys(result);
  const target = nodeKey(result.target);

  graph.forEachNode((key) => {
    if (key === target) {
      graph.setNodeAttribute(key, "color", TARGET_COLOR);
      // The target is also enlarged: colour alone is not enough to find one
      // node among several hundred highlighted ones.
      graph.setNodeAttribute(
        key,
        "size",
        baseNodeSize(graph, key) * 2.2,
      );
      graph.setNodeAttribute(key, "zIndex", 3);
      return;
    }
    if (impactedNodes.has(key)) {
      graph.setNodeAttribute(key, "color", IMPACTED_COLOR);
      graph.setNodeAttribute(key, "size", baseNodeSize(graph, key));
      graph.setNodeAttribute(key, "zIndex", 2);
      return;
    }
    graph.setNodeAttribute(key, "color", DIMMED_NODE_COLOR);
    graph.setNodeAttribute(key, "size", baseNodeSize(graph, key));
    graph.setNodeAttribute(key, "zIndex", 0);
  });

  graph.forEachEdge((key) => {
    if (impactedEdges.has(key)) {
      graph.setEdgeAttribute(key, "color", IMPACTED_EDGE_COLOR);
      graph.setEdgeAttribute(key, "size", 1.8);
      graph.setEdgeAttribute(key, "zIndex", 2);
      return;
    }
    graph.setEdgeAttribute(key, "color", DIMMED_EDGE_COLOR);
    graph.setEdgeAttribute(key, "size", 0.5);
    graph.setEdgeAttribute(key, "zIndex", 0);
  });
}

/**
 * Restores the scene's own colours.
 *
 * Recomputed from `cluster` and `kind`, which are the attributes `buildGraph`
 * derived them from in the first place, so this cannot drift from what a fresh
 * build would produce.
 */
export function clearBlastHighlight(graph: Graph): void {
  graph.forEachNode((key, attributes) => {
    const cluster = attributes["cluster"] as number | undefined;
    graph.setNodeAttribute(key, "color", clusterColor(cluster ?? -1));
    graph.setNodeAttribute(key, "size", baseNodeSize(graph, key));
    graph.setNodeAttribute(key, "zIndex", 0);
  });

  graph.forEachEdge((key, attributes) => {
    const kind = attributes["kind"] as string | undefined;
    graph.setEdgeAttribute(key, "color", edgeColor(kind ?? ""));
    graph.setEdgeAttribute(key, "size", edgeSize(kind ?? ""));
    graph.setEdgeAttribute(key, "zIndex", 0);
  });
}

/**
 * A node's size from its kind, not from whatever it is currently set to.
 *
 * Reading the live `size` would compound: enlarging the target twice would
 * make it 4.84 times its base rather than 2.2.
 */
function baseNodeSize(graph: Graph, key: string): number {
  const kind = graph.getNodeAttribute(key, "kind") as NodeKind | undefined;
  return nodeSize(kind);
}

/** A one-line summary of what a blast result found, for the status area. */
export function describeBlast(result: BlastResult | null): string | null {
  if (result === null) {
    return null;
  }
  if (result.reached.length === 0) {
    return "Nothing depends on this artefact.";
  }
  const depth = Math.max(...result.reached.map((entry) => entry.depth));
  const count = result.reached.length;
  return `${count} artefact${count === 1 ? "" : "s"} depend${
    count === 1 ? "s" : ""
  } on this, up to ${depth} hop${depth === 1 ? "" : "s"} away.`;
}
