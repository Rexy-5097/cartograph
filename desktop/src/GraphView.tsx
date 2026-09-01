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

import { applyBlastHighlight, clearBlastHighlight } from "./blast";
import { buildGraph, type BuildDiagnostics } from "./graph";
import type { BlastResult, Scene } from "./session";

interface Props {
  scene: Scene;
  /** Told what could not be drawn, so the surrounding page can say so. */
  onDiagnostics?: (diagnostics: BuildDiagnostics) => void;
  /**
   * An edge was clicked. The number is the Rust `EdgeId`.
   *
   * Selection state deliberately lives above this component: keeping it here
   * would put it inside the memo that owns the graph, and every selection
   * would risk rebuilding ten thousand nodes.
   */
  onSelectEdge?: (edge: number) => void;
  /** Clicking the background clears whatever was selected. */
  onClearSelection?: () => void;
  /** A node was clicked. The number is the Rust `NodeId`. */
  onSelectNode?: (node: number) => void;
  /**
   * The blast radius to paint, or `null` for none.
   *
   * A *derived view* of the same scene, not a different scene: it is applied
   * as attributes on the graph already loaded, so it never reaches the memo
   * that would rebuild ten thousand nodes.
   */
  blast?: BlastResult | null;
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

export default function GraphView({
  scene,
  onDiagnostics,
  onSelectEdge,
  onClearSelection,
  onSelectNode,
  blast = null,
}: Props) {
  const container = useRef<HTMLDivElement | null>(null);
  const sigma = useRef<Sigma | null>(null);
  const [hovered, setHovered] = useState<string | null>(null);

  // Rebuilt only when the scene object itself changes. Selecting an edge does
  // not touch `scene`, so it cannot reach this memo — which is the property
  // that keeps selection off the 10,000-node rebuild path.
  const built = useMemo(() => buildGraph(scene), [scene]);

  // Sigma handlers are registered once, against a ref, so a changed callback
  // does not force the renderer to be rebuilt or the listeners re-bound.
  const handlers = useRef({ onSelectEdge, onClearSelection, onSelectNode });
  handlers.current = { onSelectEdge, onClearSelection, onSelectNode };

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
      instance.on("clickEdge", ({ edge }) => {
        // Graphology keys edges `e<EdgeId>`; recover the Rust id.
        const id = Number.parseInt(edge.replace(/^e/, ""), 10);
        if (Number.isFinite(id)) {
          handlers.current.onSelectEdge?.(id);
        }
      });
      instance.on("clickStage", () => handlers.current.onClearSelection?.());
      instance.on("clickNode", ({ node }) => {
        const id = Number.parseInt(node, 10);
        if (Number.isFinite(id)) {
          handlers.current.onSelectNode?.(id);
        }
      });
    } else {
      // Reuse: swap the data, keep the WebGL context.
      sigma.current.setGraph(built.graph);
      sigma.current.getCamera().animatedReset();
    }

    return undefined;
  }, [built]);

  // Paint or clear the blast radius on the graph already loaded.
  //
  // Keyed on `built` as well as `blast` so a replaced scene starts unpainted:
  // a highlight computed for the previous graph must not survive into a new
  // one, and the ids would silently still resolve if it did.
  useEffect(() => {
    if (blast === null) {
      clearBlastHighlight(built.graph);
    } else {
      applyBlastHighlight(built.graph, blast);
    }
    sigma.current?.refresh();
  }, [built, blast]);

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
