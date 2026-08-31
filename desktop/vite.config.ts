import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

// Tauri serves the frontend from a fixed port in development and from the
// built assets in release. `clearScreen: false` keeps Rust's compiler output
// visible, which is otherwise wiped by Vite on every reload.
export default defineConfig({
  plugins: [react()],
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
  },
  build: {
    // Matches the Rust toolchain's target: a webview, not a legacy browser.
    target: "es2022",
    outDir: "dist",
    emptyOutDir: true,
  },
});
