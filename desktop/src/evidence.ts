/**
 * Presentation helpers for the evidence panel.
 *
 * Everything here turns data Cartograph produced into words a person reads.
 * **Nothing here decides anything about a repository** — no confidence is
 * computed, no relationship inferred, no location resolved. An unknown value
 * falls through to itself rather than being replaced by a guess, which is
 * RULE 009 applied to wording: if the interface does not recognise a kind it
 * shows the kind, it does not invent a friendlier one.
 *
 * Extracted from the component so these rules can be tested directly.
 */

import type { EvidenceLocation } from "./session";

/** Human wording for a provenance value. */
const PROVENANCE_WORDING: Record<string, string> = {
  "lsp-symbol-resolution": "LSP symbol resolution",
  "route-matcher": "Route matcher",
  "open-api-spec": "OpenAPI specification",
  "static-import-resolution": "Static import resolution",
  "constant-propagation": "Constant propagation",
  "template-evaluation": "Template evaluation",
  "orm-resolution": "ORM resolution",
  "model-inference": "Model inference",
};

/** Human wording for an edge kind. */
const KIND_WORDING: Record<string, string> = {
  import: "imports",
  call: "calls",
  inherits: "inherits from",
  implements: "implements",
  references: "references",
  "http-call": "calls over HTTP",
  "orm-access": "accesses via ORM",
  queries: "queries",
};

/** Wording for a provenance, or the raw value when it is not recognised. */
export function provenanceWording(provenance: string): string {
  return PROVENANCE_WORDING[provenance] ?? provenance;
}

/** Wording for an edge kind, or the raw value when it is not recognised. */
export function kindWording(kind: string): string {
  return KIND_WORDING[kind] ?? kind;
}

/**
 * `file:line` or `file:line:column`.
 *
 * The column is omitted when the analysis did not record one. It is never
 * defaulted to 1: a fabricated column is a fabricated source position, and the
 * panel would be pointing at a place nothing was observed.
 */
export function formatLocation(location: EvidenceLocation): string {
  const base = `${location.file}:${location.line}`;
  return location.column === null || location.column === undefined
    ? base
    : `${base}:${location.column}`;
}
