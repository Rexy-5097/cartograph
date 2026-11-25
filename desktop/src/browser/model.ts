/**
 * Pure helpers for the browser explorer (M18 experiment).
 *
 * Everything here rearranges data the Rust side produced. Nothing decides
 * whether two artefacts are related, where anything is drawn, or what a
 * request matches: those answers arrive in the scene, the report, a trace or
 * a blast radius, and are only grouped, counted or highlighted here.
 */

import type { AnalysisReport, RequestDecision, TraceHop, TraceResult } from "./api";
import type { EdgeKind, NodeKind, Scene } from "../session";

/**
 * Where a node sits in the stack, read from its declaring file's language.
 *
 * Presentational only: a `.ts` function is drawn as frontend because its file
 * is TypeScript, not because anything established it is a client. Nodes with
 * no file fall back to their kind.
 */
export type Layer = "frontend" | "api" | "backend" | "model" | "database" | "external" | "other";

export const LAYER_LABEL: Record<Layer, string> = {
  frontend: "Frontend (TS/JS)",
  api: "API route",
  backend: "Backend (Python)",
  model: "Python class",
  database: "Database table",
  external: "External service",
  other: "Other",
};

/**
 * Node colours by layer. Chosen apart from the renderer's edge colours
 * (`graph.ts`: HTTP orange, ORM green, queries teal, calls grey), so a node
 * is never mistaken for a relationship.
 */
export const LAYER_COLOR: Record<Layer, string> = {
  frontend: "#6ab0ff",
  api: "#ffd166",
  backend: "#b39ddb",
  model: "#f48fb1",
  database: "#ff8a65",
  external: "#cfd8dc",
  other: "#6b7280",
};

export const EDGE_LABEL: Record<EdgeKind, string> = {
  "http-call": "HTTP call",
  "orm-access": "ORM access",
  queries: "Queries table",
  call: "Calls",
  import: "Imports",
  inherits: "Inherits",
  implements: "Implements",
  references: "References",
};

const FRONTEND_EXT = /\.(tsx?|jsx?|mjs|cjs|vue)$/i;
const BACKEND_EXT = /\.py$/i;

/** The layer of one node, from its kind and its file. */
export function layerOf(kind: NodeKind, file: string | undefined): Layer {
  if (kind === "table" || kind === "column") {
    return "database";
  }
  if (kind === "route") {
    return "api";
  }
  if (kind === "external-service") {
    return "external";
  }
  if (kind === "class" && file !== undefined && BACKEND_EXT.test(file)) {
    return "model";
  }
  if (file !== undefined && FRONTEND_EXT.test(file)) {
    return "frontend";
  }
  if (file !== undefined && BACKEND_EXT.test(file)) {
    return "backend";
  }
  return "other";
}

/** Node id → declared file and line, from the report. */
export function locations(
  report: AnalysisReport | null,
): Map<number, { file?: string; line?: number }> {
  const out = new Map<number, { file?: string; line?: number }>();
  for (const node of report?.nodes ?? []) {
    out.set(node.id, { file: node.file, line: node.line });
  }
  return out;
}

/** Node id → layer, for every node in the scene. */
export function layers(scene: Scene, report: AnalysisReport | null): Map<number, Layer> {
  const where = locations(report);
  const out = new Map<number, Layer>();
  for (const node of scene.nodes) {
    out.set(node.id, layerOf(node.kind, where.get(node.id)?.file));
  }
  return out;
}

/** How many of each value occur, in descending order of count. */
export function tally<T extends string>(values: Iterable<T>): [T, number][] {
  const counts = new Map<T, number>();
  for (const value of values) {
    counts.set(value, (counts.get(value) ?? 0) + 1);
  }
  return [...counts.entries()].sort((a, b) => b[1] - a[1] || a[0].localeCompare(b[0]));
}

/** The edge kinds present in a scene, with counts. */
export function edgeKinds(scene: Scene): [EdgeKind, number][] {
  return tally(scene.edges.map((e) => e.kind));
}

/** The node kinds present in a scene, with counts. */
export function nodeKinds(scene: Scene): [NodeKind, number][] {
  return tally(scene.nodes.map((n) => n.kind));
}

/** Request decisions grouped by status, refusals first. */
export function groupDecisions(
  decisions: readonly RequestDecision[],
): { status: RequestDecision["status"]; items: RequestDecision[] }[] {
  const order: RequestDecision["status"][] = [
    "ambiguous",
    "unsupported",
    "no-match",
    "other",
    "strong",
    "exact",
  ];
  return order
    .map((status) => ({ status, items: decisions.filter((d) => d.status === status) }))
    .filter((group) => group.items.length > 0);
}

/** What each refusal status means, in plain words. */
export const STATUS_MEANING: Record<RequestDecision["status"], string> = {
  exact:
    "Accepted: exactly one compatible route, with a declared method and every path position determined. The graph gets an edge for it.",
  strong:
    "Accepted: exactly one compatible route, with something undetermined (an undeclared method or an unknown path value). The graph gets an edge for it.",
  ambiguous:
    "Not decided, so no edge: several routes remain equally plausible, or the best one would only match by accepting an unknown value or a whole class of paths. This is not evidence that no connection exists.",
  "no-match":
    "No compatible route in the analysed code — none with this path, or none whose method fits. Cartograph drew no edge. The route may live outside the repository, be generated at runtime (for example by a router), or be declared in a way Cartograph does not read.",
  unsupported:
    "Not compared: the request could not be compared safely — its path is not a fixed pattern, its prefix is unknown, or it names an absolute host (so it may address a third party, not this application). No edge was drawn.",
  other: "A decision kind this interface does not know. Shown as reported.",
};

