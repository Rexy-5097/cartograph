/**
 * MAP's application shell.
 *
 * This slice establishes the surfaces, not the drawing: choose a repository,
 * wait, and then see either what was found or why it failed. The graph view
 * belongs to the next slice, and the placeholder below says so rather than
 * pretending to be one.
 *
 * The window computes nothing about a repository. It calls two Tauri commands
 * and renders what comes back (ADR-0001, RULE 002).
 */

import {
  useCallback,
  useEffect,
  useMemo,
  useReducer,
  useRef,
  useState,
} from "react";
import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";

import GraphView from "./GraphView";
import { clusterColor, clusterSummary, type BuildDiagnostics } from "./graph";

import {
  type AnalysisPayload,
  type DesktopError,
  initialState,
  isUserCorrectable,
  reduce,
} from "./session";

/**
 * A failure that reached us without a `kind`.
 *
 * The Rust side always returns a structured error, so this only fires if the
 * IPC boundary itself broke. It is classified as `internal` rather than
 * guessed at, because guessing is what the error contract exists to prevent.
 */
function asDesktopError(raw: unknown): DesktopError {
  if (
    typeof raw === "object" &&
    raw !== null &&
    "kind" in raw &&
    "message" in raw
  ) {
    return raw as DesktopError;
  }
  return {
    kind: "internal",
    message: "The analysis could not be started.",
    hint: "This is a defect in Cartograph. Please report it.",
  };
}

export default function App() {
  const [state, dispatch] = useReducer(reduce, initialState);
  const headingRef = useRef<HTMLHeadingElement>(null);

  // Move focus to the heading whenever the surface changes. Without this, a
  // screen-reader user who presses "Choose repository" is left with focus on a
  // button that no longer describes what is on screen, and a keyboard user has
  // to tab from the top again.
  useEffect(() => {
    headingRef.current?.focus();
  }, [state.phase]);

  const choose = useCallback(async () => {
    if (state.phase === "analyzing") return;
    dispatch({ type: "selectRequested" });

    let chosen: string | null;
    try {
      chosen = await open({ directory: true, multiple: false });
    } catch (raw) {
      dispatch({ type: "analysisFailed", error: asDesktopError(raw) });
      return;
    }

    // The dialog returning null is the user pressing Cancel. That is an
    // ordinary outcome, not a failure, and it must not look like one.
    if (typeof chosen !== "string") {
      dispatch({ type: "selectionCancelled" });
      return;
    }

    dispatch({ type: "analysisStarted", repository: chosen });
    try {
      const payload = await invoke<AnalysisPayload>("analyze_repository", {
        path: chosen,
      });
      dispatch({ type: "analysisSucceeded", payload });
    } catch (raw) {
      dispatch({ type: "analysisFailed", error: asDesktopError(raw) });
    }
  }, [state.phase]);

  const busy = state.phase === "analyzing";

  return (
    <div className="app">
      <header className="bar">
        <span className="wordmark">Cartograph</span>
        <span className="milestone">MAP · M11 shell</span>
      </header>

      <main className="stage">
        <h1 className="heading" ref={headingRef} tabIndex={-1}>
          {headingFor(state.phase)}
        </h1>

        {state.phase === "idle" && (
          <p className="prose">
            Choose a repository to analyse. Nothing is uploaded and nothing is
            executed — Cartograph reads source files and builds a graph locally.
          </p>
        )}

        {state.phase === "cancelled" && (
          <p className="prose">No repository was chosen.</p>
        )}

        {busy && (
          <p className="prose" role="status" aria-live="polite">
            Reading files, resolving relationships and computing layout. Large
            repositories take a moment.
          </p>
        )}

        {state.phase === "failed" && <Failure error={state.error} />}

        {state.phase === "ready" && <Result payload={state.payload} />}

        <button
          className="action"
          type="button"
          onClick={() => void choose()}
          disabled={busy}
          aria-busy={busy}
        >
          {busy ? "Analysing…" : "Choose repository"}
        </button>
      </main>
    </div>
  );
}

function headingFor(phase: string): string {
  switch (phase) {
    case "analyzing":
      return "Analysing";
    case "ready":
      return "Analysis complete";
    case "failed":
      return "That did not work";
    case "cancelled":
      return "Nothing selected";
    default:
      return "No repository open";
  }
}

/**
 * A failure, shown with its next action.
 *
 * `role="alert"` rather than a toast: this is the result of something the user
 * just did, and it should be announced immediately rather than appearing
 * quietly beside the button that caused it.
 */
function Failure({ error }: { error: DesktopError }) {
  return (
    <div className="failure" role="alert">
      <p className="failure-message">{error.message}</p>
      {error.hint && <p className="failure-hint">{error.hint}</p>}
      {!isUserCorrectable(error.kind) && (
        <p className="failure-code">
          Reported as <code>{error.kind}</code>.
        </p>
      )}
    </div>
  );
}

/**
 * What was found.
 *
 * Counts and cluster names only. The graph itself is the next slice, and the
 * placeholder says so — a fake preview would be worse than an honest gap.
 */
function Result({ payload }: { payload: AnalysisPayload }) {
  const { summary, scene } = payload;
  const [diagnostics, setDiagnostics] = useState<BuildDiagnostics | null>(null);
  const largest = useMemo(() => clusterSummary(scene).slice(0, 8), [scene]);

  return (
    <div className="result">
      <p className="repository">{payload.repository}</p>

      <GraphView scene={scene} onDiagnostics={setDiagnostics} />

      {diagnostics !== null &&
        (diagnostics.skippedNodes > 0 || diagnostics.skippedEdges > 0) && (
          <p className="note" role="status">
            {diagnostics.skippedNodes > 0 &&
              `${diagnostics.skippedNodes} node${diagnostics.skippedNodes === 1 ? "" : "s"} `}
            {diagnostics.skippedNodes > 0 && diagnostics.skippedEdges > 0 && "and "}
            {diagnostics.skippedEdges > 0 &&
              `${diagnostics.skippedEdges} edge${diagnostics.skippedEdges === 1 ? "" : "s"} `}
            could not be drawn: {diagnostics.reasons.join("; ")}.
          </p>
        )}

      <dl className="counts">
        <Count label="Files" value={summary.files} />
        <Count label="Nodes" value={summary.nodes} />
        <Count label="Edges" value={summary.edges} />
        <Count label="Clusters" value={summary.clusters} />
        {summary.failed > 0 && <Count label="Unparsed" value={summary.failed} />}
      </dl>

      {summary.failed > 0 && (
        <p className="note">
          {summary.failed} file{summary.failed === 1 ? "" : "s"} could not be
          parsed. Analysis continued without them.
        </p>
      )}

      {/*
        The canvas is a WebGL surface and is not semantically readable, so the
        map is not the only way to reach this information. This list is the
        accessible account of the same clustering, and it is always present
        rather than being an assistive-technology afterthought.
      */}
      <h2 className="subheading">Largest clusters</h2>
      <ul className="clusters">
        {largest.map((cluster) => (
          <li key={cluster.id}>
            <span
              className="cluster-swatch"
              style={{ background: clusterColor(cluster.id) }}
              aria-hidden="true"
            />
            <span className="cluster-label">{cluster.label}</span>
            <span className="cluster-count">{cluster.count}</span>
          </li>
        ))}
      </ul>
    </div>
  );
}

function Count({ label, value }: { label: string; value: number }) {
  return (
    <div className="count">
      <dt>{label}</dt>
      <dd>{value.toLocaleString()}</dd>
    </div>
  );
}
