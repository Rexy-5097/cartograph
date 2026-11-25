import { describe, expect, it } from "vitest";

import type { AnalysisReport, RequestDecision } from "./api";
import type { Layer } from "./model";
import {
  findings,
  mostDependedOn,
  partOf,
  relationSentence,
  requestExample,
  systemFlows,
} from "./plain";
import type { Scene } from "../session";

const node = (id: number, label: string, kind: Scene["nodes"][number]["kind"]) => ({
  id,
  x: 0,
  y: 0,
  cluster: 0,
  label,
  kind,
});
const edge = (id: number, source: number, target: number, kind: Scene["edges"][number]["kind"]) => ({
  id,
  source,
  target,
  kind,
});

// A small system: a screen, a test, a view, a model and its table.
const scene: Scene = {
  nodes: [
    node(0, "loadOrders", "function"),
    node(1, "test_orders", "function"),
    node(2, "OrderView", "class"),
    node(3, "Order", "class"),
    node(4, "orders", "table"),
    node(5, "list_orders", "function"),
  ],
  edges: [
    edge(0, 1, 2, "http-call"),
    edge(1, 5, 3, "orm-access"),
    edge(2, 1, 3, "orm-access"),
    edge(3, 3, 4, "queries"),
    edge(4, 0, 0, "call"),
  ],
  clusters: [],
};
const layer = new Map<number, Layer>([
  [0, "frontend"],
  [1, "backend"],
  [2, "model"],
  [3, "model"],
  [4, "database"],
  [5, "backend"],
]);
const isTest = (id: number) => id === 1;

const decision = (clientFile: string, status: RequestDecision["status"]): RequestDecision => ({
  clientFile,
  clientLine: 1,
  clientRoute: "GET /x",
  status,
  candidates: [],
});

function report(decisions: RequestDecision[], totals: Partial<AnalysisReport["totals"]> = {}): AnalysisReport {
  return {
    analysis: 1,
    totals: {
      orm_models: 1,
      orm_tables: 1,
      ambiguous: 0,
      no_match: 0,
      unsupported: 0,
      ...totals,
    } as AnalysisReport["totals"],
    languages: {},
    decisions,
    ormAmbiguous: [],
    diagnostics: [],
    nodes: [],
    elapsedMs: 1,
  };
}

describe("partOf", () => {
  it("groups layers into the diagram's four parts", () => {
    expect(partOf("frontend")).toBe("screens");
    expect(partOf("backend")).toBe("server");
    expect(partOf("model")).toBe("server");
    expect(partOf("database")).toBe("database");
    expect(partOf("other")).toBeNull();
  });
});

describe("systemFlows", () => {
  it("counts parts, links inside a part, and links between parts, from the scene only", () => {
    const { parts, within, flows } = systemFlows(scene, layer);
    expect(parts).toEqual({ screens: 1, api: 0, server: 4, database: 1 });
    // http-call 1→2, orm 5→3, orm 1→3 stay in "server"; the self-call stays in "screens".
    expect(within).toEqual({ screens: 1, api: 0, server: 3, database: 0 });
    expect(flows).toEqual([{ from: "server", to: "database", count: 1 }]);
  });
});

describe("relationSentence", () => {
  it("words a relationship in plain language", () => {
    expect(relationSentence("orm-access", "a", "B")).toBe("a reads or writes data through B.");
    expect(relationSentence("unknown-kind", "a", "B")).toBe("a is connected to B.");
  });
});

describe("mostDependedOn", () => {
  it("ranks nodes by incoming relationships", () => {
    expect(mostDependedOn(scene, 2).map((x) => [x.label, x.count])).toEqual([
      ["Order", 2],
      ["loadOrders", 1],
    ]);
  });
});

describe("requestExample", () => {
  it("prefers a request sender outside tests, and falls back to a test", () => {
    expect(requestExample(scene, isTest)).toEqual({ id: 1, label: "test_orders" });
    expect(requestExample({ ...scene, edges: [] }, isTest)).toBeNull();
  });
});

describe("findings", () => {
  it("states only what the numbers support", () => {
    const out = findings(
      report([decision("web/a.ts", "no-match"), decision("web/b.ts", "unsupported")], { no_match: 1, unsupported: 1 }),
      scene,
      isTest,
    );
    const titles = out.map((f) => f.title);
    expect(titles).toContain("The data layer is mapped.");
    expect(titles).toContain("Most proven web requests come from automated tests.");
    expect(titles).toContain("The screens are not yet linked to the server.");
    expect(titles).toContain("Uncertain links are shown, not guessed.");
    const screens = out.find((f) => f.title.startsWith("The screens"));
    expect(screens?.body).toContain("2 web requests");
    expect(screens?.body).toContain("1 had no matching endpoint");
    expect(screens?.body).toContain("1 use an address that cannot be read");
  });

  it("says the screens are linked when a frontend request was accepted, and omits absent findings", () => {
    const out = findings(report([decision("web/a.ts", "strong")]), { ...scene, edges: [] }, isTest);
    expect(out.map((f) => f.title)).toEqual(["The screens are linked to the server."]);
  });
});
