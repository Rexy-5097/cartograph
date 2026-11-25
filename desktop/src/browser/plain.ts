/**
 * Plain-language wording for the explorer (M18 experiment).
 *
 * Everything here turns analysis results into sentences a reader who has
 * never used Cartograph can follow. It words what the Rust analysis found;
 * it never adds a relationship, a count or a reason the analysis did not
 * produce. The grouping into "parts of the system" is the same presentation
 * choice as `model.ts`'s layers (file type and node kind), and the interface
 * says so wherever it is shown.
 */

import type { AnalysisReport, RequestDecision } from "./api";
import type { Layer } from "./model";
import type { EdgeKind, NodeKind, Scene } from "../session";

/** The four parts of a system the report diagram shows, in reading order. */
export type Part = "screens" | "api" | "server" | "database";

export const PARTS: readonly Part[] = ["screens", "api", "server", "database"];

export const PART_LABEL: Record<Part, string> = {
  screens: "Screens and UI",
  api: "API endpoints",
  server: "Server code",
  database: "Database",
};

export const PART_HINT: Record<Part, string> = {
  screens: "TypeScript and JavaScript files",
  api: "Endpoints reached without a known handler",
  server: "Python functions and classes",
  database: "Tables the data models are stored in",
};

/** Which part of the diagram a layer belongs to; `null` for none. */
export function partOf(layer: Layer): Part | null {
  switch (layer) {
    case "frontend":
      return "screens";
    case "api":
      return "api";
    case "backend":
    case "model":
      return "server";
    case "database":
      return "database";
    default:
      return null;
  }
}

/** A layer's name for a reader who does not know the codebase. */
export const PLAIN_LAYER: Record<Layer, string> = {
  frontend: "Screens and UI code",
  api: "API endpoints",
  backend: "Server functions",
  model: "Server classes",
  database: "Database tables",
  external: "Outside services",
  other: "Other",
};

/** A relationship kind as a short phrase, and the technical name beside it. */
/**
 * A relationship kind in plain words: its name, the verb from the source's
 * side, and the phrase from the target's side ("used by …").
 */
export const PLAIN_EDGE: Record<EdgeKind, { name: string; verb: string; incoming: string }> = {
  "http-call": { name: "Web request", verb: "sends a web request to", incoming: "sends it a web request" },
  "orm-access": { name: "Uses data", verb: "reads or writes data through", incoming: "reads or writes its data" },
  queries: { name: "Stored in table", verb: "is stored in the table", incoming: "is stored here" },
  call: { name: "Calls", verb: "calls", incoming: "calls it" },
  import: { name: "Imports", verb: "imports", incoming: "imports it" },
  inherits: { name: "Builds on", verb: "builds on", incoming: "builds on it" },
  implements: { name: "Implements", verb: "implements", incoming: "implements it" },
  references: { name: "Refers to", verb: "refers to", incoming: "refers to it" },
};

export function plainEdge(kind: string): { name: string; verb: string; incoming: string } {
  return PLAIN_EDGE[kind as EdgeKind] ?? { name: kind, verb: "is connected to", incoming: "is connected to it" };
}

/** One sentence for one relationship, e.g. "a reads or writes data through B". */
export function relationSentence(kind: string, from: string, to: string): string {
  return `${from} ${plainEdge(kind).verb} ${to}.`;
}

/** A node kind as a reader would say it. */
export function plainKind(kind: NodeKind | string): string {
  switch (kind) {
    case "function":
    case "method":
      return "A function";
    case "class":
      return "A class";
    case "table":
      return "A database table";
    case "route":
      return "An API endpoint";
    case "file":
      return "A file";
    case "module":
      return "A module";
    default:
      return `A ${kind}`;
  }
}

export interface Flow {
  from: Part;
  to: Part;
  count: number;
}

/**
 * The diagram's numbers: how many map nodes fall in each part, how many
 * relationships stay inside a part, and how many cross from one part to
 * another. Counted from the scene; nothing is inferred.
 */
export function systemFlows(
  scene: Scene,
  layerOfNode: ReadonlyMap<number, Layer>,
): { parts: Record<Part, number>; within: Record<Part, number>; flows: Flow[] } {
  const parts: Record<Part, number> = { screens: 0, api: 0, server: 0, database: 0 };
  const within: Record<Part, number> = { screens: 0, api: 0, server: 0, database: 0 };
  const partById = new Map<number, Part>();
  for (const node of scene.nodes) {
    const part = partOf(layerOfNode.get(node.id) ?? "other");
    if (part !== null) {
      parts[part] += 1;
      partById.set(node.id, part);
    }
  }
  const crossing = new Map<string, number>();
  for (const edge of scene.edges) {
    const a = partById.get(edge.source);
    const b = partById.get(edge.target);
    if (a === undefined || b === undefined) {
      continue;
    }
    if (a === b) {
      within[a] += 1;
    } else {
      const key = `${a}>${b}`;
      crossing.set(key, (crossing.get(key) ?? 0) + 1);
    }
  }
  const flows: Flow[] = [];
  for (const [key, count] of crossing) {
    const [from, to] = key.split(">") as [Part, Part];
    flows.push({ from, to, count });
  }
  flows.sort((x, y) => PARTS.indexOf(x.from) - PARTS.indexOf(y.from) || PARTS.indexOf(x.to) - PARTS.indexOf(y.to));
  return { parts, within, flows };
}

