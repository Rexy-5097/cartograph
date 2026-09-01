/**
 * The 10,000-node rendering benchmark.
 *
 * M11's acceptance criterion is "10k-node graph at 60 FPS on the dev machine
 * (measured)". This is the instrument that measures it. It is **development
 * only** — `main.tsx` mounts it behind `import.meta.env.DEV`, which Vite
 * evaluates statically, so none of this code is in a production bundle.
 *
 * # Methodology, stated before the numbers
 *
 * 1. **Fixture.** `public/scene-10k.json`, produced by the real Rust layout
 *    engine and the real `scene::compose` (see
 *    `crates/cartograph-desktop/tests/fixture_10k.rs`). Coordinates are not
 *    invented for the benchmark.
 * 2. **Construction.** `buildGraph` — the same function the window uses — is
 *    timed on its own.
 * 3. **Initialisation.** A `Sigma` instance is constructed with
 *    `SIGMA_SETTINGS`, the same settings the window uses, and timed on its own.
 * 4. **First frame.** Time from construction returning to the first
 *    `afterRender` event.
 * 5. **Sustained interaction.** The camera is moved *every frame* for the whole
 *    observation window. This matters: Sigma only redraws when something
 *    changes, so a `requestAnimationFrame` loop that left the camera alone
 *    would measure an idle browser and report a meaningless 60 FPS. Every
 *    sample here is a frame that actually drew 10,000 nodes.
 * 6. **Warm-up.** The first 30 frames are discarded — they include shader
 *    compilation and texture upload, which happen once and are not what
 *    "sustained frame rate" means.
 *
 * # What "60 FPS" is taken to mean
 *
 * A frame budget of 16.67 ms. The headline number is the **median** frame
 * interval over the observation window, because a single instantaneous reading
 * says nothing. Minimum FPS (the worst single frame) and the 95th-percentile
 * frame time are reported alongside, because a median that passes while one
 * frame in twenty takes 40 ms is a stutter a user would see.
 *
 * The observation window is deliberately long enough to include garbage
 * collection: a two-second sample can miss it entirely and flatter the result.
 *
 * # What this does not measure
 *
 * The React wrapper around the canvas. This mounts `Sigma` directly with the
 * same graph and settings `GraphView` uses, so it measures the renderer and
 * the data path but not the component's own re-render behaviour. That is
 * stated rather than hidden, and the component is deliberately thin.
 */

import { useCallback, useEffect, useRef, useState } from "react";
import Sigma from "sigma";

import { SIGMA_SETTINGS } from "./GraphView";
import { buildGraph } from "./graph";
import type { Scene } from "./session";

/** Frames discarded before sampling begins. */
const WARMUP_FRAMES = 30;
/** How long to observe, in milliseconds. Long enough to include a GC pause. */
const OBSERVATION_MS = 10_000;
/** The 60 FPS budget. */
const BUDGET_MS = 1000 / 60;

interface Measurement {
  nodes: number;
  edges: number;
  clusters: number;
  fixtureBytes: number;
  buildMs: number;
  initMs: number;
  firstFrameMs: number;
  frames: number;
  durationMs: number;
  medianFrameMs: number;
  meanFrameMs: number;
  p95FrameMs: number;
  worstFrameMs: number;
  medianFps: number;
  meanFps: number;
  minFps: number;
  framesOverBudget: number;
  memoryMb: number | null;
  environment: string;
}

function percentile(sorted: number[], fraction: number): number {
  if (sorted.length === 0) {
    return Number.NaN;
  }
  const index = Math.min(
    sorted.length - 1,
    Math.max(0, Math.ceil(fraction * sorted.length) - 1),
  );
  return sorted[index] ?? Number.NaN;
}

