import { describe, expect, it } from "vitest";

import type { AnalysisReport, RequestDecision, TraceResult } from "./api";
import {
  groupDecisions,
  layerOf,
  layers,
  overviewFigures,
  tally,
  traceKeys,
  tracePaths,
} from "./model";
import type { Scene } from "../session";

const decision = (status: RequestDecision["status"]): RequestDecision => ({
  clientFile: "web/api.ts",
  clientLine: 1,
  clientRoute: "GET /api/x",
  status,
  candidates: [],
});

const report = (over: Partial<AnalysisReport> = {}): AnalysisReport => ({
  analysis: 1,
  totals: {
    files: 10,
    files_clean: 9,
    files_with_diagnostics: 1,
    files_failed: 1,
    symbols: 50,
    routes: 4,
    routes_composed: 0,
    http_observations: 6,
    exact: 0,
    strong: 3,
    ambiguous: 1,
    no_match: 1,
    unsupported: 1,
    dynamic_resolved: 0,
    dynamic_partial: 0,
    orm_models: 2,
    orm_tables: 2,
    orm_ambiguous: 0,
    nodes: 3,
    edges: 2,
    http_call_edges: 1,
    client_call_edges: 0,
    orm_edges: 1,
  },
  languages: { typescript: 1, tsx: 0, python: 2 },
  decisions: [],
  ormAmbiguous: [],
  diagnostics: [],
  nodes: [],
  elapsedMs: 5,
  ...over,
});

describe("layerOf", () => {
  it("places a node by its kind first, then its file's language", () => {
    expect(layerOf("table", undefined)).toBe("database");
    expect(layerOf("route", "api/urls.py")).toBe("api");
    expect(layerOf("class", "shop/models.py")).toBe("model");
    expect(layerOf("function", "web/src/api.tsx")).toBe("frontend");
    expect(layerOf("function", "shop/views.py")).toBe("backend");
    expect(layerOf("function", undefined)).toBe("other");
    expect(layerOf("external-service", undefined)).toBe("external");
  });

  it("reads files from the report, never from labels", () => {
    const scene: Scene = {
      nodes: [
        { id: 1, x: 0, y: 0, cluster: 0, label: "views.py", kind: "function" },
        { id: 2, x: 0, y: 0, cluster: 0, label: "load", kind: "function" },
      ],
      edges: [],
      clusters: [],
    };
    const mapped = layers(scene, report({ nodes: [{ id: 2, file: "web/api.ts", line: 1 }] }));
    // Node 1's label looks like a Python file, but the report gives it no
    // file, so it is not placed in the backend.
    expect(mapped.get(1)).toBe("other");
    expect(mapped.get(2)).toBe("frontend");
  });
});

describe("tally", () => {
  it("counts and orders by count, then name", () => {
    expect(tally(["b", "a", "b", "c", "a", "b"])).toEqual([
      ["b", 3],
      ["a", 2],
      ["c", 1],
    ]);
  });
});

describe("groupDecisions", () => {
  it("puts refusals first and drops empty groups", () => {
    const groups = groupDecisions([
      decision("strong"),
      decision("no-match"),
      decision("ambiguous"),
      decision("strong"),
    ]);
    expect(groups.map((g) => g.status)).toEqual(["ambiguous", "no-match", "strong"]);
    expect(groups[2]?.items).toHaveLength(2);
  });
});

describe("trace helpers", () => {
  const trace: TraceResult = {
    analysis: 1,
    from: 1,
    maxDepth: 10,
    steps: [
      {
        edge: 10,
        to: 2,
        next: [
          { edge: 11, to: 3, next: [], stop: "chain-end" },
          { edge: 12, to: 4, next: [], stop: "max-depth" },
        ],
      },
    ],
  };

  it("collects exactly the visited nodes and edges", () => {
    const keys = traceKeys(trace);
    expect([...keys.nodes].sort()).toEqual(["1", "2", "3", "4"]);
    expect([...keys.edges].sort()).toEqual(["e10", "e11", "e12"]);
    expect(traceKeys(null).nodes.size).toBe(0);
  });

  it("flattens to root-to-leaf paths with their stop reasons", () => {
    const paths = tracePaths(trace);
    expect(paths).toHaveLength(2);
    expect(paths[0]?.hops.map((h) => h.to)).toEqual([2, 3]);
    expect(paths[0]?.stop).toBe("chain-end");
    expect(paths[1]?.stop).toBe("max-depth");
    expect(tracePaths(trace, 1)).toHaveLength(1);
  });
});

describe("overviewFigures", () => {
  it("copies every figure from the report's totals", () => {
    const r = report();
    const figures = new Map(overviewFigures(r).map((f) => [f.label, f.value]));
    expect(figures.get("Files analysed")).toBe(r.totals.files);
    expect(figures.get("Relationships")).toBe(r.totals.edges);
    expect(figures.get("Routes")).toBe(r.totals.routes);
    expect(figures.get("Unmatched requests")).toBe(r.totals.no_match);
  });
});
