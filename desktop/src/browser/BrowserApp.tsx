/**
 * Cartograph Architecture Explorer — the browser client (M18 experiment;
 * proposed ADR-0024, not accepted).
 *
 * It renders what `cartograph-serve` returns, which is what
 * `cartograph-desktop` computed, which is what the Rust core decided. It
 * reuses the desktop's renderer (`GraphView`, `buildGraph`, Sigma settings),
 * its evidence panel, its blast highlight and its search. What it adds is
 * presentation: an overview, layer and kind filters, a trace view, an
 * unresolved-relationship view, diagnostics and a source preview.
 *
 * It never draws a relationship the scene does not contain, never computes a
 * count the report does not state, and never shows evidence for an analysis
 * other than the one on screen: every lookup carries the analysis id, and a
 * stale answer is discarded.
 */

import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import type Sigma from "sigma";

import EvidencePanel from "../EvidencePanel";
import GraphView, { type GraphViewOptions } from "../GraphView";
import { describeBlast, impactedNodeKeys } from "../blast";
import { edgeColor } from "../graph";
import { findNodes } from "../search";
import type {
  AnalysisPayload,
  BlastResult,
  EdgeKind,
  EvidenceRecord,
  SceneNode,
} from "../session";
import {
  Api,
  ApiError,
  type AnalysisReport,
  type NodeDetail,
  type ServerStatus,
  type SourceExcerpt,
  type TraceResult,
  sessionToken,
} from "./api";
import {
  EDGE_LABEL,
  LAYER_COLOR,
  type Layer,
  STATUS_MEANING,
  TRACE_PATH_LIMIT,
  frontendRequests,
  STOP_MEANING,
  edgeKinds,
  groupDecisions,
  isTestPath,
  neighbourhoodKeys,
  layers as layerMap,
  locations,
  nodeKinds,
  overviewFigures,
  tally,
  traceKeys,
  tracePaths,
} from "./model";
import SourcePreview from "./SourcePreview";
import Report from "./Report";
import { PLAIN_LAYER, PLAIN_STATUS, plainEdge, plainKind, relationSentence } from "./plain";

type View =
  | "overview"
  | "map"
  | "frontend"
  | "api"
  | "backend"
  | "database"
  | "dependencies"
  | "trace"
  | "blast"
  | "unresolved"
  | "diagnostics";

type Selection =
  | { kind: "none" }
  | { kind: "node"; id: number }
  | { kind: "edge"; id: number };

const LAYER_VIEWS: Partial<Record<View, Layer[]>> = {
  frontend: ["frontend"],
  api: ["api"],
  backend: ["backend", "model"],
  database: ["database"],
};

const ALL_EDGE_KINDS: EdgeKind[] = [
  "http-call",
  "orm-access",
  "queries",
  "call",
  "import",
  "inherits",
  "implements",
  "references",
];

function errorText(error: unknown): { message: string; hint?: string } {
  if (error instanceof ApiError) {
    return { message: error.message, hint: error.hint };
  }
  return { message: "Something went wrong talking to the Cartograph server." };
}

