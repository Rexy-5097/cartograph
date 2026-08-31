/** Entry point. Mounts the shell; owns nothing else. */
import { StrictMode } from "react";
import { createRoot } from "react-dom/client";

import App from "./App";
import "./styles.css";

const container = document.getElementById("root");
if (!container) {
  throw new Error("index.html must provide #root");
}

createRoot(container).render(
  <StrictMode>
    <App />
  </StrictMode>,
);
