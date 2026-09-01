/**
 * Finding an already-loaded node by its label.
 *
 * # Why this exists
 *
 * A node can be impossible to click. In the Airflow graph the class
 * `Connection` sits 0.19px from a larger neighbour at default zoom, and the
 * neighbour's Sigma hit area contains it at *every* zoom level — so no click
 * anywhere on the canvas can ever select it. Without a way to name a node, that
 * artefact is unreachable in the desktop, though the CLI addresses it by name
 * happily.
 *
 * # What this is not
 *
 * It is not analysis. It reads the labels of the scene already on screen and
 * returns matching ids; it does not traverse the graph, resolve symbols, rank
 * by relevance to anything semantic, or know what a blast radius is. The one
 * question it answers is "which loaded node did you mean?" — the caller then
 * uses the ordinary selection path, exactly as a click would.
 *
 * # Labels are not unique
 *
 * Airflow holds `Connection` (class), `connection` (table) and `connection`
 * (function). Matching is case-insensitive so a user who types either spelling
 * sees all three, and every match carries its `kind` so the choice is made
 * deliberately rather than by luck of ordering.
 */

import type { NodeKind, SceneNode } from "./session";

/** A node the user could mean, with enough context to tell it from its namesakes. */
export interface NodeMatch {
  /** Addresses the current scene only. Graph-local, like every other id here. */
  id: number;
  label: string;
  kind: NodeKind;
}

/**
 * How well a label answers the query. Lower sorts first.
 *
 * Ranked rather than filtered so an exact match is never buried under the
 * substring matches that happen to contain it: typing `Connection` puts
 * `Connection` above `to_connection`.
 */
function rank(label: string, query: string): number | null {
  if (label === query) {
    return 0;
  }
  const l = label.toLowerCase();
  const q = query.toLowerCase();
  if (l === q) {
    return 1;
  }
  if (l.startsWith(q)) {
    return 2;
  }
  if (l.includes(q)) {
    return 3;
  }
  return null;
}

/**
 * The loaded nodes whose label matches `query`, best first.
 *
 * Empty or whitespace-only input matches nothing — an empty box is not a
 * request for all 3,104 nodes.
 *
 * Ties break by label then id, so the same query over the same scene always
 * lists the same nodes in the same order. Determinism matters here for the same
 * reason it does everywhere else in Cartograph: a list that reshuffles between
 * renders makes the user's choice unrepeatable.
 */
export function findNodes(
  nodes: readonly SceneNode[],
  query: string,
): NodeMatch[] {
  const trimmed = query.trim();
  if (trimmed === "") {
    return [];
  }

  const scored: { match: NodeMatch; rank: number }[] = [];
  for (const node of nodes) {
    const r = rank(node.label, trimmed);
    if (r !== null) {
      scored.push({
        match: { id: node.id, label: node.label, kind: node.kind },
        rank: r,
      });
    }
  }

  scored.sort((a, b) => {
    if (a.rank !== b.rank) {
      return a.rank - b.rank;
    }
    if (a.match.label !== b.match.label) {
      return a.match.label < b.match.label ? -1 : 1;
    }
    return a.match.id - b.match.id;
  });

  return scored.map((entry) => entry.match);
}
