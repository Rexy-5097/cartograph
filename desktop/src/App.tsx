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

import EvidencePanel from "./EvidencePanel";
import GraphView from "./GraphView";
import { describeBlast } from "./blast";
import { findNodes } from "./search";
import { clusterColor, clusterSummary, type BuildDiagnostics } from "./graph";

import {
  type AnalysisPayload,
  type AskAnswer,
  type BlastResult,
  type DesktopError,
  type EvidenceRecord,
  initialState,
  isUserCorrectable,
  reduce,
} from "./session";

/**
 * How many search results to list at once.
 *
 * A bare substring can match hundreds of nodes; a list that long is a wall
 * rather than a choice. The count of what is hidden is shown, so nobody
 * concludes a node is absent when it is merely further down.
 */
const MAX_MATCHES = 8;

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
  const [selected, setSelected] = useState<EvidenceRecord | null>(null);
  const [selectionError, setSelectionError] = useState<DesktopError | null>(null);
  const [blast, setBlast] = useState<BlastResult | null>(null);
  const [blastPending, setBlastPending] = useState(false);
  const [answer, setAnswer] = useState<AskAnswer | null>(null);
  // The only thing the window is told about the grant: a boolean.
  // The repository identity stays in Rust (ADR-0020 Amendment 3).
  const [askEnabled, setAskEnabled] = useState(false);
  const [askPending, setAskPending] = useState(false);
  const largest = useMemo(() => clusterSummary(scene).slice(0, 8), [scene]);

  // A selection belongs to one analysis. When the payload is replaced the old
  // selection must go, or the panel would describe a graph nobody is looking
  // at. Rust refuses a stale lookup as well — this is the interface half of
  // that guarantee, not the whole of it.
  useEffect(() => {
    blastRequest.current += 1;
    askRequest.current += 1;
    setSelected(null);
    setSelectionError(null);
    // Off until Rust says otherwise, so a failed read shows "off" rather than
    // the previous repository's answer.
    setAskEnabled(false);
    void invoke<boolean>("ask_enabled")
      .then(setAskEnabled)
      .catch(() => setAskEnabled(false));
    setBlast(null);
    setBlastPending(false);
    setAnswer(null);
    setAskPending(false);
  }, [payload.analysis]);

  const selectEdge = useCallback(
    async (edge: number) => {
      setSelectionError(null);
      try {
        const record = await invoke<EvidenceRecord>("edge_evidence", {
          analysis: payload.analysis,
          edge,
        });
        setAnswer(null);
        setSelected(record);
      } catch (raw) {
        // A refused lookup must not leave the previous edge's evidence on
        // screen next to a new error: that reads as evidence *for* the thing
        // that just failed.
        setSelected(null);
        setSelectionError(asDesktopError(raw));
      }
    },
    [payload.analysis],
  );

  /**
   * Requests the blast radius of a node.
   *
   * Two guards, and both are needed. Rust refuses a query whose `AnalysisId`
   * is not the current one, which catches a graph replacement. This token
   * catches the other case Rust cannot see: two queries in flight against the
   * *same* analysis, where the slower one would otherwise land last and paint
   * a highlight for a node the user has already moved off.
   */
  const blastRequest = useRef(0);

  const blastNode = useCallback(
    async (node: number) => {
      const token = blastRequest.current + 1;
      blastRequest.current = token;
      setSelectionError(null);
      setBlastPending(true);
      try {
        const result = await invoke<BlastResult>("blast_radius", {
          analysis: payload.analysis,
          node,
        });
        if (blastRequest.current !== token) {
          return; // superseded; a later request owns the view
        }
        setBlast(result);
      } catch (raw) {
        if (blastRequest.current !== token) {
          return;
        }
        setBlast(null);
        setSelectionError(asDesktopError(raw));
      } finally {
        if (blastRequest.current === token) {
          setBlastPending(false);
        }
      }
    },
    [payload.analysis],
  );

  /**
   * Asks for the derived evidence behind one artefact.
   *
   * The degraded ASK path: no model, no key, no network. The same two guards
   * `blastNode` needs, for the same reasons — Rust refuses a stale
   * `AnalysisId`, and this token stops a slower answer landing after a faster
   * one the user has already moved past.
   */
  const askRequest = useRef(0);

  const askNode = useCallback(
    async (node: number) => {
      const token = askRequest.current + 1;
      askRequest.current = token;
      setSelectionError(null);
      setAskPending(true);
      try {
        const result = await invoke<AskAnswer>("ask_evidence", {
          analysis: payload.analysis,
          node,
        });
        if (askRequest.current !== token) {
          return; // superseded; a later request owns the panel
        }
        // One panel, one subject: an explanation replaces a single-edge
        // selection rather than stacking on top of it.
        setSelected(null);
        setAnswer(result);
      } catch (raw) {
        if (askRequest.current !== token) {
          return;
        }
        setAnswer(null);
        setSelectionError(asDesktopError(raw));
      } finally {
        if (askRequest.current === token) {
          setAskPending(false);
        }
      }
    },
    [payload.analysis],
  );

  const clearAnswer = useCallback(() => {
    askRequest.current += 1;
    setAnswer(null);
    setAskPending(false);
  }, []);

  const clearBlast = useCallback(() => {
    // Invalidate anything in flight, so a response that has not arrived yet
    // cannot repaint after the user cleared.
    blastRequest.current += 1;
    askRequest.current += 1;
    setBlast(null);
    setBlastPending(false);
    setAnswer(null);
    setAskPending(false);
  }, []);

  const clearSelection = useCallback(() => {
    blastRequest.current += 1;
    askRequest.current += 1;
    setBlast(null);
    setBlastPending(false);
    setAnswer(null);
    setAskPending(false);
    setSelected(null);
    setSelectionError(null);
  }, []);

  // Search is a way to reach a node, not a second kind of selection: the only
  // thing it holds is what the user typed.
  const [query, setQuery] = useState("");
  const matches = useMemo(() => findNodes(scene.nodes, query), [scene.nodes, query]);

  return (
    <div className="result">
      <p className="repository">{payload.repository}</p>

      <div className="node-search">
        <label htmlFor="node-search-input">Find an artefact</label>
        <input
          id="node-search-input"
          type="search"
          value={query}
          placeholder="Name, e.g. Connection"
          autoComplete="off"
          onChange={(event) => setQuery(event.target.value)}
        />
        {query.trim() !== "" && (
          matches.length === 0 ? (
            <p className="note" role="status">
              No artefact here is called that.
            </p>
          ) : (
            <>
              <ul className="node-search-results">
                {matches.slice(0, MAX_MATCHES).map((match) => (
                  <li key={match.id}>
                    {/* The same handler a click uses. Search decides *which*
                        node; everything after that is the ordinary path. */}
                    <button type="button" onClick={() => void blastNode(match.id)}>
                      <span className="match-label">{match.label}</span>
                      <span className="match-kind">{match.kind}</span>
                    </button>
                  </li>
                ))}
              </ul>
              {matches.length > MAX_MATCHES && (
                <p className="note" role="status">
                  Showing {MAX_MATCHES} of {matches.length}. Type more to narrow it.
                </p>
              )}
            </>
          )
        )}
      </div>

      <GraphView
        scene={scene}
        onDiagnostics={setDiagnostics}
        onSelectEdge={(edge) => void selectEdge(edge)}
        onClearSelection={clearSelection}
        onSelectNode={(node) => void blastNode(node)}
        blast={blast}
      />

      {blastPending && (
        <p className="note" role="status">
          Computing what depends on that artefact…
        </p>
      )}

      <p className="note ask-optin">
        <label>
          <input
            type="checkbox"
            checked={askEnabled}
            onChange={(event) => {
              const wanted = event.target.checked;
              void invoke<boolean>("set_ask_enabled", { enabled: wanted })
                .then(setAskEnabled)
                .catch(() => setAskEnabled(false));
            }}
          />{" "}
          Allow AI explanations for this repository
        </label>{" "}
        <span className="evidence-kind">
          {askEnabled ? "on" : "off"}
        </span>
        <span className="evidence-caveat">
          answers are derived evidence until a model is configured
        </span>
      </p>

      {askPending && (
        <p className="note" role="status">
          Gathering the evidence behind that artefact…
        </p>
      )}

      {blast !== null && (
        <div className="blast-summary" role="status">
          <p>{describeBlast(blast)}</p>
          {!blast.calibrated && blast.reached.length > 0 && (
            <p className="evidence-footnote">
              Confidence is the weakest step of each route — an uncalibrated
              prior, not a probability. One representative route is shown per
              artefact; others may exist.
            </p>
          )}
          <button type="button" onClick={() => void askNode(blast.target)}>
            Explain this artefact
          </button>
          <button type="button" onClick={clearBlast}>
            Clear blast radius
          </button>
        </div>
      )}

      {selected !== null && (
        <EvidencePanel record={selected} onClose={clearSelection} />
      )}

      {answer !== null && (
        <EvidencePanel answer={answer} onClose={clearAnswer} />
      )}

      {selectionError !== null && (
        <p className="note evidence-error" role="alert">
          {selectionError.message}
          {selectionError.hint !== undefined && ` ${selectionError.hint}`}
        </p>
      )}

      {selected === null && selectionError === null && summary.edges > 0 && (
        <p className="note evidence-prompt">
          Select a relationship on the map to see the evidence behind it.
        </p>
      )}

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