export default function BrowserApp() {
  const token = useMemo(() => sessionToken(), []);
  const api = useMemo(() => (token === null ? null : new Api(token)), [token]);

  const [status, setStatus] = useState<ServerStatus | null>(null);
  const [reachable, setReachable] = useState(true);
  const [payload, setPayload] = useState<AnalysisPayload | null>(null);
  const [report, setReport] = useState<AnalysisReport | null>(null);
  const [loadError, setLoadError] = useState<{ message: string; hint?: string } | null>(null);

  const [view, setView] = useState<View>("overview");
  const [selection, setSelection] = useState<Selection>({ kind: "none" });
  const [evidence, setEvidence] = useState<EvidenceRecord | null>(null);
  const [detail, setDetail] = useState<NodeDetail | null>(null);
  const [inspectorError, setInspectorError] = useState<string | null>(null);
  const [source, setSource] = useState<SourceExcerpt | null>(null);

  const [trace, setTrace] = useState<TraceResult | null>(null);
  const [blast, setBlast] = useState<BlastResult | null>(null);

  const [query, setQuery] = useState("");
  const [hiddenEdgeKinds, setHiddenEdgeKinds] = useState<Set<string>>(new Set());
  const [hiddenLayers, setHiddenLayers] = useState<Set<Layer>>(new Set());
  const [colourBy, setColourBy] = useState<"layer" | "cluster">("layer");
  const [settingsOpen, setSettingsOpen] = useState(false);
  // A first-time reader sees the product's own code; tests are one click away
  // and the legend says how many are hidden.
  const [hideTests, setHideTests] = useState(true);
  const [focus, setFocus] = useState<{ nodes: Set<string>; edges: Set<string> } | null>(null);

  const sigma = useRef<Sigma | null>(null);
  const sourceSeq = useRef(0);
  // The analysis on screen, updated the moment a new one is installed, and a
  // counter per kind of request. An answer is applied only if it is still the
  // newest of its kind and belongs to the analysis on screen; anything else
  // arrived late and is dropped.
  const liveAnalysis = useRef<number | null>(null);
  const inspectorSeq = useRef(0);
  const overlaySeq = useRef(0);
  const inspector = useRef<HTMLElement | null>(null);
  const onSigma = useCallback((instance: Sigma | null) => {
    sigma.current = instance;
  }, []);

  const analysisId = payload?.analysis ?? null;

  // ── Loading and refreshing ────────────────────────────────────────────
  /** Installs the newest analysis; true if one was installed. */
  const load = useCallback(async (): Promise<boolean> => {
    if (api === null) {
      return false;
    }
    try {
      const [nextPayload, nextReport] = await Promise.all([api.analysis(), api.report()]);
      if (nextPayload.analysis !== nextReport.analysis) {
        // Two different analyses: the next poll will bring a matching pair.
        return false;
      }
      liveAnalysis.current = nextPayload.analysis;
      inspectorSeq.current += 1;
      overlaySeq.current += 1;
      setPayload(nextPayload);
      setReport(nextReport);
      setSelection({ kind: "none" });
      setEvidence(null);
      setDetail(null);
      setSource(null);
      setTrace(null);
      setBlast(null);
      setFocus(null);
      setLoadError(null);
      return true;
    } catch (error) {
      setLoadError(errorText(error));
      return false;
    }
  }, [api]);

  useEffect(() => {
    if (api === null) {
      return undefined;
    }
    let cancelled = false;
    let timer: number | undefined;
    const poll = async () => {
      try {
        const next = await api.status();
        if (cancelled) {
          return;
        }
        setStatus(next);
        setReachable(true);
        if (next.analysis !== null && next.analysis !== analysisId) {
          await load();
        }
        // Fast while an analysis runs; slow otherwise, so a refresh started
        // elsewhere (another tab, the API) and a stopped server are noticed.
        const waiting =
          next.phase === "running" || (next.analysis === null && next.phase !== "failed");
        timer = window.setTimeout(poll, waiting ? 600 : 5000);
      } catch (error) {
        if (!cancelled) {
          setReachable(false);
          setLoadError(errorText(error));
          timer = window.setTimeout(poll, 2000);
        }
      }
    };
    void poll();
    return () => {
      cancelled = true;
      window.clearTimeout(timer);
    };
  }, [api, analysisId, load]);

  const refresh = async () => {
    if (api === null) {
      return;
    }
    try {
      await api.analyze();
      setStatus((s) => (s === null ? s : { ...s, phase: "running" }));
      // The status effect is keyed on the analysis id, so re-arm it by polling
      // here until the new analysis is published.
      const wait = async () => {
        const next = await api.status();
        setStatus(next);
        if (next.phase === "running") {
          window.setTimeout(() => void wait(), 600);
        } else if (next.analysis !== null && next.analysis !== analysisId) {
          await load();
        }
      };
      void wait();
    } catch (error) {
      setLoadError(errorText(error));
    }
  };

  /**
   * Shows a lookup failure. A stale refusal means the server holds a newer
   * analysis than the page (refreshed from another tab or client), so the
   * page loads it rather than asking the user to select again on an old map.
   */
  const lookupFailed = useCallback(
    (error: unknown) => {
      setInspectorError(errorText(error).message);
      if (api === null || !(error instanceof ApiError) || error.kind !== "staleSelection") {
        return;
      }
      void (async () => {
        try {
          const next = await api.status();
          setStatus(next);
          // The poll may already have installed it; compare with what is on
          // screen now, not with the id this callback was created under.
          if (next.analysis !== null && next.analysis !== liveAnalysis.current) {
            if (await load()) {
              setInspectorError(
                `The analysis was refreshed elsewhere, so the map now shows analysis #${next.analysis}. Select again.`,
              );
            }
          } else if (next.analysis !== null) {
            setInspectorError(
              `The analysis was refreshed elsewhere, so the map now shows analysis #${next.analysis}. Select again.`,
            );
          }
        } catch (statusError) {
          setLoadError(errorText(statusError));
        }
      })();
    },
    [api, load],
  );

  // ── Derived, presentational data ──────────────────────────────────────
  const scene = payload?.scene ?? null;
  const layerOfNode = useMemo(
    () => (scene === null ? new Map<number, Layer>() : layerMap(scene, report)),
    [scene, report],
  );
  const where = useMemo(() => locations(report), [report]);
  const nodeById = useMemo(() => {
    const out = new Map<number, SceneNode>();
    for (const node of scene?.nodes ?? []) {
      out.set(node.id, node);
    }
    return out;
  }, [scene]);
  const layerCounts = useMemo(() => tally([...layerOfNode.values()]), [layerOfNode]);
  const edgeKindById = useMemo(() => {
    const out = new Map<number, EdgeKind>();
    for (const edge of scene?.edges ?? []) {
      out.set(edge.id, edge.kind);
    }
    return out;
  }, [scene]);
  const presentEdgeKinds = useMemo(
    () => (scene === null ? [] : edgeKinds(scene)),
    [scene],
  );

  const traceHighlight = useMemo(() => (trace === null ? null : traceKeys(trace)), [trace]);

  const viewHighlight = useMemo(() => {
    const wanted = LAYER_VIEWS[view];
    if (wanted === undefined || scene === null) {
      return null;
    }
    const nodes = new Set<string>();
    for (const node of scene.nodes) {
      const layer = layerOfNode.get(node.id);
      if (layer !== undefined && wanted.includes(layer)) {
        nodes.add(String(node.id));
      }
    }
    const edges = new Set<string>();
    for (const edge of scene.edges) {
      if (nodes.has(String(edge.source)) || nodes.has(String(edge.target))) {
        edges.add(`e${edge.id}`);
        nodes.add(String(edge.source));
        nodes.add(String(edge.target));
      }
    }
    return { nodes, edges };
  }, [view, scene, layerOfNode]);

  const hiddenKinds = useMemo(() => {
    if (view === "dependencies") {
      return new Set(ALL_EDGE_KINDS.filter((k) => k !== "import" && k !== "call"));
    }
    return hiddenEdgeKinds;
  }, [view, hiddenEdgeKinds]);

  const graphView = useMemo<GraphViewOptions>(() => {
    const hiddenNodeKinds = new Set<string>();
    const hiddenClusters = new Set<number>();
    const highlight =
      view === "trace"
        ? traceHighlight
        : view === "blast"
          ? null
          : (focus ?? viewHighlight);
    return {
      hiddenNodeKinds,
      // A trace or blast is shown whole: edge-kind filters do not cut it.
      hiddenEdgeKinds: view === "trace" || view === "blast" ? new Set<string>() : hiddenKinds,
      hiddenClusters,
      highlight,
      selectedNode: selection.kind === "node" ? String(selection.id) : null,
      selectedEdge: selection.kind === "edge" ? `e${selection.id}` : null,
      colorForKind: null,
    };
  }, [view, traceHighlight, viewHighlight, hiddenKinds, selection, focus]);

  const layerView = useMemo<GraphViewOptions>(() => {
    const out: GraphViewOptions = { ...graphView };
    if (hiddenLayers.size > 0 || colourBy === "layer" || hideTests) {
      const layerByKey = new Map<string, Layer>();
      for (const [id, layer] of layerOfNode) {
        layerByKey.set(String(id), layer);
      }
      const testKeys = new Set<string>();
      if (hideTests) {
        for (const [id, at] of where) {
          if (isTestPath(at.file)) {
            testKeys.add(String(id));
          }
        }
      }
      // Nodes in the trace or blast on screen stay visible whatever the
      // filters say: hiding part of a result would misstate it.
      const kept = new Set<string>();
      if (selection.kind === "node") {
        kept.add(String(selection.id));
      }
      if (view === "blast" && blast !== null) {
        kept.add(String(blast.target));
        for (const key of impactedNodeKeys(blast)) {
          kept.add(key);
        }
      } else if (view === "trace" && traceHighlight !== null) {
        for (const key of traceHighlight.nodes) {
          kept.add(key);
        }
      }
      out.nodeStyle = (key: string) => {
        const layer = layerByKey.get(key);
        if (layer === undefined) {
          return {};
        }
        return {
          hidden: !kept.has(key) && (hiddenLayers.has(layer) || testKeys.has(key)),
          // In the blast view the renderer's target / impacted / dimmed
          // colours carry the result, so the layer colour does not replace them.
          color: colourBy === "layer" && view !== "blast" ? LAYER_COLOR[layer] : undefined,
        };
      };
    }
    return out;
  }, [graphView, hiddenLayers, colourBy, layerOfNode, hideTests, where, view, blast, traceHighlight, selection]);

  const testNodeCount = useMemo(() => {
    let n = 0;
    for (const at of where.values()) {
      if (isTestPath(at.file)) {
        n += 1;
      }
    }
    return n;
  }, [where]);

  // ── Selection and lookups ─────────────────────────────────────────────
  const selectEdge = useCallback(
    async (edge: number) => {
      setSelection({ kind: "edge", id: edge });
      setDetail(null);
      setSource(null);
      setInspectorError(null);
      if (api === null || analysisId === null) {
        return;
      }
      const seq = ++inspectorSeq.current;
      try {
        const record = await api.evidence(analysisId, edge);
        if (seq === inspectorSeq.current && record.analysis === liveAnalysis.current) {
          setEvidence(record);
        }
      } catch (error) {
        if (seq === inspectorSeq.current) {
          setEvidence(null);
          lookupFailed(error);
        }
      }
    },
    [api, analysisId, lookupFailed],
  );

  const selectNode = useCallback(
    async (node: number) => {
      setSelection({ kind: "node", id: node });
      setEvidence(null);
      setSource(null);
      setInspectorError(null);
      if (api === null || analysisId === null) {
        return;
      }
      const seq = ++inspectorSeq.current;
      try {
        const found = await api.node(analysisId, node);
        if (seq === inspectorSeq.current && found.analysis === liveAnalysis.current) {
          setDetail(found);
        }
      } catch (error) {
        if (seq === inspectorSeq.current) {
          setDetail(null);
          lookupFailed(error);
        }
      }
    },
    [api, analysisId, lookupFailed],
  );

  const clearSelection = useCallback(() => {
    inspectorSeq.current += 1;
    setSelection({ kind: "none" });
    setEvidence(null);
    setDetail(null);
    setSource(null);
    setInspectorError(null);
  }, []);

  const focusNode = (node: number) => {
    const instance = sigma.current;
    const data = instance?.getNodeDisplayData(String(node));
    if (instance !== null && instance !== undefined && data !== undefined) {
      void instance.getCamera().animate({ x: data.x, y: data.y, ratio: 0.15 }, { duration: 400 });
    }
  };

  /** Frames the camera on a set of nodes, from their drawn positions. */
  const fitTo = useCallback((keys: ReadonlySet<string>) => {
    const instance = sigma.current;
    if (instance === null || keys.size === 0) {
      return;
    }
    let minX = Infinity;
    let minY = Infinity;
    let maxX = -Infinity;
    let maxY = -Infinity;
    for (const key of keys) {
      const data = instance.getNodeDisplayData(key);
      if (data === undefined) {
        continue;
      }
      minX = Math.min(minX, data.x);
      maxX = Math.max(maxX, data.x);
      minY = Math.min(minY, data.y);
      maxY = Math.max(maxY, data.y);
    }
    if (!Number.isFinite(minX)) {
      return;
    }
    const span = Math.max(maxX - minX, maxY - minY);
    void instance.getCamera().animate(
      { x: (minX + maxX) / 2, y: (minY + maxY) / 2, ratio: Math.max(0.06, span * 1.6) },
      { duration: 400 },
    );
  }, []);

  useEffect(() => {
    if (view === "trace" && traceHighlight !== null) {
      window.setTimeout(() => fitTo(traceHighlight.nodes), 80);
    }
  }, [view, traceHighlight, fitTo]);

  useEffect(() => {
    if (view === "blast" && blast !== null) {
      const keys = impactedNodeKeys(blast);
      keys.add(String(blast.target));
      window.setTimeout(() => fitTo(keys), 80);
    }
  }, [view, blast, fitTo]);

  useEffect(() => {
    if (focus !== null) {
      window.setTimeout(() => fitTo(focus.nodes), 80);
    }
  }, [focus, fitTo]);

  const showSource = async (file: string, line?: number) => {
    if (api === null || analysisId === null) {
      return;
    }
    // Belongs to the current selection (a newer selection makes it late), and
    // a newer source request supersedes it.
    const seq = inspectorSeq.current;
    const mine = ++sourceSeq.current;
    const current = () => seq === inspectorSeq.current && mine === sourceSeq.current;
    try {
      const found = await api.source(analysisId, file, line);
      if (current() && found.analysis === liveAnalysis.current) {
        setSource(found);
      }
    } catch (error) {
      if (current()) {
        lookupFailed(error);
      }
    }
  };

  const runTrace = async (node: number) => {
    if (api === null || analysisId === null) {
      return;
    }
    const seq = ++overlaySeq.current;
    try {
      const found = await api.trace(analysisId, node);
      if (seq === overlaySeq.current && found.analysis === liveAnalysis.current) {
        setTrace(found);
        setBlast(null);
        setView("trace");
      }
    } catch (error) {
      if (seq === overlaySeq.current) {
        lookupFailed(error);
      }
    }
  };

  const runBlast = async (node: number) => {
    if (api === null || analysisId === null) {
      return;
    }
    const seq = ++overlaySeq.current;
    try {
      const found = await api.blast(analysisId, node);
      if (seq === overlaySeq.current && found.analysis === liveAnalysis.current) {
        setBlast(found);
        setTrace(null);
        setView("blast");
      }
    } catch (error) {
      if (seq === overlaySeq.current) {
        lookupFailed(error);
      }
    }
  };

  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        clearSelection();
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [clearSelection]);

  const matches = useMemo(
    () => (scene === null ? [] : findNodes(scene.nodes, query).slice(0, 12)),
    [scene, query],
  );

  // ── Rendering ─────────────────────────────────────────────────────────
  if (token === null) {
    return (
      <main className="bx-empty">
        <h1>Cartograph Architecture Explorer</h1>
        <p>
          This page needs the session link printed by <code>cartograph-serve</code>. Open the
          URL it printed, including the part after <code>#</code>.
        </p>
      </main>
    );
  }

  const label = (id: number) => nodeById.get(id)?.label ?? `node ${id}`;
  const decisions = report?.decisions ?? [];
  const refusals = decisions.filter((d) => d.status !== "strong" && d.status !== "exact");
  const routeNodes = scene?.nodes.filter((n) => n.kind === "route") ?? [];
  const frontend = frontendRequests(decisions);

  const layerCount = (layer: Layer) => layerCounts.find(([l]) => l === layer)?.[1] ?? 0;
  const navItems: { id: View; label: string; group: string; count?: number; empty?: boolean }[] = [
    { id: "overview", label: "Report", group: "Start here" },
    { id: "map", label: "Whole system", group: "Explore", count: scene?.nodes.length },
    { id: "frontend", label: "Screens and UI", group: "Explore", count: layerCount("frontend") },
    { id: "api", label: "API endpoints", group: "Explore", count: layerCount("api") },
    { id: "backend", label: "Server code", group: "Explore", count: layerCount("backend") + layerCount("model") },
    { id: "database", label: "Database", group: "Explore", count: layerCount("database") },
    {
      id: "dependencies",
      label: "Direct calls",
      group: "Explore",
      count: scene?.edges.filter((e) => e.kind === "import" || e.kind === "call").length ?? 0,
    },
    { id: "trace", label: "Follow a request", group: "Ask" },
    { id: "blast", label: "Impact of a change", group: "Ask" },
    { id: "unresolved", label: "Gaps in the map", group: "Check", count: refusals.length + routeNodes.length },
    { id: "diagnostics", label: "Unreadable code", group: "Check", count: report?.diagnostics.length ?? 0 },
  ];
  for (const item of navItems) {
    item.empty =
      item.count === 0 &&
      item.id !== "overview" &&
      item.id !== "trace" &&
      item.id !== "blast";
  }

  const phase = status?.phase ?? "idle";
  const showCanvas = view !== "overview" && view !== "unresolved" && view !== "diagnostics";

  return (
    <div className={view === "overview" && selection.kind === "none" && source === null ? "bx-app bx-app-report" : "bx-app"}>
      <header className="bx-top">
        <div className="bx-brand">
          <CartographMark />
          <span className="bx-name">Cartograph</span>
          <span className="bx-sub">Architecture report</span>
          <span className="bx-experimental" title="Experimental build: proposed ADR-0024, not accepted">
            Preview
          </span>
        </div>
        <div className="bx-repo" title="Repository being explored">
          {status?.repository ?? payload?.repository ?? "…"}
        </div>
        <div className={`bx-status bx-status-${phase}`} role="status" aria-live="polite">
          {!reachable
            ? "Disconnected"
            : phase === "running"
            ? "Scanning…"
            : phase === "failed"
              ? "Last scan failed"
              : analysisId === null
                ? "Waiting for the first scan"
                : <span title={`Analysis #${analysisId}`}>Up to date</span>}
        </div>
        <button
          type="button"
          className="bx-button"
          onClick={() => void refresh()}
          disabled={phase === "running"}
        >
          Re-scan
        </button>
        <div className="bx-search">
          <label htmlFor="bx-search-input" className="bx-visually-hidden">
            Find a function, class or table
          </label>
          <input
            id="bx-search-input"
            type="search"
            placeholder="Find a function, class or table…"
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            autoComplete="off"
          />
          {matches.length > 0 && (
            <ul className="bx-search-results" role="listbox" aria-label="Matching symbols">
              {matches.map((m) => (
                <li key={m.id}>
                  <button
                    type="button"
                    onClick={() => {
                      setQuery("");
                      if (view === "overview" || view === "unresolved" || view === "diagnostics") {
                        setView("map");
                      }
                      void selectNode(m.id);
                      window.setTimeout(() => focusNode(m.id), 50);
                      // The result list closes; keep keyboard focus on what was chosen.
                      inspector.current?.focus();
                    }}
                  >
                    <span>{m.label}</span>
                    <span className="bx-kind">{plainKind(m.kind).replace(/^An? /, "")}</span>
                    <span className="bx-muted">{where.get(m.id)?.file ?? ""}</span>
                  </button>
                </li>
              ))}
            </ul>
          )}
        </div>
        <div className="bx-controls" role="group" aria-label="Graph controls">
          <button type="button" className="bx-icon-button" aria-label="Zoom in"
            onClick={() => void sigma.current?.getCamera().animatedZoom({ duration: 200 })}>+</button>
          <button type="button" className="bx-icon-button" aria-label="Zoom out"
            onClick={() => void sigma.current?.getCamera().animatedUnzoom({ duration: 200 })}>−</button>
          <button type="button" className="bx-icon-button" aria-label="Reset view"
            onClick={() => void sigma.current?.getCamera().animatedReset({ duration: 200 })}>⟲</button>
          <button type="button" className="bx-icon-button" aria-label="Settings"
            aria-expanded={settingsOpen} onClick={() => setSettingsOpen((o) => !o)}>⚙</button>
        </div>
        {settingsOpen && (
          <div className="bx-settings" role="dialog" aria-label="Display settings">
            <fieldset>
              <legend>Colour the map by</legend>
              <label>
                <input type="radio" name="colour" checked={colourBy === "layer"}
                  onChange={() => setColourBy("layer")} /> Part of the system
              </label>
              <label>
                <input type="radio" name="colour" checked={colourBy === "cluster"}
                  onChange={() => setColourBy("cluster")} /> Folder
              </label>
            </fieldset>
          </div>
        )}
      </header>

      <nav className="bx-nav" aria-label="Views">
        <ul>
          {navItems.map((item, i) => (
            <li key={item.id}>
              {(i === 0 || navItems[i - 1]?.group !== item.group) && (
                <span className="bx-nav-group">{item.group}</span>
              )}
              <button
                type="button"
                className={view === item.id ? "bx-nav-item bx-nav-current" : "bx-nav-item"}
                aria-current={view === item.id ? "page" : undefined}
                onClick={() => {
                  overlaySeq.current += 1;
                  setView(item.id);
                }}
              >
                <span>{item.label}</span>
                {item.count !== undefined && (
                  <span className={item.empty === true ? "bx-count bx-count-empty" : "bx-count"}>
                    {item.empty === true ? "none" : item.count}
                  </span>
                )}
              </button>
            </li>
          ))}
        </ul>
        <p className="bx-nav-note">
          Numbers count what each view shows. Code is grouped by file type. “None” means nothing
          was found, not that the code has none.
        </p>
      </nav>

      <main className="bx-main">
        {loadError !== null && (
          <div className="bx-banner bx-banner-error" role="alert">
            <strong>{loadError.message}</strong>
            {loadError.hint !== undefined && <span> {loadError.hint}</span>}
          </div>
        )}
        {status?.error != null && phase === "failed" && (
          <div className="bx-banner bx-banner-error" role="alert">
            <strong>The last analysis failed: {status.error.message}</strong>
            {payload !== null && <span> The previous analysis is still shown.</span>}
          </div>
        )}
        {payload === null && loadError === null && phase !== "failed" && (
          <div className="bx-loading" role="status">
            <div className="bx-spinner" aria-hidden="true" />
            <p>Cartograph is analysing the repository. The map appears when it finishes.</p>
          </div>
        )}

        {payload !== null && report !== null && view === "overview" && (
          <Report
            repository={payload.repository}
            report={report}
            scene={payload.scene}
            layerOfNode={layerOfNode}
            isTest={(id) => isTestPath(where.get(id)?.file)}
            onOpenMap={() => setView("map")}
            onOpenGaps={() => setView("unresolved")}
            onImpact={(id) => {
              void selectNode(id);
              void runBlast(id);
            }}
            onFollow={(id) => {
              void selectNode(id);
              void runTrace(id);
            }}
            technical={
          <section className="bx-overview" aria-label="Technical details">
            <p className="bx-lede">
              Analysis #{payload.analysis}, {report.elapsedMs} ms. Every figure below is a count
              from Cartograph’s Rust analysis of this repository; nothing is estimated.
            </p>
            {frontend.observed > 0 && frontend.connected === 0 && (
              <div className="bx-callout" role="note">
                <strong>No frontend request is connected to a backend route in this analysis.</strong>{" "}
                Cartograph observed {report.totals.http_observations.toLocaleString()} HTTP call sites;{" "}
                {frontend.observed.toLocaleString()} of them are in frontend (TypeScript or JavaScript)
                files, and none of those was matched, so the map shows the frontend and the backend as
                separate regions. This is a statement about what Cartograph could establish, not about
                how the application works. See <em>Unresolved</em> for every request it declined to
                match, and why.
              </div>
            )}
            <div className="bx-figures">
              {overviewFigures(report).map((f) => (
                <div className="bx-figure" key={f.label}>
                  <div className="bx-figure-value">{f.value.toLocaleString()}</div>
                  <div className="bx-figure-label">{f.label}</div>
                </div>
              ))}
            </div>
            <div className="bx-columns">
              <div>
                <h2>Languages (files)</h2>
                <ul className="bx-bars">
                  {Object.entries(report.languages).map(([lang, n]) => (
                    <li key={lang}><span>{lang}</span><span>{n}</span></li>
                  ))}
                </ul>
              </div>
              <div>
                <h2>Node kinds</h2>
                <ul className="bx-bars">
                  {scene !== null && nodeKinds(scene).map(([k, n]) => (
                    <li key={k}><span>{k}</span><span>{n}</span></li>
                  ))}
                </ul>
              </div>
              <div>
                <h2>Relationship kinds</h2>
                <ul className="bx-bars">
                  {presentEdgeKinds.map(([k, n]) => (
                    <li key={k}>
                      <span><EdgeSwatch kind={k} />{EDGE_LABEL[k] ?? k}</span>
                      <span>{n}</span>
                    </li>
                  ))}
                </ul>
              </div>
            </div>
            <h2>How to read this</h2>
            <ul className="bx-prose">
              <li>Every line on the map is a relationship Cartograph established from source, with evidence. Click it to see that evidence.</li>
              <li>A missing line is not proof of no connection. Requests Cartograph could not resolve are listed under <em>Unresolved</em>, with the reason.</li>
              <li>Confidence values are uncalibrated priors chosen by evidence class — not probabilities.</li>
              <li>Node colours show the file language (TypeScript → frontend, Python → backend) or kind (route, table); they are a reading aid, not an analysis result.</li>
            </ul>
          </section>
            }
          />
        )}

        {payload !== null && scene !== null && showCanvas && (
          <section className="bx-canvas-area" aria-label="Architecture map">
            <GraphView
              scene={scene}
              onSelectEdge={(e) => void selectEdge(e)}
              onSelectNode={(n) => void selectNode(n)}
              onClearSelection={clearSelection}
              blast={view === "blast" ? blast : null}
              view={layerView}
              onSigma={onSigma}
            />
            <aside className="bx-legend" aria-label="Legend and filters">
              <label className="bx-legend-toggle">
                <input type="checkbox" checked={hideTests} onChange={() => setHideTests((h) => !h)} />
                Hide automated tests <span className="bx-muted">{testNodeCount} pieces, by file name</span>
              </label>
              {focus !== null && (
                <button type="button" className="bx-button bx-small" onClick={() => setFocus(null)}>
                  Show everything again
                </button>
              )}
              <h2>Parts of the system</h2>
              <ul>
                {layerCounts.map(([layer, n]) => (
                  <li key={layer}>
                    <label>
                      <input
                        type="checkbox"
                        checked={!hiddenLayers.has(layer)}
                        onChange={() =>
                          setHiddenLayers((s) => {
                            const next = new Set(s);
                            if (next.has(layer)) next.delete(layer);
                            else next.add(layer);
                            return next;
                          })
                        }
                      />
                      <i className="bx-dot" style={{ background: LAYER_COLOR[layer] }} />
                      {PLAIN_LAYER[layer]} <span className="bx-muted">{n}</span>
                    </label>
                  </li>
                ))}
              </ul>
              <h2>Connections</h2>
              <ul>
                {presentEdgeKinds.map(([kind, n]) => (
                  <li key={kind}>
                    <label>
                      <input
                        type="checkbox"
                        checked={!hiddenKinds.has(kind)}
                        disabled={view === "dependencies"}
                        onChange={() =>
                          setHiddenEdgeKinds((s) => {
                            const next = new Set(s);
                            if (next.has(kind)) next.delete(kind);
                            else next.add(kind);
                            return next;
                          })
                        }
                      />
                      <EdgeSwatch kind={kind} />
                      {plainEdge(kind).name} <span className="bx-muted">{n}</span>
                    </label>
                  </li>
                ))}
              </ul>
            </aside>
            {view === "trace" && (
              <div className="bx-overlay" aria-label="Trace">
                {trace === null ? (
                  <p>Pick any piece of code on the map, then choose <strong>Follow from here</strong>. Cartograph shows where it leads, step by step.</p>
                ) : (
                  <TracePanel
                    trace={trace}
                    label={label}
                    kindOf={(id) => nodeById.get(id)?.kind ?? ""}
                    edgeKindOf={(id) => edgeKindById.get(id) ?? ""}
                    onEdge={(e) => void selectEdge(e)}
                  />
                )}
              </div>
            )}
            {view === "blast" && (
              <div className="bx-overlay" aria-label="Blast radius">
                {blast === null ? (
                  <p>Pick any piece of code on the map, then choose <strong>What depends on this?</strong> to see everything a change to it could affect.</p>
                ) : (
                  <BlastPanel blast={blast} label={label} onEdge={(e) => void selectEdge(e)} onNode={(n) => void selectNode(n)} />
                )}
              </div>
            )}
          </section>
        )}

        {payload !== null && report !== null && view === "unresolved" && (
          <section className="bx-list-view" aria-labelledby="bx-unresolved-title">
            <h1 id="bx-unresolved-title">Gaps in the map</h1>
            <p className="bx-lede">
              Places where the code makes a connection Cartograph could not prove — for example, a
              web request whose address is only known when the app runs. They are listed here instead
              of being drawn as guesses. A gap is not proof that nothing is connected.
            </p>
            {groupDecisions(refusals).map((group) => (
              <details key={group.status} open={group.items.length <= 25}>
                <summary>
                  {PLAIN_STATUS[group.status]}{" "}
                  <span className="bx-muted">
                    {group.items.length} request{group.items.length === 1 ? "" : "s"}
                  </span>{" "}
                  <span className={`bx-status-pill bx-pill-${group.status}`}>{group.status}</span>
                </summary>
                <p className="bx-muted">{STATUS_MEANING[group.status]}</p>
                <ul className="bx-decisions">
                  {group.items.slice(0, 300).map((d, i) => (
                    <li key={`${d.clientFile}:${d.clientLine}:${i}`}>
                      <code>{d.clientRoute}</code>
                      <button type="button" className="bx-link" onClick={() => void showSource(d.clientFile, d.clientLine)}>
                        {d.clientFile}:{d.clientLine}
                      </button>
                      {d.unsupportedReason !== undefined && (
                        <span className="bx-muted"> reason: {d.unsupportedReason}</span>
                      )}
                      {d.candidates.length > 0 && (
                        <details className="bx-candidates-toggle">
                          <summary>
                            {d.candidates.length} possible endpoint{d.candidates.length === 1 ? "" : "s"}
                          </summary>
                          <ul className="bx-candidates">
                            {d.candidates.slice(0, 5).map((c, j) => (
                              <li key={j}>
                                <code>{c.route}</code> at {c.file}:{c.line}
                                {c.handler !== undefined && <> → <code>{c.handler}</code></>} ·{" "}
                                method {c.methodCompatibility}, path {c.pathCompatibility}
                              </li>
                            ))}
                          </ul>
                        </details>
                      )}
                    </li>
                  ))}
                  {group.items.length > 300 && (
                    <li className="bx-muted">…and {group.items.length - 300} more.</li>
                  )}
                </ul>
              </details>
            ))}
            {routeNodes.length > 0 && (
              <details>
                <summary>
                  Endpoints reached without a known handler{" "}
                  <span className="bx-muted">{routeNodes.length}</span>{" "}
                  <span className="bx-status-pill bx-pill-route">route node</span>
                </summary>
                <p className="bx-muted">
                  A request that matched a route whose handler Cartograph could not resolve to one
                  declaration ends at the route itself, rather than at a guessed function.
                </p>
                <ul className="bx-decisions">
                  {routeNodes.slice(0, 300).map((n) => (
                    <li key={n.id}>
                      <button type="button" className="bx-link" onClick={() => { setView("map"); void selectNode(n.id); window.setTimeout(() => focusNode(n.id), 50); }}>
                        {n.label}
                      </button>
                    </li>
                  ))}
                </ul>
              </details>
            )}
            {report.ormAmbiguous.length > 0 && (
              <details>
                <summary>
                  <span className="bx-status-pill bx-pill-ambiguous">ORM</span> {report.ormAmbiguous.length} model
                  name{report.ormAmbiguous.length === 1 ? "" : "s"} not resolved to one declaration
                </summary>
                <ul className="bx-decisions">
                  {report.ormAmbiguous.map((m) => <li key={m}><code>{m}</code></li>)}
                </ul>
              </details>
            )}
            {refusals.length === 0 && routeNodes.length === 0 && report.ormAmbiguous.length === 0 && (
              <p>Cartograph refused nothing in this analysis.</p>
            )}
            {source !== null && <SourcePreview excerpt={source} onClose={() => setSource(null)} />}
          </section>
        )}

        {payload !== null && report !== null && view === "diagnostics" && (
          <section className="bx-list-view" aria-labelledby="bx-diag-title">
            <h1 id="bx-diag-title">Unreadable code</h1>
            <p className="bx-lede">
              Parts of files Cartograph could not read, so anything inside them is missing from the
              map. {report.totals.files_failed} file{report.totals.files_failed === 1 ? "" : "s"} could
              not be read at all; {report.totals.files_with_diagnostics} had a region it skipped.
            </p>
            {report.diagnostics.length === 0 ? (
              <p>No parser diagnostics.</p>
            ) : (
              <table className="bx-table">
                <thead>
                  <tr><th>File</th><th>Line</th><th>Severity</th><th>Kind</th><th>Message</th></tr>
                </thead>
                <tbody>
                  {report.diagnostics.slice(0, 500).map((d, i) => (
                    <tr key={i}>
                      <td><code>{d.file}</code></td>
                      <td>{d.line ?? ""}</td>
                      <td>{d.severity}</td>
                      <td>{d.kind}</td>
                      <td>{d.message}</td>
                    </tr>
                  ))}
                </tbody>
              </table>
            )}
          </section>
        )}
      </main>

      <aside className="bx-inspector" aria-label="Inspector" ref={inspector} tabIndex={-1}>
        {selection.kind === "none" && (
          <div className="bx-inspector-empty">
            <h2>Details</h2>
            <p>Each dot on the map is a piece of code or a database table. Each line is a connection Cartograph can prove.</p>
            <p>Click a dot to see what it is and what it connects to, or a line to see the proof.</p>
            {blast !== null && view === "blast" && <p className="bx-muted">{describeBlast(blast)}</p>}
          </div>
        )}
        {inspectorError !== null && (
          <div className="bx-banner bx-banner-error" role="alert">{inspectorError}</div>
        )}
        {selection.kind === "edge" && evidence !== null && (
          <>
            <div className="bx-plain">
              <p className="bx-eyebrow-dark">{plainEdge(evidence.kind).name}</p>
              <p className="bx-plain-sentence">
                {relationSentence(evidence.kind, evidence.source.label, evidence.target.label)}
              </p>
              <p className="bx-muted">
                Proven by line {evidence.location.line} of <code className="bx-path">{evidence.location.file}</code>.
              </p>
            </div>
            <h3 className="bx-proof-heading">The proof</h3>
            <EvidencePanel record={evidence} onClose={clearSelection} />
            <div className="bx-inspector-actions">
              <button type="button" className="bx-button"
                onClick={() => void showSource(evidence.location.file, evidence.location.line)}>
                Show source at{" "}
                <code className="bx-path">
                  {evidence.location.file}:{evidence.location.line}
                </code>
              </button>
            </div>
          </>
        )}
        {selection.kind === "node" && detail !== null && (
          <div className="bx-node">
            <h2>{detail.label}</h2>
            <p className="bx-plain-sentence">
              {plainKind(detail.kind)} in {PLAIN_LAYER[layerOfNode.get(detail.node) ?? "other"].toLowerCase()}
              {isTestPath(detail.file) ? " (automated test)" : ""}.{" "}
              It uses {detail.outgoing.length} other {detail.outgoing.length === 1 ? "part" : "parts"} and{" "}
              {detail.incoming.length} {detail.incoming.length === 1 ? "part depends" : "parts depend"} on it.
            </p>
            {detail.file !== undefined ? (
              <p>
                <button type="button" className="bx-link" onClick={() => void showSource(detail.file ?? "", detail.line)}>
                  {detail.file}{detail.line !== undefined ? `:${detail.line}` : ""}
                </button>
              </p>
            ) : (
              <p className="bx-muted">Tables have no line of code of their own.</p>
            )}
            <div className="bx-inspector-actions">
              <button type="button" className="bx-button bx-primary" onClick={() => void runTrace(detail.node)}>Follow from here</button>
              <button type="button" className="bx-button" onClick={() => void runBlast(detail.node)}>What depends on this?</button>
              <button type="button" className="bx-button" onClick={() => focusNode(detail.node)}>Show on map</button>
              <button
                type="button"
                className="bx-button"
                onClick={() => {
                  setFocus(neighbourhoodKeys(detail.node, detail.outgoing, detail.incoming));
                  if (view === "trace" || view === "blast") {
                    setView("map");
                  }
                }}
              >
                Only its neighbours
              </button>
            </div>
            <NeighbourList title={`Uses (${detail.outgoing.length})`} items={detail.outgoing} direction="out" onEdge={(e) => void selectEdge(e)} />
            <NeighbourList title={`Used by (${detail.incoming.length})`} items={detail.incoming} direction="in" onEdge={(e) => void selectEdge(e)} />
          </div>
        )}
        {source !== null && view !== "unresolved" && <SourcePreview excerpt={source} onClose={() => setSource(null)} />}
      </aside>
    </div>
  );
}

