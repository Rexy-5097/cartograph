/**
 * The contract this window shares with the Rust core.
 *
 * Every type here mirrors one in `crates/cartograph-desktop`, and the Rust
 * test suite pins the wire names — `error_kinds_serialise_under_their_agreed_names`
 * and `phase_names_match_the_frontend_union` fail the build if either side
 * drifts. That is deliberate: a mismatch between these unions and the Rust
 * enums would not throw, it would simply stop matching, and a branch that
 * quietly stops being taken is the hardest kind of bug to see.
 *
 * Nothing in this file computes anything about a repository. Layout,
 * clustering and analysis all happen in Rust (ADR-0001, ADR-0015); this
 * describes what arrives.
 */

/** Where a session has got to. Mirrors `cartograph_desktop::Phase`. */
export type Phase =
  | "idle"
  | "selecting"
  | "analyzing"
  | "ready"
  | "failed"
  | "cancelled";

/**
 * Why something failed. Mirrors `cartograph_desktop::error::DesktopErrorKind`.
 *
 * **Branch on this, never on `message`.** The message is written to be shown
 * to a person and may be reworded at any time.
 */
export type DesktopErrorKind =
  | "notFound"
  | "notADirectory"
  | "permissionDenied"
  | "noSupportedSources"
  | "analysisFailed"
  | "cancelled"
  | "staleSelection"
  | "unknownEdge"
  | "unknownNode"
  | "noAnalysis"
  | "internal";

/** A failure, ready to show and ready to branch on. */
export interface DesktopError {
  kind: DesktopErrorKind;
  /** One sentence for the user. Never contains an absolute path. */
  message: string;
  /** The next action, when there is an obvious one. */
  hint?: string;
}

/** Whether choosing a different folder could plausibly fix this. */
export function isUserCorrectable(kind: DesktopErrorKind): boolean {
  return (
    kind === "notFound" ||
    kind === "notADirectory" ||
    kind === "permissionDenied" ||
    kind === "noSupportedSources"
  );
}

/**
 * What an artefact is. Mirrors `cartograph_core::NodeKind`, kebab-cased.
 *
 * Used for colour and size only. Nothing here decides what a node *means* —
 * that was settled in Rust before this arrived.
 */
export type NodeKind =
  | "repository"
  | "package"
  | "directory"
  | "file"
  | "module"
  | "class"
  | "function"
  | "method"
  | "variable"
  | "route"
  | "table"
  | "column"
  | "external-service"
  | "env-var";

/** What a relationship is. Mirrors `cartograph_core::EdgeKind`, kebab-cased. */
export type EdgeKind =
  | "import"
  | "call"
  | "inherits"
  | "implements"
  | "references"
  | "http-call"
  | "orm-access"
  | "queries";

/**
 * One node, ready to draw.
 *
 * `x`, `y` and `cluster` are computed in Rust (ADR-0001, ADR-0015) and are
 * **used exactly as received**. No code in this application may recompute
 * them; `graph.test.ts` asserts that they arrive in Graphology unchanged.
 */
export interface SceneNode {
  id: number;
  x: number;
  y: number;
  cluster: number;
  label: string;
  kind: NodeKind;
}

/** One edge, ready to draw. */
export interface SceneEdge {
  id: number;
  source: number;
  target: number;
  kind: EdgeKind;
}

/** Which analysis a payload or selection belongs to. */
export type AnalysisId = number;

/** One endpoint of a relationship. */
export interface Endpoint {
  id: number;
  label: string;
  kind: NodeKind;
}

/** Where a claim was observed. Repository-relative, never absolute. */
export interface EvidenceLocation {
  file: string;
  line: number;
  column: number | null;
}

/**
 * Everything Cartograph knows about one edge.
 *
 * Every field is read off the edge in Rust. Nothing here is computed, and
 * nothing may be computed from it: this file describes what arrives.
 */
export interface EvidenceRecord {
  analysis: AnalysisId;
  edge: number;
  kind: EdgeKind;
  source: Endpoint;
  target: Endpoint;
  /**
   * The raw value. **Not a probability** — an uncalibrated prior selected by
   * evidence class (M08: ECE 0.18, low bands unverified). `calibrated` says so
   * from the data; the panel must not render this as a likelihood.
   */
  confidence: number;
  calibrated: boolean;
  provenance: string;
  /** Whether the producing analysis computes rather than estimates. */
  deterministic: boolean;
  evidence: string;
  location: EvidenceLocation;
}

