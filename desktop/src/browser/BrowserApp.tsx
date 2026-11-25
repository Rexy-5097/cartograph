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
import { describeBlast } from "../blast";
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
  LAYER_LABEL,
  type Layer,
  STATUS_MEANING,
  STOP_MEANING,
  edgeKinds,
  groupDecisions,
  layers as layerMap,
  locations,
  nodeKinds,
  overviewFigures,
  tally,
  traceKeys,
  tracePaths,
} from "./model";
import SourcePreview from "./SourcePreview";

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
  database: ["database", "model"],
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

  const sigma = useRef<Sigma | null>(null);
  const onSigma = useCallback((instance: Sigma | null) => {
    sigma.current = instance;
  }, []);

  const analysisId = payload?.analysis ?? null;

  // ── Loading and refreshing ────────────────────────────────────────────
  const load = useCallback(async () => {
    if (api === null) {
      return;
    }
    try {
      const [nextPayload, nextReport] = await Promise.all([api.analysis(), api.report()]);
      if (nextPayload.analysis !== nextReport.analysis) {
        // Two different analyses: the next poll will bring a matching pair.
        return;
      }
      setPayload(nextPayload);
      setReport(nextReport);
      setSelection({ kind: "none" });
      setEvidence(null);
      setDetail(null);
      setSource(null);
      setTrace(null);
      setBlast(null);
      setLoadError(null);
    } catch (error) {
      setLoadError(errorText(error));
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
        if (next.analysis !== null && next.analysis !== analysisId) {
          await load();
        }
        if (next.phase === "running" || next.analysis === null) {
          timer = window.setTimeout(poll, 600);
        }
      } catch (error) {
        if (!cancelled) {
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
      view === "trace" ? traceHighlight : view === "blast" ? null : viewHighlight;
    return {
      hiddenNodeKinds,
      hiddenEdgeKinds: hiddenKinds,
      hiddenClusters,
      highlight,
      selectedNode: selection.kind === "node" ? String(selection.id) : null,
      selectedEdge: selection.kind === "edge" ? `e${selection.id}` : null,
      colorForKind: null,
    };
  }, [view, traceHighlight, viewHighlight, hiddenKinds, selection]);

  const layerView = useMemo<GraphViewOptions>(() => {
    const out: GraphViewOptions = { ...graphView };
    if (hiddenLayers.size > 0 || colourBy === "layer") {
      const layerByKey = new Map<string, Layer>();
      for (const [id, layer] of layerOfNode) {
        layerByKey.set(String(id), layer);
      }
      out.nodeStyle = (key: string) => {
        const layer = layerByKey.get(key);
        if (layer === undefined) {
          return {};
        }
        return {
          hidden: hiddenLayers.has(layer),
          color: colourBy === "layer" ? LAYER_COLOR[layer] : undefined,
        };
      };
    }
    return out;
  }, [graphView, hiddenLayers, colourBy, layerOfNode]);

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
      try {
        const record = await api.evidence(analysisId, edge);
        // Discard an answer for an analysis that is no longer on screen.
        if (record.analysis === analysisId) {
          setEvidence(record);
        }
      } catch (error) {
        setEvidence(null);
        setInspectorError(errorText(error).message);
      }
    },
    [api, analysisId],
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
      try {
        const found = await api.node(analysisId, node);
        if (found.analysis === analysisId) {
          setDetail(found);
        }
      } catch (error) {
        setDetail(null);
        setInspectorError(errorText(error).message);
      }
    },
    [api, analysisId],
  );

  const clearSelection = useCallback(() => {
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

  const showSource = async (file: string, line?: number) => {
    if (api === null || analysisId === null) {
      return;
    }
    try {
      const found = await api.source(analysisId, file, line);
      if (found.analysis === analysisId) {
        setSource(found);
      }
    } catch (error) {
      setInspectorError(errorText(error).message);
    }
  };

  const runTrace = async (node: number) => {
    if (api === null || analysisId === null) {
      return;
    }
    try {
      const found = await api.trace(analysisId, node);
      if (found.analysis === analysisId) {
        setTrace(found);
        setBlast(null);
        setView("trace");
      }
    } catch (error) {
      setInspectorError(errorText(error).message);
    }
  };

  const runBlast = async (node: number) => {
    if (api === null || analysisId === null) {
      return;
    }
    try {
      const found = await api.blast(analysisId, node);
      if (found.analysis === analysisId) {
        setBlast(found);
        setTrace(null);
        setView("blast");
      }
    } catch (error) {
      setInspectorError(errorText(error).message);
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
  const totals = report?.totals ?? null;
  const decisions = report?.decisions ?? [];
  const refusals = decisions.filter((d) => d.status !== "strong" && d.status !== "exact");
  const routeNodes = scene?.nodes.filter((n) => n.kind === "route") ?? [];

  const navItems: { id: View; label: string; count?: number; empty?: boolean }[] = [
    { id: "overview", label: "Architecture overview" },
    { id: "map", label: "Full map", count: scene?.nodes.length },
    {
      id: "frontend",
      label: "Frontend",
      count: layerCounts.find(([l]) => l === "frontend")?.[1] ?? 0,
    },
    { id: "api", label: "API and routes", count: totals?.routes ?? 0 },
    {
      id: "backend",
      label: "Backend",
      count:
        (layerCounts.find(([l]) => l === "backend")?.[1] ?? 0) +
        (layerCounts.find(([l]) => l === "model")?.[1] ?? 0),
    },
    { id: "database", label: "Database", count: totals?.orm_tables ?? 0 },
    {
      id: "dependencies",
      label: "Dependencies",
      count: scene?.edges.filter((e) => e.kind === "import" || e.kind === "call").length ?? 0,
    },
    { id: "trace", label: "Trace" },
    { id: "blast", label: "Blast radius" },
    { id: "unresolved", label: "Unresolved", count: refusals.length + routeNodes.length },
    { id: "diagnostics", label: "Diagnostics", count: report?.diagnostics.length ?? 0 },
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
    <div className="bx-app">
      <header className="bx-top">
        <div className="bx-brand">
          <span className="bx-logo" aria-hidden="true">◈</span>
          <span className="bx-name">Cartograph</span>
          <span className="bx-sub">Architecture Explorer</span>
          <span className="bx-experimental" title="Proposed ADR-0024; not accepted">
            experimental
          </span>
        </div>
        <div className="bx-repo" title="Repository being explored">
          {status?.repository ?? payload?.repository ?? "…"}
        </div>
        <div className={`bx-status bx-status-${phase}`} role="status" aria-live="polite">
          {phase === "running"
            ? "Analysing…"
            : phase === "failed"
              ? "Last analysis failed"
              : analysisId === null
                ? "Waiting for analysis"
                : `Analysis #${analysisId} ready`}
        </div>
        <button
          type="button"
          className="bx-button"
          onClick={() => void refresh()}
          disabled={phase === "running"}
        >
          Refresh analysis
        </button>
        <div className="bx-search">
          <label htmlFor="bx-search-input" className="bx-visually-hidden">
            Search symbols
          </label>
          <input
            id="bx-search-input"
            type="search"
            placeholder="Search symbols…"
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
                    }}
                  >
                    <span>{m.label}</span>
                    <span className="bx-kind">{m.kind}</span>
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
              <legend>Colour nodes by</legend>
              <label>
                <input type="radio" name="colour" checked={colourBy === "layer"}
                  onChange={() => setColourBy("layer")} /> Layer (file language)
              </label>
              <label>
                <input type="radio" name="colour" checked={colourBy === "cluster"}
                  onChange={() => setColourBy("cluster")} /> Cluster (Rust layout)
              </label>
            </fieldset>
          </div>
        )}
      </header>

      <nav className="bx-nav" aria-label="Views">
        <ul>
          {navItems.map((item) => (
            <li key={item.id}>
              <button
                type="button"
                className={view === item.id ? "bx-nav-item bx-nav-current" : "bx-nav-item"}
                aria-current={view === item.id ? "page" : undefined}
                onClick={() => setView(item.id)}
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
          Counts come from the Rust analysis. “none” means Cartograph detected nothing of that
          kind — not that the codebase has none.
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
        {payload === null && loadError === null && (
          <div className="bx-loading" role="status">
            <div className="bx-spinner" aria-hidden="true" />
            <p>Cartograph is analysing the repository. The map appears when it finishes.</p>
          </div>
        )}

        {payload !== null && report !== null && view === "overview" && (
          <section className="bx-overview" aria-labelledby="bx-overview-title">
            <h1 id="bx-overview-title">{payload.repository}</h1>
            <p className="bx-lede">
              Analysis #{payload.analysis}, {report.elapsedMs} ms. Every figure below is a count
              from Cartograph’s Rust analysis of this repository; nothing is estimated.
            </p>
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
                      <span><i className={`bx-edge-swatch bx-edge-${k}`} />{EDGE_LABEL[k] ?? k}</span>
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
            <button type="button" className="bx-button bx-primary" onClick={() => setView("map")}>
              Open the architecture map
            </button>
          </section>
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
              <h2>Layers</h2>
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
                      {LAYER_LABEL[layer]} <span className="bx-muted">{n}</span>
                    </label>
                  </li>
                ))}
              </ul>
              <h2>Relationships</h2>
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
                      <i className={`bx-edge-swatch bx-edge-${kind}`} />
                      {EDGE_LABEL[kind] ?? kind} <span className="bx-muted">{n}</span>
                    </label>
                  </li>
                ))}
              </ul>
            </aside>
            {view === "trace" && (
              <div className="bx-overlay" aria-label="Trace">
                {trace === null ? (
                  <p>Select a node on the map, then choose <strong>Trace from here</strong> in the inspector.</p>
                ) : (
                  <TracePanel
                    trace={trace}
                    label={label}
                    kindOf={(id) => nodeById.get(id)?.kind ?? ""}
                    onEdge={(e) => void selectEdge(e)}
                  />
                )}
              </div>
            )}
            {view === "blast" && (
              <div className="bx-overlay" aria-label="Blast radius">
                {blast === null ? (
                  <p>Select a node on the map, then choose <strong>Blast radius</strong> in the inspector.</p>
                ) : (
                  <BlastPanel blast={blast} label={label} onEdge={(e) => void selectEdge(e)} onNode={(n) => void selectNode(n)} />
                )}
              </div>
            )}
          </section>
        )}

        {payload !== null && report !== null && view === "unresolved" && (
          <section className="bx-list-view" aria-labelledby="bx-unresolved-title">
            <h1 id="bx-unresolved-title">Unresolved relationships</h1>
            <p className="bx-lede">
              Cartograph draws a relationship only when the source determines it. These are the
              places it declined to. An unresolved request is <em>not</em> proof that no
              connection exists — only that the static evidence does not establish one.
            </p>
            {groupDecisions(refusals).map((group) => (
              <details key={group.status} open={group.items.length <= 25}>
                <summary>
                  <span className={`bx-status-pill bx-pill-${group.status}`}>{group.status}</span>{" "}
                  {group.items.length} request{group.items.length === 1 ? "" : "s"}
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
                        <ul className="bx-candidates">
                          {d.candidates.slice(0, 5).map((c, j) => (
                            <li key={j}>
                              candidate <code>{c.route}</code> at {c.file}:{c.line}
                              {c.handler !== undefined && <> → <code>{c.handler}</code></>} ·{" "}
                              method {c.methodCompatibility}, path {c.pathCompatibility}
                            </li>
                          ))}
                        </ul>
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
                  <span className="bx-status-pill bx-pill-route">route node</span> {routeNodes.length} route
                  {routeNodes.length === 1 ? "" : "s"} drawn without a resolved handler
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
            <h1 id="bx-diag-title">Diagnostics</h1>
            <p className="bx-lede">
              {report.totals.files_failed} file{report.totals.files_failed === 1 ? "" : "s"} failed to
              parse; {report.totals.files_with_diagnostics} had diagnostics. Files with no supported
              language are not counted here.
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

      <aside className="bx-inspector" aria-label="Inspector">
        {selection.kind === "none" && (
          <div className="bx-inspector-empty">
            <h2>Inspector</h2>
            <p>Click a relationship on the map to see the evidence Cartograph used to establish it, or a node to see where it is and what it connects to.</p>
            {blast !== null && view === "blast" && <p className="bx-muted">{describeBlast(blast)}</p>}
          </div>
        )}
        {inspectorError !== null && (
          <div className="bx-banner bx-banner-error" role="alert">{inspectorError}</div>
        )}
        {selection.kind === "edge" && evidence !== null && (
          <>
            <EvidencePanel record={evidence} onClose={clearSelection} />
            <div className="bx-inspector-actions">
              <button type="button" className="bx-button"
                onClick={() => void showSource(evidence.location.file, evidence.location.line)}>
                Show source at {evidence.location.file}:{evidence.location.line}
              </button>
            </div>
          </>
        )}
        {selection.kind === "node" && detail !== null && (
          <div className="bx-node">
            <h2>{detail.label}</h2>
            <p>
              <span className="bx-kind">{detail.kind}</span>{" "}
              <span className="bx-muted">
                {LAYER_LABEL[layerOfNode.get(detail.node) ?? "other"]}
              </span>
            </p>
            {detail.file !== undefined ? (
              <p>
                <button type="button" className="bx-link" onClick={() => void showSource(detail.file ?? "", detail.line)}>
                  {detail.file}{detail.line !== undefined ? `:${detail.line}` : ""}
                </button>
              </p>
            ) : (
              <p className="bx-muted">This artefact has no source location in the graph.</p>
            )}
            <div className="bx-inspector-actions">
              <button type="button" className="bx-button bx-primary" onClick={() => void runTrace(detail.node)}>Trace from here</button>
              <button type="button" className="bx-button" onClick={() => void runBlast(detail.node)}>Blast radius</button>
              <button type="button" className="bx-button" onClick={() => focusNode(detail.node)}>Centre on map</button>
            </div>
            <NeighbourList title={`Outgoing (${detail.outgoing.length})`} items={detail.outgoing} arrow="→" onEdge={(e) => void selectEdge(e)} />
            <NeighbourList title={`Incoming (${detail.incoming.length})`} items={detail.incoming} arrow="←" onEdge={(e) => void selectEdge(e)} />
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
  arrow,
  onEdge,
}: {
  title: string;
  items: NodeDetail["outgoing"];
  arrow: string;
  onEdge: (edge: number) => void;
}) {
  if (items.length === 0) {
    return (
      <div className="bx-neighbours">
        <h3>{title}</h3>
        <p className="bx-muted">None established.</p>
      </div>
    );
  }
  return (
    <div className="bx-neighbours">
      <h3>{title}</h3>
      <ul>
        {items.slice(0, 100).map((n) => (
          <li key={n.edge}>
            <button type="button" className="bx-link" onClick={() => onEdge(n.edge)}>
              <i className={`bx-edge-swatch bx-edge-${n.kind}`} /> {EDGE_LABEL[n.kind] ?? n.kind} {arrow}{" "}
              {n.label} <span className="bx-kind">{n.nodeKind}</span>
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
  onEdge,
}: {
  trace: TraceResult;
  label: (id: number) => string;
  kindOf: (id: number) => string;
  onEdge: (edge: number) => void;
}) {
  const paths = tracePaths(trace);
  return (
    <div className="bx-trace">
      <h2>Trace from {label(trace.from)}</h2>
      {trace.steps.length === 0 ? (
        <p>
          The trace stopped immediately: {STOP_MEANING[trace.stop ?? "chain-end"]}. Cartograph
          established no relationship leading out of this artefact.
        </p>
      ) : (
        <>
          <p className="bx-muted">
            {paths.length} path{paths.length === 1 ? "" : "s"} within depth {trace.maxDepth}. Click a
            hop for its evidence.
          </p>
          <ol className="bx-paths">
            {paths.slice(0, 50).map((path, i) => (
              <li key={i}>
                <span className="bx-hop-node">{label(trace.from)}</span>
                {path.hops.map((hop) => (
                  <span key={hop.edge}>
                    <button type="button" className="bx-hop" onClick={() => onEdge(hop.edge)} title="Show evidence">
                      →
                    </button>
                    <span className="bx-hop-node">
                      {label(hop.to)} <span className="bx-kind">{kindOf(hop.to)}</span>
                    </span>
                  </span>
                ))}
                <div className="bx-stop">
                  Stopped: {STOP_MEANING[path.stop ?? "chain-end"]}.
                  {kindOf(path.hops[path.hops.length - 1]?.to ?? trace.from) !== "table" &&
                    " This path does not reach a database table in the analysed graph."}
                </div>
              </li>
            ))}
          </ol>
          {paths.length > 50 && <p className="bx-muted">…and {paths.length - 50} more paths.</p>}
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
      <h2>Blast radius of {label(blast.target)}</h2>
      <p>
        {blast.reached.length} artefact{blast.reached.length === 1 ? "" : "s"} depend on it, up to{" "}
        {depth} hop{depth === 1 ? "" : "s"} away, through calls, HTTP calls, ORM access and table
        queries Cartograph established. Imports are not followed. Dependencies Cartograph did not
        resolve (see <em>Unresolved</em>) cannot appear here.
      </p>
      <ol className="bx-paths">
        {[...blast.reached]
          .sort((a, b) => a.depth - b.depth)
          .slice(0, 200)
          .map((r) => (
            <li key={r.node}>
              <button type="button" className="bx-link" onClick={() => onNode(r.node)}>{label(r.node)}</button>{" "}
              <span className="bx-muted">depth {r.depth}, confidence {r.confidence.toFixed(2)} (uncalibrated)</span>{" "}
              <button type="button" className="bx-hop" onClick={() => onEdge(r.via)} title="Evidence for the edge reached through">
                evidence
              </button>
            </li>
          ))}
      </ol>
    </div>
  );
}