function NeighbourList({
  title,
  items,
  direction,
  onEdge,
}: {
  title: string;
  items: NodeDetail["outgoing"];
  direction: "in" | "out";
  onEdge: (edge: number) => void;
}) {
  if (items.length === 0) {
    return (
      <div className="bx-neighbours">
        <h3>{title}</h3>
        <p className="bx-muted">Nothing proven.</p>
      </div>
    );
  }
  return (
    <div className="bx-neighbours">
      <h3>{title}</h3>
      <ul>
        {items.slice(0, 100).map((n) => (
          <li key={n.edge}>
            <button type="button" className="bx-link" onClick={() => onEdge(n.edge)} title="Show the proof">
              <EdgeSwatch kind={n.kind} />{" "}
              {direction === "out" && <span className="bx-muted">{plainEdge(n.kind).verb} </span>}
              <span className="bx-neighbour-name">{n.label}</span>{" "}
              {direction === "in" && <span className="bx-muted">{plainEdge(n.kind).incoming}</span>}
            </button>
          </li>
        ))}
        {items.length > 100 && <li className="bx-muted">…and {items.length - 100} more.</li>}
      </ul>
    </div>
  );
}

function TracePanel({
  trace,
  label,
  kindOf,
  edgeKindOf,
  onEdge,
}: {
  trace: TraceResult;
  label: (id: number) => string;
  kindOf: (id: number) => string;
  edgeKindOf: (id: number) => string;
  onEdge: (edge: number) => void;
}) {
  const paths = tracePaths(trace);
  return (
    <div className="bx-trace">
      <p className="bx-eyebrow-dark">Follow a request</p>
      <h2>Where {label(trace.from)} leads</h2>
      {trace.steps.length === 0 ? (
        <p>
          Nothing leads out of this piece of code that Cartograph could prove, so there is nothing
          to follow from here.
        </p>
      ) : (
        <>
          <p className="bx-muted">
            {paths.length >= TRACE_PATH_LIMIT
              ? `${TRACE_PATH_LIMIT} or more routes, up to ${trace.maxDepth} steps each; this list stops at ${TRACE_PATH_LIMIT}.`
              : `${paths.length} ${paths.length === 1 ? "route" : "routes"} through the code, up to ${trace.maxDepth} steps each.`}{" "}
            Click a step to see its proof.
          </p>
          <ol className="bx-paths">
            {paths.slice(0, 50).map((path, i) => (
              <li key={i} className="bx-route">
                <span className="bx-step-node"><code>{label(trace.from)}</code></span>
                {path.hops.map((hop) => (
                  <span key={hop.edge} className="bx-step">
                    <button type="button" className="bx-step-verb" onClick={() => onEdge(hop.edge)} title="Show the proof">
                      {plainEdge(edgeKindOf(hop.edge)).verb}
                    </button>
                    <span className="bx-step-node">
                      <code>{label(hop.to)}</code>{" "}
                      <span className="bx-muted">{plainKind(kindOf(hop.to)).replace(/^An? /, "").toLowerCase()}</span>
                    </span>
                  </span>
                ))}
                <div className="bx-stop">
                  {(() => {
                    const last = path.hops[path.hops.length - 1];
                    const end = last?.to ?? trace.from;
                    const viaHttp = last !== undefined && edgeKindOf(last.edge) === "http-call";
                    return (
                      <>
                        {kindOf(end) === "table"
                          ? "Ends at a database table."
                          : path.stop === "max-depth"
                            ? "Stops after the maximum number of steps."
                            : path.stop === "cycle"
                              ? "Stops where the route loops back on itself."
                              : "Ends here: nothing further could be proven."}
                        {viaHttp && kindOf(end) === "class" &&
                          " If this class is a view, what happens inside it is not followed yet."}
                        <span className="bx-visually-hidden"> ({STOP_MEANING[path.stop ?? "chain-end"]})</span>
                      </>
                    );
                  })()}
                </div>
              </li>
            ))}
          </ol>
          {paths.length > 50 && (
            <p className="bx-muted">
              …and {paths.length >= TRACE_PATH_LIMIT ? "at least " : ""}
              {paths.length - 50} more paths.
            </p>
          )}
        </>
      )}
    </div>
  );
}

