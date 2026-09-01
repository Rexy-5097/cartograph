/**
 * Entry point. Mounts the shell; owns nothing else.
 *
 * In a development build only, `?bench` mounts the renderer benchmark instead.
 * `import.meta.env.DEV` is replaced by a literal at build time and the dead
 * branch is eliminated, so neither the benchmark nor its fixture loader exists
 * in a production bundle. It is reachable in the real desktop window — the one
 * `npm run tauri dev` opens — which is what makes the measurement a measurement
 * of this application rather than of a browser tab.
 */
import {
  type ComponentType,
  type LazyExoticComponent,
  StrictMode,
  Suspense,
  lazy,
} from "react";
import { createRoot } from "react-dom/client";

import App from "./App";
import "./styles.css";

const container = document.getElementById("root");
if (!container) {
  throw new Error("index.html must provide #root");
}

// The `lazy` call itself sits inside the `import.meta.env.DEV` branch, not
// merely the decision to render it. Vite replaces that expression with a
// literal `false` in a production build, so the whole block — including the
// dynamic `import()` — is eliminated and no benchmark chunk is emitted.
// Guarding only the JSX would leave the import reachable and Rollup would
// still emit the chunk, which is exactly the mistake this comment records.
let Benchmark: LazyExoticComponent<ComponentType> | null = null;
if (import.meta.env.DEV) {
  Benchmark = lazy(async () => import("./Benchmark"));
}

const wantsBenchmark =
  Benchmark !== null &&
  new URLSearchParams(window.location.search).has("bench");

createRoot(container).render(
  <StrictMode>
    {wantsBenchmark && Benchmark !== null ? (
      <Suspense fallback={<p className="note">Loading the benchmark…</p>}>
        <Benchmark />
      </Suspense>
    ) : (
      <App />
    )}
  </StrictMode>,
);
