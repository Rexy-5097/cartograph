/**
 * The browser explorer's build (M18 experiment; proposed ADR-0024).
 *
 * Separate from `vite.config.ts` so the desktop build — the one Tauri ships —
 * is untouched: different entry (`browser.html`), different output
 * (`dist-browser/`). Everything else is shared: the same React, the same
 * renderer components, the same TypeScript settings.
 */
import react from "@vitejs/plugin-react";
import { defineConfig } from "vite";

export default defineConfig({
  plugins: [react()],
  clearScreen: false,
  build: {
    target: "es2022",
    outDir: "dist-browser",
    emptyOutDir: true,
    rollupOptions: {
      input: "browser.html",
    },
  },
});
