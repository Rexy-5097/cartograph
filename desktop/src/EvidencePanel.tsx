/**
 * The evidence record for one selected edge, and for a whole answer.
 *
 * M11 calls this the defining interaction: an edge is a *claim*, and a claim
 * the reader cannot interrogate is just a line on a picture. Everything shown
 * here was produced by the Rust analysis and is rendered as received — this
 * component computes nothing about the repository.
 *
 * # Confidence is not presented as a probability
 *
 * The record carries `calibrated`, which is `false` and has been since M08
 * measured it (ECE 0.18, and the low bands have no verified observations at
 * all). So the value is shown as what it is — an uncalibrated prior selected
 * by evidence class — and never as "78% likely". RULE 009 and the M08 finding
 * both point the same way: presenting an uncalibrated prior as a likelihood
 * would be the product lying about what it knows.
 *
 * The caveat is driven by the `calibrated` flag rather than hard-coded, so if
 * calibration ever lands the interface follows the data instead of needing to
 * be remembered.
 *
 * # One panel, two questions
 *
 * M16 asks a wider question — "explain this artefact" — but the answer is made
 * of the same claims. So this panel gained a second mode rather than a second
 * panel: two surfaces rendering one claim in two ways is how two versions of
 * the truth end up on one screen. `EvidenceBody` is the single rendering of a
 * claim, used once for a selected edge and once per entry in an answer.
 *
 * With no model configured, an answer is the derived evidence and nothing
 * else — and the panel says so **from `answer.ai`** rather than from an
 * assumption baked in here, the same treatment `calibrated` gets and for the
 * same reason.
 */

import { useEffect, useRef } from "react";

import { aiWording, askSummary, orderedEntries } from "./ask";
import {
  formatLocation,
  kindWording,
  provenanceWording,
} from "./evidence";
import type { AskAnswer, EvidenceRecord } from "./session";

type Props =
  | { record: EvidenceRecord; answer?: undefined; onClose: () => void }
  | { answer: AskAnswer; record?: undefined; onClose: () => void };

/**
 * One claim, rendered.
 *
 * The single place a relationship becomes text. Both modes go through here, so
 * a change to how a claim reads cannot apply to one of them and not the other.
 */
function EvidenceBody({ record }: { record: EvidenceRecord }) {
  const relationship = kindWording(record.kind);
  const provenance = provenanceWording(record.provenance);
  const location = formatLocation(record.location);

  return (
    <div className="evidence-entry">
      <p className="evidence-claim">
        <strong>{record.source.label}</strong>{" "}
        <span className="evidence-relationship">{relationship}</span>{" "}
        <strong>{record.target.label}</strong>
      </p>

      <dl className="evidence-fields">
        <dt>From</dt>
        <dd>
          {record.source.label} <span className="evidence-kind">{record.source.kind}</span>
        </dd>

        <dt>To</dt>
        <dd>
          {record.target.label} <span className="evidence-kind">{record.target.kind}</span>
        </dd>

        <dt>Relationship</dt>
        <dd>{record.kind}</dd>

        <dt>Established by</dt>
        <dd>
          {provenance}
          <span className="evidence-kind">
            {record.deterministic ? "derived" : "inferred"}
          </span>
        </dd>

        <dt>Confidence</dt>
        <dd>
          {record.confidence.toFixed(3)}
          {!record.calibrated && (
            <span className="evidence-caveat">
              uncalibrated prior — not a probability
            </span>
          )}
        </dd>

        <dt>Observed at</dt>
        <dd>
          <code className="evidence-location">{location}</code>
        </dd>
      </dl>

      <h3 className="evidence-subheading">Why Cartograph believes this</h3>
      <blockquote className="evidence-text">{record.evidence}</blockquote>

      {!record.calibrated && (
        <p className="evidence-footnote">
          Confidence is a prior selected by evidence class, not a measured
          likelihood. It must not be thresholded as though it were one.
        </p>
      )}
    </div>
  );
}

export default function EvidencePanel(props: Props) {
  const { record, answer, onClose } = props;
  const closeButton = useRef<HTMLButtonElement | null>(null);

  // Focus moves into the panel when it opens, so a keyboard user is not left
  // behind on the canvas with no way to reach what just appeared. Keyed on
  // whichever handle this mode is about.
  const focusHandle = answer === undefined ? record.edge : answer.target;
  const focusAnalysis = answer === undefined ? record.analysis : answer.analysis;
  useEffect(() => {
    closeButton.current?.focus();
  }, [focusHandle, focusAnalysis]);

  // Escape closes, which is what every dialog-like surface does and what a
  // person will try first.
  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        onClose();
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [onClose]);

  return (
    <aside
      className="evidence"
      role="region"
      aria-labelledby="evidence-heading"
      // Not `role="dialog"`: the map stays interactive behind it, and claiming
      // a modal role for a non-modal surface misleads assistive technology.
    >
      <header className="evidence-bar">
        <h2 id="evidence-heading">
          {answer === undefined ? "Evidence" : "Explanation"}
        </h2>
        <button
          type="button"
          ref={closeButton}
          className="evidence-close"
          onClick={onClose}
          aria-label="Close the evidence panel"
        >
          Close
        </button>
      </header>

      {answer === undefined ? (
        <EvidenceBody record={record} />
      ) : (
        <>
          <p className="evidence-ai" role="status">
            {aiWording(answer.ai)}
          </p>
          <p className="evidence-footnote">{askSummary(answer)}</p>

          {orderedEntries(answer).map((entry) => (
            <EvidenceBody key={entry.edge} record={entry} />
          ))}
        </>
      )}
    </aside>
  );
}
