/**
 * Entry point of the browser explorer (M18 experiment). Mounts the shell and
 * nothing else. Built by `vite.browser.config.ts` into `dist-browser/`, which
 * `cartograph-serve` serves; the desktop's `index.html` build is unaffected.
 */
import { StrictMode } from "react";
import { createRoot } from "react-dom/client";

import "@fontsource-variable/geist";
import "@fontsource-variable/geist-mono";

import BrowserApp from "./BrowserApp";
import "../styles.css";
import "./browser.css";
import "./report.css";

const container = document.getElementById("root");
if (!container) {
  throw new Error("browser.html must provide #root");
}

createRoot(container).render(
  <StrictMode>
    <BrowserApp />
  </StrictMode>,
);