export default function Benchmark() {
  const container = useRef<HTMLDivElement | null>(null);
  const [status, setStatus] = useState("Idle.");
  const [result, setResult] = useState<Measurement | null>(null);
  const [running, setRunning] = useState(false);

  const run = useCallback(async () => {
    if (container.current === null || running) {
      return;
    }
    setRunning(true);
    setResult(null);

    try {
      setStatus("Loading the 10,000-node fixture…");
      const response = await fetch("/scene-10k.json");
      if (!response.ok) {
        setStatus(
          "scene-10k.json is missing. Generate it with: cargo test -p cartograph-desktop --test fixture_10k -- --ignored",
        );
        setRunning(false);
        return;
      }
      const text = await response.text();
      const fixtureBytes = new Blob([text]).size;
      const scene = JSON.parse(text) as Scene;

      setStatus("Building the Graphology graph…");
      const buildStart = performance.now();
      const built = buildGraph(scene);
      const buildMs = performance.now() - buildStart;

      if (built.diagnostics.skippedNodes > 0) {
        setStatus(
          `Fixture rejected ${built.diagnostics.skippedNodes} nodes — aborting rather than measuring a reduced workload.`,
        );
        setRunning(false);
        return;
      }

      setStatus("Constructing Sigma…");
      container.current.innerHTML = "";
      const initStart = performance.now();
      const sigma = new Sigma(built.graph, container.current, SIGMA_SETTINGS);
      const initMs = performance.now() - initStart;

      const firstFrameMs = await new Promise<number>((resolve) => {
        const started = performance.now();
        sigma.once("afterRender", () => resolve(performance.now() - started));
      });

      setStatus(`Measuring for ${OBSERVATION_MS / 1000} s of continuous motion…`);
      const camera = sigma.getCamera();
      const intervals: number[] = [];
      let frame = 0;
      let previous = performance.now();
      const started = performance.now();

      await new Promise<void>((resolve) => {
        const step = () => {
          const now = performance.now();
          const delta = now - previous;
          previous = now;
          frame += 1;

          // Discard the warm-up: shader compilation and texture upload happen
          // once and are not the sustained frame rate.
          if (frame > WARMUP_FRAMES) {
            intervals.push(delta);
          }

          // Force a real redraw every frame. Without this the renderer idles
          // and the loop measures nothing.
          const t = (now - started) / 1000;
          camera.setState({
            x: 0.5 + Math.cos(t * 0.7) * 0.18,
            y: 0.5 + Math.sin(t * 0.9) * 0.18,
            ratio: 1.25 + Math.sin(t * 0.5) * 0.6,
            angle: 0,
          });

          if (now - started < OBSERVATION_MS) {
            requestAnimationFrame(step);
          } else {
            resolve();
          }
        };
        requestAnimationFrame(step);
      });

      const durationMs = performance.now() - started;
      const sorted = [...intervals].sort((a, b) => a - b);
      const median = percentile(sorted, 0.5);
      const mean = intervals.reduce((a, b) => a + b, 0) / intervals.length;
      const worst = sorted[sorted.length - 1] ?? Number.NaN;

      // `performance.memory` is Chromium-only and absent under some settings.
      const memory = (
        performance as unknown as { memory?: { usedJSHeapSize: number } }
      ).memory;

      setResult({
        nodes: built.graph.order,
        edges: built.graph.size,
        clusters: scene.clusters.length,
        fixtureBytes,
        buildMs,
        initMs,
        firstFrameMs,
        frames: intervals.length,
        durationMs,
        medianFrameMs: median,
        meanFrameMs: mean,
        p95FrameMs: percentile(sorted, 0.95),
        worstFrameMs: worst,
        medianFps: 1000 / median,
        meanFps: 1000 / mean,
        minFps: 1000 / worst,
        framesOverBudget: intervals.filter((d) => d > BUDGET_MS).length,
        memoryMb:
          memory === undefined
            ? null
            : Math.round((memory.usedJSHeapSize / 1024 / 1024) * 10) / 10,
        environment: navigator.userAgent,
      });
      setStatus("Complete.");
      sigma.kill();
    } catch (error) {
      setStatus(`Failed: ${String(error)}`);
    } finally {
      setRunning(false);
    }
  }, [running]);

  // `?bench=auto` starts the run without a click, so the measurement can be
  // driven from a script and captured from the dev server's console output.
  // That is what lets the numbers come from the real Tauri window rather than
  // from a browser tab that merely shares its engine.
  const autoStart = useRef(false);
  useEffect(() => {
    if (autoStart.current) {
      return;
    }
    const params = new URLSearchParams(window.location.search);
    if (params.get("bench") === "auto") {
      autoStart.current = true;
      void run();
    }
  }, [run]);

  useEffect(() => {
    if (result !== null) {
      // Emitted as one line so a harness can pick it out of the log, and
      // posted to the dev server because WebView2 does not forward the console
      // to the terminal that launched the window.
      const json = JSON.stringify(result);
      console.log(`CARTOGRAPH_BENCHMARK ${json}`);
      void fetch("/__bench", { method: "POST", body: json }).catch(() => {
        // The sink is development tooling; its absence must not fail a run.
      });
    }
  }, [result]);

  return (
    <div className="app">
      <header className="bar">
        <h1>Renderer benchmark</h1>
        <button type="button" onClick={run} disabled={running}>
          {running ? "Measuring…" : "Run 10k benchmark"}
        </button>
      </header>

      <main className="panel">
        <p className="note" role="status">
          {status}
        </p>

        <div ref={container} className="graph-canvas" />

        {result !== null && (
          <>
            <h2 className="subheading">Result</h2>
            <p className={result.medianFps >= 60 ? "note" : "note"}>
              Median <strong>{result.medianFps.toFixed(1)} FPS</strong> over{" "}
              {result.frames} sampled frames ({(result.durationMs / 1000).toFixed(1)} s
              of continuous camera motion).{" "}
              {result.medianFps >= 60
                ? "Meets the 60 FPS budget."
                : "Does NOT meet the 60 FPS budget."}
            </p>
            <pre className="benchmark-json">
              {JSON.stringify(result, null, 2)}
            </pre>
          </>
        )}
      </main>
    </div>
  );
}
