/**
 * Degraded ASK: presentation for an answer no model contributed to.
 *
 * # This file computes nothing about the repository
 *
 * Rust decided which relationships belong to an answer, in what order, and
 * what each one says (`cartograph_desktop::ask`). What arrives is a list, and
 * this file chooses wording for it. No entry is added, removed, reordered or
 * rewritten here.
 *
 * # The AI state comes from the answer, not from this file
 *
 * `ai` is carried in the payload for the same reason `calibrated` is: so the
 * interface states the condition from the data rather than from an assumption
 * that was true when it was written. When a provider lands, the panel follows
 * the answer instead of needing to be remembered.
 *
 * # Unknown values are shown, not guessed at
 *
 * `aiWording` passes an unrecognised state straight through, the way
 * `kindWording` and `provenanceWording` already do. A future `"enabled"` must
 * surface as itself rather than silently reading as "no model was consulted",
 * which is the one mistake that would misdescribe where an answer came from.
 */

import type { AskAnswer, EvidenceRecord } from "./session";

/**
 * How to describe the model's involvement.
 *
 * Deliberately plain. "Degraded" is the milestone's word for the code path,
 * not a thing to tell a reader who simply has no key configured.
 */
export function aiWording(ai: string): string {
  switch (ai) {
    case "disabled":
      return "No model was consulted — this is the derived evidence.";
    default:
      return ai;
  }
}

/** Whether the answer was produced without a model. */
export function isDegraded(answer: AskAnswer): boolean {
  return answer.ai === "disabled";
}

/**
 * The entries, in the order Rust returned them.
 *
 * An identity function on purpose: it exists so the ordering rule has one
 * place to be stated and one place to be tested. The bundle's order is
 * deterministic and semantic (`cartograph_graph::bundle`), and a surface that
 * re-sorted it would be inventing an emphasis the analysis did not have.
 */
export function orderedEntries(answer: AskAnswer): EvidenceRecord[] {
  return answer.entries;
}

/**
 * A one-line summary of what the answer rests on.
 *
 * An empty answer says so rather than rendering nothing: "no derived
 * relationships" is a finding, and a blank panel would read as a failure.
 */
export function askSummary(answer: AskAnswer): string {
  const count = answer.entries.length;
  if (count === 0) {
    return "No derived relationships lead away from this artefact.";
  }
  if (count === 1) {
    return "1 derived relationship.";
  }
  return `${count} derived relationships.`;
}
