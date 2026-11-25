/**
 * The explorer's landing page: an architecture report a reader who has never
 * used Cartograph can follow (M18 experiment).
 *
 * Every number and sentence comes from the analysis (`plain.ts` words it).
 * The four "parts" are the same presentation grouping as the map's layers,
 * by file type and node kind, and the page says so.
 */

import type { ReactNode } from "react";

import type { AnalysisReport } from "./api";
import type { Layer } from "./model";
import {
  PARTS,
  PART_HINT,
  PART_LABEL,
  type Part,
  findings,
  mostDependedOn,
  plainKind,
  requestExample,
  systemFlows,
} from "./plain";
import type { Scene } from "../session";

interface Props {
  repository: string;
  report: AnalysisReport;
  scene: Scene;
  layerOfNode: ReadonlyMap<number, Layer>;
  isTest: (id: number) => boolean;
  onOpenMap: () => void;
  onOpenGaps: () => void;
  onImpact: (node: number) => void;
  onFollow: (node: number) => void;
  /** The full technical figures, shown under "Technical details". */
  technical: ReactNode;
}

const n = (value: number) => value.toLocaleString("en-GB");

export default function Report({
  repository,
  report,
  scene,
  layerOfNode,
  isTest,
  onOpenMap,
  onOpenGaps,
  onImpact,
  onFollow,
  technical,
}: Props) {
  const t = report.totals;
  const { parts, within, flows } = systemFlows(scene, layerOfNode);
  const found = findings(report, scene, isTest);
  const hubs = mostDependedOn(scene, 5);
  const top = hubs[0];
  const example = requestExample(scene, isTest);
  const refused = t.ambiguous + t.no_match + t.unsupported;
  const languages = Object.entries(report.languages)
    .filter(([, count]) => count > 0)
    .map(([name]) => (name === "tsx" ? "TSX" : name === "typescript" ? "TypeScript" : name === "python" ? "Python" : name));

  // Links between neighbouring parts of the diagram, each direction kept, so
  // the arrow points the way the proven links actually run.
  const flow = (a: Part, b: Part) => flows.find((f) => f.from === a && f.to === b)?.count ?? 0;
  const between = (a: Part, b: Part) => flow(a, b) + flow(b, a);
  const adjacent = new Set(PARTS.slice(0, -1).map((p, i) => `${p}|${PARTS[i + 1]}`));
  const distant = flows.filter(
    (f) => !adjacent.has(`${f.from}|${f.to}`) && !adjacent.has(`${f.to}|${f.from}`),
  );

  return (
    <article className="bx-report" aria-labelledby="bx-report-title">
      <header className="bx-report-hero">
        <p className="bx-eyebrow">Architecture report — {repository}</p>
        <h1 id="bx-report-title">
          How <span className="bx-report-repo">{repository}</span> fits together
        </h1>
        <p className="bx-report-lede">
          Cartograph read {n(t.files)} files of {languages.join(", ").replace(/, ([^,]*)$/, " and $1")} and mapped{" "}
          {n(t.edges)} connections between the code and the data it uses. Every connection points to
          the line of source code it comes from. Every number here is counted, and none of this was
          written by AI.
        </p>
        <div className="bx-report-actions">
          <button type="button" className="bx-button bx-primary" onClick={onOpenMap}>
            Explore the map
          </button>
          <button type="button" className="bx-button" onClick={onOpenGaps}>
            See what couldn’t be proven
          </button>
        </div>
      </header>

      <section className="bx-report-section" aria-labelledby="bx-r1">
        <h2 id="bx-r1"><span className="bx-section-no">01</span>The system at a glance</h2>
        <div className="bx-diagram" role="img" aria-label={diagramDescription(parts, within, between)}>
          {PARTS.map((part, i) => (
            <div className="bx-diagram-step" key={part}>
              <div className={parts[part] === 0 ? "bx-part bx-part-empty" : "bx-part"}>
                <span className="bx-part-name">{PART_LABEL[part]}</span>
                <span className="bx-part-count">{parts[part] === 0 ? "—" : n(parts[part])}</span>
                <span className="bx-part-unit">
                  {parts[part] === 0 ? "none found" : part === "database" ? "tables" : "pieces of code"}
                </span>
                <span className="bx-part-hint">{PART_HINT[part]}</span>
                {within[part] > 0 && (
                  <span className="bx-part-within">{n(within[part])} links inside</span>
                )}
              </div>
              {i < PARTS.length - 1 && (
                <Connector
                  forward={flow(part, PARTS[i + 1] as Part)}
                  back={flow(PARTS[i + 1] as Part, part)}
                />
              )}
            </div>
          ))}
        </div>
        {distant.length > 0 && (
          <p className="bx-report-note">
            Also linked directly:{" "}
            {distant
              .map((f) => `${PART_LABEL[f.from]} → ${PART_LABEL[f.to]} (${n(f.count)})`)
              .join("; ")}
            .
          </p>
        )}
        <p className="bx-report-note">
          Code is grouped by file type: TypeScript and JavaScript as screens, Python as server code.
          A line means a connection Cartograph proved; “no proven link” means it found none it could
          prove, not that none exists.
        </p>
      </section>

      <section className="bx-report-section" aria-labelledby="bx-r2">
        <h2 id="bx-r2"><span className="bx-section-no">02</span>What Cartograph found</h2>
        <ol className="bx-findings">
          {found.map((f) => (
            <li key={f.title}>
              <p>
                <strong>{f.title}</strong> {f.body}
              </p>
            </li>
          ))}
        </ol>
      </section>

      <section className="bx-report-section" aria-labelledby="bx-r3">
        <h2 id="bx-r3"><span className="bx-section-no">03</span>Questions you can ask it</h2>
        <ul className="bx-questions">
          {top !== undefined && (
            <li>
              <p className="bx-question">
                What would be affected if someone changed <code>{top.label}</code>?
              </p>
              <p className="bx-answer">
                It is the most depended-on part of this codebase: {n(top.count)} direct connections
                lead to it.
              </p>
              <button type="button" className="bx-button" onClick={() => onImpact(top.id)}>
                Show the impact
              </button>
            </li>
          )}
          {example !== null && (
            <li>
              <p className="bx-question">
                Where does a request from <code>{example.label}</code> end up?
              </p>
              <p className="bx-answer">
                Follow it step by step, from the code that sends it to the data it reaches.
              </p>
              <button type="button" className="bx-button" onClick={() => onFollow(example.id)}>
                Follow the request
              </button>
            </li>
          )}
          <li>
            <p className="bx-question">What couldn’t Cartograph prove?</p>
            <p className="bx-answer">
              {refused === 0
                ? "Nothing: every connection it saw, it could prove."
                : `${n(refused)} connections, each listed with the reason it stopped short.`}
            </p>
            <button type="button" className="bx-button" onClick={onOpenGaps}>
              See the gaps
            </button>
          </li>
        </ul>
      </section>

      {hubs.length > 0 && (
        <section className="bx-report-section" aria-labelledby="bx-r4">
          <h2 id="bx-r4"><span className="bx-section-no">04</span>Most depended-on code</h2>
          <p className="bx-report-note bx-report-note-first">
            The parts the most other code connects to. A change here reaches furthest.
          </p>
          <ol className="bx-hubs">
            {hubs.map((h) => (
              <li key={h.id}>
                <button type="button" className="bx-hub" onClick={() => onImpact(h.id)}>
                  <span className="bx-hub-name"><code>{h.label}</code></span>
                  <span className="bx-hub-kind">{plainKind(h.kind)}</span>
                  <span className="bx-hub-bar" aria-hidden="true">
                    <i style={{ width: `${Math.max(4, (h.count / (top?.count ?? 1)) * 100)}%` }} />
                  </span>
                  <span className="bx-hub-count">{n(h.count)}</span>
                </button>
              </li>
            ))}
          </ol>
        </section>
      )}

      <section className="bx-report-section bx-report-method" aria-labelledby="bx-r5">
        <h2 id="bx-r5"><span className="bx-section-no">05</span>How to trust this</h2>
        <div className="bx-method">
          <p>
            <strong>Read from the code, not guessed.</strong> Cartograph parses the source itself
            and draws a connection only from evidence in it, recording the exact file and line. Some
            links are inferred from conventions, such as how a data model is named; the proof says
            which.
          </p>
          <p>
            <strong>Gaps are listed, not hidden.</strong> When a connection can’t be proven — an
            address built at run time, two endpoints that both fit — it says so instead of drawing a
            guess.
          </p>
          <p>
            <strong>Nothing leaves this computer.</strong> The analysis runs locally; no source code is
            uploaded anywhere.
          </p>
        </div>
      </section>

      <details className="bx-report-tech">
        <summary>Technical details</summary>
        {technical}
      </details>
    </article>
  );
}

/** A connector between two neighbouring parts; the arrow follows the links. */
function Connector({ forward, back }: { forward: number; back: number }) {
  const count = forward + back;
  const direction =
    count === 0 ? "none" : forward > 0 && back > 0 ? "both" : forward > 0 ? "forward" : "back";
  return (
    <div className={`bx-connector bx-connector-${direction}`} aria-hidden="true">
      <span className="bx-connector-line" />
      <span className="bx-connector-label">
        {count === 0 ? "no proven link" : `${n(count)} ${count === 1 ? "link" : "links"}`}
      </span>
    </div>
  );
}

function diagramDescription(
  parts: Record<Part, number>,
  within: Record<Part, number>,
  between: (a: Part, b: Part) => number,
): string {
  const items = PARTS.map((p) => `${PART_LABEL[p]}: ${parts[p]} (${within[p]} links inside)`);
  const links = PARTS.slice(0, -1).map((p, i) => {
    const next = PARTS[i + 1] as Part;
    return `between ${PART_LABEL[p]} and ${PART_LABEL[next]}: ${between(p, next)} proven links`;
  });
  return `${items.join("; ")}. ${links.join("; ")}.`;
}
