import { writeFileSync } from "node:fs";

import react from "@vitejs/plugin-react";
import { type Plugin, defineConfig } from "vite";

/**
 * A development-only sink for benchmark results.
 *
 * WebView2 does not forward `console.log` to the terminal that launched
 * `tauri dev`, so a measurement taken inside the real desktop window has no way
 * out. This accepts one POST from the benchmark page and writes it to disk,
 * which is what lets the recorded numbers come from the actual application
 * rather than from a browser tab that merely shares its rendering engine.
 *
 * `configureServer` runs only under `vite dev`; nothing here exists in a build.
 */
function benchmarkSink(): Plugin {
  return {
    name: "cartograph-benchmark-sink",
    apply: "serve",
    configureServer(server) {
      server.middlewares.use("/__bench", (request, response, next) => {
        if (request.method !== "POST") {
          next();
          return;
        }
        const chunks: Buffer[] = [];
        request.on("data", (chunk: Buffer) => chunks.push(chunk));
        request.on("end", () => {
          const body = Buffer.concat(chunks).toString("utf8");
          writeFileSync("bench-result.json", body, "utf8");
          server.config.logger.info(`benchmark result written: ${body}`);
          response.statusCode = 204;
          response.end();
        });
      });
    },
  };
}

// Tauri serves the frontend from a fixed port in development and from the
// built assets in release. `clearScreen: false` keeps Rust's compiler output
// visible, which is otherwise wiped by Vite on every reload.
export default defineConfig({
  plugins: [react(), benchmarkSink()],
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
    watch: {
      // Vite watches the project root, which includes `src-tauri/target`. On
      // Windows the running `cartograph-map.exe` is locked, so chokidar raises
      // EBUSY and takes the dev server down with it — `tauri dev` then fails
      // with "beforeDevCommand terminated with a non-zero status code", which
      // says nothing about the actual cause. Nothing under `src-tauri` is
      // served to the browser, so there is no reason to watch it at all.
      ignored: ["**/src-tauri/**"],
    },
  },
  build: {
    // Matches the Rust toolchain's target: a webview, not a legacy browser.
    target: "es2022",
    outDir: "dist",
    emptyOutDir: true,
  },
});