/** The nodes most others depend on (most incoming relationships). */
export function mostDependedOn(scene: Scene, limit = 5): { id: number; label: string; kind: NodeKind; count: number }[] {
  const incoming = new Map<number, number>();
  for (const edge of scene.edges) {
    incoming.set(edge.target, (incoming.get(edge.target) ?? 0) + 1);
  }
  return scene.nodes
    .map((n) => ({ id: n.id, label: n.label, kind: n.kind, count: incoming.get(n.id) ?? 0 }))
    .filter((n) => n.count > 0)
    .sort((a, b) => b.count - a.count || a.label.localeCompare(b.label))
    .slice(0, limit);
}

/**
 * A node to start "follow a request" from: code outside tests first, then
 * one that also reads or writes data (so the walk reaches tables), then the
 * shortest name (easiest to read in a sentence), then the one sending the
 * most web requests. `null` when nothing sends a request.
 */
export function requestExample(
  scene: Scene,
  isTest: (id: number) => boolean,
): { id: number; label: string } | null {
  const sent = new Map<number, number>();
  const usesData = new Set<number>();
  for (const edge of scene.edges) {
    if (edge.kind === "http-call") {
      sent.set(edge.source, (sent.get(edge.source) ?? 0) + 1);
    } else if (edge.kind === "orm-access") {
      usesData.add(edge.source);
    }
  }
  const labelOf = new Map(scene.nodes.map((n) => [n.id, n.label]));
  const ranked = [...sent]
    .filter(([id]) => labelOf.has(id))
    .map(([id, count]) => ({ id, count, test: isTest(id), data: usesData.has(id), label: labelOf.get(id) ?? "" }))
    .sort(
      (a, b) =>
        Number(a.test) - Number(b.test) ||
        Number(b.data) - Number(a.data) ||
        a.label.length - b.label.length ||
        b.count - a.count ||
        a.id - b.id,
    );
  const best = ranked[0];
  return best === undefined ? null : { id: best.id, label: best.label };
}

export interface Finding {
  title: string;
  body: string;
}

const n = (value: number) => value.toLocaleString("en-GB");
const plural = (value: number, one: string, many: string) => (value === 1 ? one : many);

/**
 * The report's findings, each a sentence built from the analysis's own
 * numbers. A finding appears only when its numbers support it.
 */
export function findings(
  report: AnalysisReport,
  scene: Scene,
  isTest: (id: number) => boolean,
): Finding[] {
  const t = report.totals;
  const out: Finding[] = [];
  const ormAccess = scene.edges.filter((e) => e.kind === "orm-access").length;
  const requests = scene.edges.filter((e) => e.kind === "http-call");
  const fromTests = requests.filter((e) => isTest(e.source)).length;

  if (ormAccess > 0) {
    out.push({
      title: "The data layer is mapped.",
      body: `${n(ormAccess)} places in the code read or write data through ${n(t.orm_models)} data ${plural(t.orm_models, "model", "models")}, and ${n(t.orm_tables)} of those models are traced to the database ${plural(t.orm_tables, "table", "tables")} they are stored in.`,
    });
  }

  if (requests.length > 0) {
    out.push(
      fromTests * 2 > requests.length
        ? {
            title: "Most proven web requests come from automated tests.",
            body: `${n(fromTests)} of the ${n(requests.length)} web requests Cartograph could follow to their handler start in test code. They show which handler each request reaches.`,
          }
        : {
            title: "Web requests are followed to their handlers.",
            body: `${n(requests.length)} web ${plural(requests.length, "request is", "requests are")} traced from the code that sends ${plural(requests.length, "it", "them")} to the code that answers.`,
          },
    );
  }

  const front = report.decisions.filter((d) => /\.(tsx?|jsx?|mjs|cjs|vue)$/i.test(d.clientFile));
  const frontLinked = front.filter((d) => d.status === "exact" || d.status === "strong").length;
  if (front.length > 0 && frontLinked === 0) {
    const noMatch = front.filter((d) => d.status === "no-match").length;
    const unreadable = front.filter((d) => d.status === "unsupported").length;
    const parts: string[] = [];
    if (noMatch > 0) parts.push(`${n(noMatch)} had no matching endpoint in the code`);
    if (unreadable > 0) parts.push(`${n(unreadable)} use an address that cannot be read from the code alone`);
    out.push({
      title: "The screens are not yet linked to the server.",
      body: `Cartograph found ${n(front.length)} web ${plural(front.length, "request", "requests")} in frontend code but could not prove where ${plural(front.length, "it goes", "they go")}${parts.length > 0 ? `: ${parts.join(", and ")}` : ""}. The map shows the two sides apart rather than guess a link.`,
    });
  } else if (frontLinked > 0) {
    out.push({
      title: "The screens are linked to the server.",
      body: `${n(frontLinked)} web ${plural(frontLinked, "request", "requests")} from frontend code ${plural(frontLinked, "is", "are")} proven to reach a server handler.`,
    });
  }

  const refused = t.ambiguous + t.no_match + t.unsupported;
  if (refused > 0) {
    out.push({
      title: "Uncertain links are shown, not guessed.",
      body: `${n(refused)} ${plural(refused, "connection", "connections")} could not be proven from the source. Each is listed under “Gaps in the map” with the reason, instead of being drawn as a guess.`,
    });
  }
  return out;
}

/** The refusal groups of the "Gaps" view, in plain words. */
export const PLAIN_STATUS: Record<RequestDecision["status"], string> = {
  exact: "Proven",
  strong: "Proven",
  ambiguous: "Several possible endpoints",
  "no-match": "No matching endpoint found",
  unsupported: "Address can’t be read from the code",
  other: "Other",
};
