/**
 * The Sigma surface.
 *
 * Sigma is a *renderer*: it receives a Graphology graph whose coordinates came
 * from Rust and draws it. It computes no layout, and this component does not
 * either — there is no `forceAtlas2`, no `random` layout, no coordinate
 * assignment anywhere in this file. That is what ADR-0006 means by keeping the
 * renderer swap contained.
 *
 * # Why the graph is built outside React's render
 *
 * Building a 10,000-node Graphology graph and constructing a Sigma instance
 * are both expensive, and neither may happen on a re-render. So:
 *
 * - the graph is memoised on the `scene` object identity, so a state change
 *   that does not replace the scene rebuilds nothing;
 * - Sigma is created once per container and *reused*: a new scene calls
 *   `graph.clear()` and repopulates rather than tearing the renderer down,
 *   because constructing a WebGL context per analysis is both slow and a way
 *   to leak contexts until the driver refuses to give out more.
 *
 * The Sigma instance is kept in a ref rather than in state: it is not data the
 * view renders from, and putting it in state would re-render the tree every
 * time it changed.
 */

import { useEffect, useMemo, useRef, useState } from "react";
import Sigma from "sigma";

import { buildGraph, type BuildDiagnostics } from "./graph";
import type { Scene } from "./session";

interface Props {
  scene: Scene;
  /** Told what could not be drawn, so the surrounding page can say so. */
  onDiagnostics?: (diagnostics: BuildDiagnostics) => void;
}

/**
 * Sigma settings. Presentation only; none of these touch graph semantics.
 *
 * Exported so the benchmark measures the same renderer configuration the
 * window uses. A benchmark with different settings would measure a Sigma
 * nobody ships.
 */
export const SIGMA_SETTINGS = {
  renderEdgeLabels: false,
  defaultEdgeType: "line",
  // Labels are the expensive part of a large scene: drawing ten thousand of
  // them costs far more than drawing ten thousand nodes. The threshold hides
  // them until zoomed in, which is also what makes the map readable.
  labelRenderedSizeThreshold: 6,
  labelDensity: 0.6,
  labelGridCellSize: 80,
  labelColor: { color: "#c8ccd4" },
  labelFont: "ui-sans-serif, system-ui, sans-serif",
  labelSize: 11,
  zIndex: true,
  minCameraRatio: 0.02,
  maxCameraRatio: 40,
} as const;

export default function GraphView({ scene, onDiagnostics }: Props) {
  const container = useRef<HTMLDivElement | null>(null);
  const sigma = useRef<Sigma | null>(null);
  const [hovered, setHovered] = useState<string | null>(null);

  // Rebuilt only when the scene object itself changes.
  const built = useMemo(() => buildGraph(scene), [scene]);

  useEffect(() => {
    onDiagnostics?.(built.diagnostics);
  }, [built, onDiagnostics]);

  useEffect(() => {
    const element = container.current;
    if (element === null) {
      return undefined;
    }

    if (sigma.current === null) {
      // A renderer that fails to initialise — no WebGL context, for instance —
      // must not take the window down with it. The caller still shows the
      // summary; the canvas area reports itself unavailable.
      try {
        sigma.current = new Sigma(built.graph, element, SIGMA_SETTINGS);
      } catch {
        return undefined;
      }
      const instance = sigma.current;
      instance.on("enterNode", ({ node }) => setHovered(node));
      instance.on("leaveNode", () => setHovered(null));
    } else {
      // Reuse: swap the data, keep the WebGL context.
      sigma.current.setGraph(built.graph);
      sigma.current.getCamera().animatedReset();
    }

    return undefined;
  }, [built]);

  // Tear down only when the component actually goes away.
  useEffect(
    () => () => {
      sigma.current?.kill();
      sigma.current = null;
    },
    [],
  );

  const label =
    hovered === null
      ? null
      : (built.graph.getNodeAttribute(hovered, "label") as string | undefined);
  const kind =
    hovered === null
      ? null
      : (built.graph.getNodeAttribute(hovered, "kind") as string | undefined);

  return (
    <div className="graph-surface">
      <div
        ref={container}
        className="graph-canvas"
        // The canvas itself is not semantically accessible — see the note in
        // App.tsx. This label is what a screen reader announces on reaching it,
        // and it points at the textual summary that is accessible.
        role="img"
        aria-label={`Architecture map: ${built.graph.order} nodes, ${built.graph.size} relationships. Drag to pan, scroll to zoom. A textual summary follows.`}
      />
      {hovered !== null && label !== undefined && (
        <div className="graph-hover" role="status">
          <strong>{label}</strong>
          {kind !== undefined && <span className="graph-hover-kind">{kind}</span>}
        </div>
      )}
    </div>
  );
}