/** Words for why a trace stopped. */
export const STOP_MEANING: Record<NonNullable<TraceHop["stop"]>, string> = {
  "chain-end": "the artefact has no outgoing relationship Cartograph established",
  "max-depth": "the depth limit was reached; the chain may continue",
  cycle: "the walk returned to an artefact already on this path",
};

/** Every node and edge a trace visits, as Graphology keys. */
export function traceKeys(trace: TraceResult | null): {
  nodes: Set<string>;
  edges: Set<string>;
} {
  const nodes = new Set<string>();
  const edges = new Set<string>();
  if (trace === null) {
    return { nodes, edges };
  }
  nodes.add(String(trace.from));
  const visit = (hops: readonly TraceHop[]) => {
    for (const hop of hops) {
      edges.add(`e${hop.edge}`);
      nodes.add(String(hop.to));
      visit(hop.next);
    }
  };
  visit(trace.steps);
  return { nodes, edges };
}

/** One path of a trace, from its start to where that branch stops. */
export interface TracePath {
  hops: { edge: number; to: number }[];
  stop: TraceHop["stop"];
}

/** How many root-to-leaf paths a trace listing collects before it stops. */
export const TRACE_PATH_LIMIT = 200;

/** Flattens a trace into its root-to-leaf paths, at most `limit` of them. */
export function tracePaths(trace: TraceResult | null, limit = TRACE_PATH_LIMIT): TracePath[] {
  if (trace === null) {
    return [];
  }
  const out: TracePath[] = [];
  const walk = (hops: readonly TraceHop[], prefix: { edge: number; to: number }[]) => {
    for (const hop of hops) {
      if (out.length >= limit) {
        return;
      }
      const path = [...prefix, { edge: hop.edge, to: hop.to }];
      if (hop.next.length === 0) {
        out.push({ hops: path, stop: hop.stop });
      } else {
        walk(hop.next, path);
      }
    }
  };
  walk(trace.steps, []);
  return out;
}

/** The headline counts, each copied from the Rust report. */
export function overviewFigures(report: AnalysisReport): { label: string; value: number }[] {
  const t = report.totals;
  return [
    { label: "Files analysed", value: t.files },
    { label: "Files that failed to parse", value: t.files_failed },
    { label: "Nodes", value: t.nodes },
    { label: "Relationships", value: t.edges },
    { label: "Routes", value: t.routes },
    { label: "HTTP requests observed", value: t.http_observations },
    { label: "HTTP-call edges", value: t.http_call_edges },
    { label: "ORM models", value: t.orm_models },
    { label: "Tables", value: t.orm_tables },
    { label: "ORM edges (access + table queries)", value: t.orm_edges },
    { label: "Cross-language edges", value: t.cross_language_edges },
    { label: "Ambiguous requests (refused)", value: t.ambiguous },
    { label: "Unmatched requests", value: t.no_match },
    { label: "Unsupported requests (refused)", value: t.unsupported },
    { label: "Ambiguous ORM models (refused)", value: t.orm_ambiguous },
  ];
}

const TEST_PATH =
  /(^|\/)(tests?|__tests__|e2e|testing)\/|(^|\/)test_[^/]*\.py$|_tests?\.py$|(^|\/)conftest\.py$|\.(test|spec)\.[jt]sx?$/i;

/**
 * Whether a file looks like test code, by its path alone.
 *
 * A presentation filter, stated as such in the interface: Cartograph does not
 * separate test code from production code, and this is a path heuristic, not
 * an analysis result.
 */
export function isTestPath(file: string | undefined): boolean {
  return file !== undefined && TEST_PATH.test(file);
}

/** The nodes and edges one hop from a node, as Graphology keys. */
export function neighbourhoodKeys(
  node: number,
  outgoing: readonly { edge: number; node: number }[],
  incoming: readonly { edge: number; node: number }[],
): { nodes: Set<string>; edges: Set<string> } {
  const nodes = new Set<string>([String(node)]);
  const edges = new Set<string>();
  for (const n of [...outgoing, ...incoming]) {
    nodes.add(String(n.node));
    edges.add(`e${n.edge}`);
  }
  return { nodes, edges };
}

/**
 * Requests observed in frontend-language files, and how many were accepted.
 *
 * Counted from the report's own match decisions; the file-extension grouping
 * is the same presentation choice as {@link layerOf}.
 */
export function frontendRequests(decisions: readonly RequestDecision[]): {
  observed: number;
  connected: number;
} {
  let observed = 0;
  let connected = 0;
  for (const d of decisions) {
    if (FRONTEND_EXT.test(d.clientFile)) {
      observed += 1;
      if (d.status === "exact" || d.status === "strong") {
        connected += 1;
      }
    }
  }
  return { observed, connected };
}
