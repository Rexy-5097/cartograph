/**
 * The evidence record for one selected edge.
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
 */

import { useEffect, useRef } from "react";

import {
  formatLocation,
  kindWording,
  provenanceWording,
} from "./evidence";
import type { EvidenceRecord } from "./session";

interface Props {
  record: EvidenceRecord;
  onClose: () => void;
}

export default function EvidencePanel({ record, onClose }: Props) {
  const closeButton = useRef<HTMLButtonElement | null>(null);

  // Focus moves into the panel when it opens, so a keyboard user is not left
  // behind on the canvas with no way to reach what just appeared.
  useEffect(() => {
    closeButton.current?.focus();
  }, [record.edge, record.analysis]);

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

  const relationship = kindWording(record.kind);
  const provenance = provenanceWording(record.provenance);
  const location = formatLocation(record.location);

  return (
    <aside
      className="evidence"
      role="region"
      aria-labelledby="evidence-heading"
      // Not `role="dialog"`: the map stays interactive behind it, and claiming
      // a modal role for a non-modal surface misleads assistive technology.
    >
      <header className="evidence-bar">
        <h2 id="evidence-heading">Evidence</h2>
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
    </aside>
  );
}
