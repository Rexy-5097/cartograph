# M11 — Desktop app — Tauri, Sigma.js, MAP

Target: Week 11 · Branch: `feature/m11-desktop-map` · Tag on acceptance: `cartograph-m11`

Scope: Tauri v2 shell; React 19 + Vite + shadcn/ui + Zustand; Graphology +
Sigma.js v3; layout computed in Rust core (frontend receives {id,x,y,cluster}).
Edge click opens the evidence record — the defining interaction.

Acceptance: 10k-node graph at 60 FPS on the dev machine (measured); MAP answers
"what is this system?" on benchmark repos; gates pass.

Standard exit: PR + CI green + self-review + human acceptance + checkpoint tag + state update + STOP.
