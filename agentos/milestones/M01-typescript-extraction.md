# M01 — TypeScript extraction

Target: Day 4 · Branch: `feature/m01-typescript-extraction` · Tag on acceptance: `cartograph-m01`

Scope: tree-sitter TypeScript/TSX grammar into `cartograph-parser`. Extract
declared symbols, imports, call sites, string and template literals, with
repository-relative locations. `fetch`/`axios.*` call-site detection (method +
URL expression captured, not yet evaluated). rayon for parallel file parsing.

Acceptance: golden extraction tests over fixture TS files; 1k-file corpus
parses without panic; imports produce `Import` edge candidates with
`static-import-resolution` provenance; gates pass.

Standard exit: PR + CI green + self-review + human acceptance + checkpoint tag + state update + STOP.