/**
 * One artefact that depends on the blast-radius target.
 *
 * Mirrors `cartograph_desktop::blast::ImpactedNode`, and carries the same
 * field names as `cartograph blast --json`: one contract, two renderings.
 */
export interface ImpactedNode {
  node: number;
  /** Hops to the target along the reported route. Always at least 1. */
  depth: number;
  /**
   * Confidence of the best-supported route — the minimum along it, maximised
   * over routes (ADR-0018). **Not a probability**; see `calibrated`.
   */
  confidence: number;
  /** The edge this artefact reaches the target through, on that route. */
  via: number;
}

/**
 * What depends on a selected artefact.
 *
 * Computed entirely in Rust. Nothing in this application may recompute it, and
 * nothing may infer additional impact from it.
 */
export interface BlastResult {
  /** The analysis this belongs to; a response for any other must be discarded. */
  analysis: AnalysisId;
  /** The artefact queried. Never appears in `reached`. */
  target: number;
  reached: ImpactedNode[];
  /** Always false: an uncalibrated prior, not a likelihood. */
  calibrated: boolean;
  /** Always "representative": one route per artefact, not all of them. */
  routes: string;
}

/** A named group of nodes. */
export interface Cluster {
  id: number;
  label: string;
}

/**
 * Everything the renderer draws.
 *
 * Deliberately carries no confidence, provenance or evidence: that is the
 * evidence record, and opening it is Slice 4's work.
 */
export interface Scene {
  nodes: SceneNode[];
  edges: SceneEdge[];
  clusters: Cluster[];
}

/** What was found, in the quantities the window shows. */
export interface AnalysisSummary {
  files: number;
  failed: number;
  nodes: number;
  edges: number;
  clusters: number;
}

/** A completed analysis. */
export interface AnalysisPayload {
  /**
   * Identifies this analysis. Must be passed back with any evidence lookup:
   * edge ids restart at zero per analysis, so without it a stale selection
   * would resolve against the new graph and return wrong evidence.
   */
  analysis: AnalysisId;
  /** Safe-to-show repository name. Never an absolute path. */
  repository: string;
  summary: AnalysisSummary;
  scene: Scene;
}

/**
 * The window's state.
 *
 * A discriminated union rather than a bag of optional fields, so "ready with
 * no payload" and "failed with no error" cannot be represented at all. There
 * is no state library here: this is one value, and `useReducer` holds it
 * without ceremony. Zustand is in M11's long-term stack and will earn its
 * place when there is cross-panel state to share; adding it for a single
 * session object would be cargo.
 */
export type SessionState =
  | { phase: "idle" }
  | { phase: "selecting" }
  | { phase: "analyzing"; repository: string }
  | { phase: "ready"; payload: AnalysisPayload }
  | { phase: "failed"; error: DesktopError }
  | { phase: "cancelled" };

export type SessionEvent =
  | { type: "selectRequested" }
  | { type: "selectionCancelled" }
  | { type: "analysisStarted"; repository: string }
  | { type: "analysisSucceeded"; payload: AnalysisPayload }
  | { type: "analysisFailed"; error: DesktopError }
  | { type: "reset" };

/**
 * The transition table.
 *
 * Written as an exhaustive switch so an added event is a TypeScript error
 * rather than a silently ignored action. Unknown transitions return the
 * current state unchanged — a stray event must never move the window into a
 * state its own flow did not reach.
 */
export function reduce(state: SessionState, event: SessionEvent): SessionState {
  switch (event.type) {
    case "selectRequested":
      // Refusing while analysing is what stops two results racing to populate
      // one window; it mirrors `Phase::accepts_selection` in Rust.
      return state.phase === "analyzing" ? state : { phase: "selecting" };
    case "selectionCancelled":
      return state.phase === "selecting" ? { phase: "cancelled" } : state;
    case "analysisStarted":
      return { phase: "analyzing", repository: event.repository };
    case "analysisSucceeded":
      return state.phase === "analyzing"
        ? { phase: "ready", payload: event.payload }
        : state;
    case "analysisFailed":
      return { phase: "failed", error: event.error };
    case "reset":
      return { phase: "idle" };
  }
}

/** The initial state. */
export const initialState: SessionState = { phase: "idle" };
