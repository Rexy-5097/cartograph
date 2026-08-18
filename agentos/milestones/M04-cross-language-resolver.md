# M04 — Cross-language resolver

Target: Day 8 · Branch: `feature/m04-cross-language-resolver` · Tag on acceptance: `cartograph-m04`

Scope: join TS call sites to Python handlers on canonical route form. LSP
clients (tsserver, Pyright) for exact symbol resolution. First real `HttpCall`
edges with `route-matcher` provenance and evidence. OpenAPI documents as ground
truth when present.

Acceptance: fixture full-stack repo resolves the button→handler hop with
correct evidence record; no hallucinated matches (unmatched = no edge); gates pass.

Standard exit: PR + CI green + self-review + human acceptance + checkpoint tag + state update + STOP.