function BlastPanel({
  blast,
  label,
  onEdge,
  onNode,
}: {
  blast: BlastResult;
  label: (id: number) => string;
  onEdge: (edge: number) => void;
  onNode: (node: number) => void;
}) {
  const depth = blast.reached.reduce((m, r) => Math.max(m, r.depth), 0);
  return (
    <div className="bx-trace">
      <p className="bx-eyebrow-dark">Impact of a change</p>
      <h2>If you change {label(blast.target)}</h2>
      <p>
        {blast.reached.length === 0
          ? "No other code is proven to depend on it."
          : depth <= 1
            ? `${blast.reached.length} ${blast.reached.length === 1 ? "part of the code depends" : "parts of the code depend"} on it directly.`
            : `${blast.reached.length} parts of the code depend on it, directly or through up to ${depth} steps.`}{" "}
        <span className="bx-muted">
          Only proven connections count; anything listed under “Gaps in the map” could add more.
        </span>
      </p>
      <ol className="bx-paths">
        {[...blast.reached]
          .sort((a, b) => a.depth - b.depth)
          .slice(0, 200)
          .map((r) => (
            <li key={r.node}>
              <button type="button" className="bx-link" onClick={() => onNode(r.node)}><code>{label(r.node)}</code></button>{" "}
              {r.depth > 1 && (
                <span className="bx-muted" title={`Confidence ${r.confidence.toFixed(2)} (an uncalibrated prior, not a probability)`}>
                  {r.depth} steps away
                </span>
              )}{" "}
              <button type="button" className="bx-hop" onClick={() => onEdge(r.via)} title="Show the proof">
                proof
              </button>
            </li>
          ))}
      </ol>
    </div>
  );
}

/** A relationship's colour, taken from the renderer so the legend cannot drift. */
function EdgeSwatch({ kind }: { kind: string }) {
  return <i className="bx-edge-swatch" style={{ background: edgeColor(kind) }} aria-hidden="true" />;
}

/**
 * Cartograph's mark, drawn as the site draws it (docs/.vitepress/theme/
 * CartographMark.vue), without its motion: four corners framing two
 * artefacts and the edge between them.
 */
function CartographMark() {
  return (
    <svg className="bx-mark" viewBox="0 0 48 48" aria-hidden="true" focusable="false">
      <path d="M4 14V4h10" />
      <path d="M34 4h10v10" />
      <path d="M44 34v10H34" />
      <path d="M14 44H4V34" />
      <rect x="10.5" y="24.5" width="8" height="8" />
      <circle cx="35" cy="14.5" r="4.4" />
      <path d="M18.5 28.5 23.5 33.5 32.7 18.3" />
    </svg>
  );
}
