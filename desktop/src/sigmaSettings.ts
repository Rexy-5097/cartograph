/**
 * Sigma's settings, kept apart from the component that uses them.
 *
 * Importing `GraphView` pulls in Sigma, which touches `WebGL2RenderingContext`
 * at module load and therefore cannot be imported by a Node test at all. That
 * is precisely why `enableEdgeEvents` could sit wrong for a whole milestone
 * without a single test noticing. The settings live here so they can be
 * asserted without a GPU; `GraphView` re-exports them, so every existing
 * importer is unaffected.
 */

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
  // Without this Sigma never emits `clickEdge`. Its interaction listener tries
  // the node under the cursor, then the edge *only if this is true*, then falls
  // through to `clickStage` -- which the window reads as "clear the selection".
  // So with it off, clicking a relationship did the opposite of selecting one,
  // and M11's evidence panel could not be opened by mouse at all.
  //
  // It is not free: it adds the edges layer to Sigma's picking layers, so edges
  // are also drawn into a picking framebuffer each frame. That cost was measured
  // rather than assumed -- the M11 10k benchmark (10,000 nodes / 11,444 edges),
  // six runs on AC power at 165 Hz, gave a median of 8.00 ms and a p95 of
  // 8.50-8.70 ms, both inside the 16.67 ms budget on every run. No regression
  // against the accepted baseline was detectable at this scene size.
  enableEdgeEvents: true,
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
