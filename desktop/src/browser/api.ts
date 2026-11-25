/**
 * The browser transport (M18 experiment; proposed ADR-0024).
 *
 * The desktop reaches Rust through Tauri's `invoke`; the browser reaches the
 * same Rust functions through `cartograph-serve`'s loopback API. This module is
 * the only place that knows about HTTP. Everything it returns is what
 * `cartograph-desktop` produced, parsed and nothing more: no field is
 * computed, merged or defaulted here.
 *
 * The session token arrives in the URL fragment (`#token=…`), which the
 * browser never sends to a server. It is moved to `sessionStorage` and the
 * fragment is cleared, so it does not linger in the address bar or history.
 */

import type {
  AnalysisId,
  AnalysisPayload,
  BlastResult,
  DesktopError,
  EdgeKind,
  EvidenceRecord,
  NodeKind,
} from "../session";

/** `/api/status`. */
export interface ServerStatus {
  repository: string;
  phase: "idle" | "running" | "ready" | "failed";
  analysis: AnalysisId | null;
  running: AnalysisId | null;
  error: DesktopError | null;
  experimental: boolean;
}

/** One candidate route of a request-matching decision. */
export interface RouteCandidate {
  route: string;
  file: string;
  line: number;
  handler?: string;
  confidence: number;
  methodCompatibility: string;
  pathCompatibility: string;
  reasons: string[];
}

/** One request-matching decision, refusals included. */
export interface RequestDecision {
  clientFile: string;
  clientLine: number;
  clientRoute: string;
  status: "exact" | "strong" | "ambiguous" | "no-match" | "unsupported" | "other";
  unsupportedReason?: string;
  candidates: RouteCandidate[];
}

/** A parse problem in one file. */
export interface FileDiagnostic {
  file: string;
  severity: string;
  kind: string;
  message: string;
  line?: number | null;
}

/** The pipeline's counts, as `cartograph_pipeline::pipeline::Totals`. */
export interface Totals {
  files: number;
  files_clean: number;
  files_with_diagnostics: number;
  files_failed: number;
  symbols: number;
  routes: number;
  routes_composed: number;
  http_observations: number;
  exact: number;
  strong: number;
  ambiguous: number;
  no_match: number;
  unsupported: number;
  dynamic_resolved: number;
  dynamic_partial: number;
  orm_models: number;
  orm_tables: number;
  orm_ambiguous: number;
  nodes: number;
  edges: number;
  http_call_edges: number;
  client_call_edges: number;
  orm_edges: number;
  [other: string]: number;
}

/** `/api/report`. */
export interface AnalysisReport {
  analysis: AnalysisId;
  totals: Totals;
  languages: Record<string, number>;
  decisions: RequestDecision[];
  ormAmbiguous: string[];
  diagnostics: FileDiagnostic[];
  /** Where each node is declared, as `cartograph --json` lists it. */
  nodes: { id: number; file?: string; line?: number }[];
  elapsedMs: number;
}

/** One hop of a trace. */
export interface TraceHop {
  edge: number;
  to: number;
  stop?: "chain-end" | "max-depth" | "cycle";
  next: TraceHop[];
}

/** `/api/trace`. */
export interface TraceResult {
  analysis: AnalysisId;
  from: number;
  maxDepth: number;
  stop?: "chain-end" | "max-depth" | "cycle";
  steps: TraceHop[];
}

/** A relationship of an inspected node. */
export interface Neighbour {
  edge: number;
  kind: EdgeKind;
  node: number;
  label: string;
  nodeKind: NodeKind;
}

/** `/api/node`. */
export interface NodeDetail {
  analysis: AnalysisId;
  node: number;
  label: string;
  kind: NodeKind;
  file?: string;
  line?: number;
  outgoing: Neighbour[];
  incoming: Neighbour[];
}

/** `/api/source`. */
export interface SourceExcerpt {
  analysis: AnalysisId;
  file: string;
  firstLine: number;
  highlight?: number;
  lines: string[];
  totalLines: number;
}

/** A failed request: the server's own error record, or a transport failure. */
export class ApiError extends Error {
  readonly status: number;
  readonly kind: string;
  readonly hint: string | undefined;

  constructor(status: number, kind: string, message: string, hint?: string) {
    super(message);
    this.status = status;
    this.kind = kind;
    this.hint = hint;
  }
}

const TOKEN_KEY = "cartograph-session-token";

/** Reads the token from the fragment once, then from session storage. */
export function sessionToken(location: Location = window.location): string | null {
  const match = /(?:^#|&)token=([0-9a-f]{16,128})/.exec(location.hash);
  if (match?.[1] !== undefined) {
    try {
      sessionStorage.setItem(TOKEN_KEY, match[1]);
    } catch {
      // Storage may be unavailable; the in-memory token still works.
    }
    try {
      history.replaceState(null, "", location.pathname + location.search);
    } catch {
      // Leaving the fragment is harmless; it is never sent to a server.
    }
    return match[1];
  }
  try {
    return sessionStorage.getItem(TOKEN_KEY);
  } catch {
    return null;
  }
}

/** A client bound to one session token. */
export class Api {
  private readonly token: string;

  constructor(token: string) {
    this.token = token;
  }

  private async request<T>(method: "GET" | "POST", path: string): Promise<T> {
    let response: Response;
    try {
      response = await fetch(path, {
        method,
        headers: { "X-Cartograph-Session": this.token },
        credentials: "omit",
        cache: "no-store",
      });
    } catch {
      throw new ApiError(0, "unreachable", "The Cartograph server is not reachable.");
    }
    const text = await response.text();
    let body: unknown = null;
    try {
      body = text === "" ? null : JSON.parse(text);
    } catch {
      throw new ApiError(response.status, "bad-response", "The server sent something unreadable.");
    }
    if (!response.ok) {
      const error = (body ?? {}) as { kind?: string; message?: string; hint?: string };
      throw new ApiError(
        response.status,
        error.kind ?? "error",
        error.message ?? `The server answered ${response.status}.`,
        error.hint,
      );
    }
    return body as T;
  }

  status(): Promise<ServerStatus> {
    return this.request("GET", "/api/status");
  }

  analyze(): Promise<{ started: AnalysisId }> {
    return this.request("POST", "/api/analyze");
  }

  analysis(): Promise<AnalysisPayload> {
    return this.request("GET", "/api/analysis");
  }

  report(): Promise<AnalysisReport> {
    return this.request("GET", "/api/report");
  }

  evidence(analysis: AnalysisId, edge: number): Promise<EvidenceRecord> {
    return this.request("GET", `/api/evidence?analysis=${analysis}&edge=${edge}`);
  }

  blast(analysis: AnalysisId, node: number): Promise<BlastResult> {
    return this.request("GET", `/api/blast?analysis=${analysis}&node=${node}`);
  }

  trace(analysis: AnalysisId, node: number): Promise<TraceResult> {
    return this.request("GET", `/api/trace?analysis=${analysis}&node=${node}`);
  }

  node(analysis: AnalysisId, node: number): Promise<NodeDetail> {
    return this.request("GET", `/api/node?analysis=${analysis}&node=${node}`);
  }

  source(analysis: AnalysisId, file: string, line?: number): Promise<SourceExcerpt> {
    const at = line === undefined ? "" : `&line=${line}`;
    return this.request(
      "GET",
      `/api/source?analysis=${analysis}&file=${encodeURIComponent(file)}${at}`,
    );
  }
}
