# M04 — Cross-language resolver

Target: Day 8 · Branch: `feature/m04-cross-language-resolver` · Tag on acceptance: `cartograph-m04`

Scope: join TS call sites to Python handlers on canonical route form. LSP
clients (tsserver, Pyright) for exact symbol resolution. First real `HttpCall`
edges with `route-matcher` provenance and evidence. OpenAPI documents as ground
truth when present.

Acceptance: fixture full-stack repo resolves the button→handler hop with
correct evidence record; no hallucinated matches (unmatched = no edge); gates pass.

Implementation notes (recorded during execution):

- **LSP was not added.** The scope line names tsserver/Pyright, but route
  matching joins *observations*, not symbols: the client side is an HTTP call
  site and the backend side is a route declaration whose handler M02 already
  extracts syntactically. Adding a language-server runtime would have been a
  large dependency and process-management burden with nothing in M04 depending
  on it. It remains available for the milestone that genuinely needs symbol
  resolution.
- **OpenAPI confirmation is deferred.** It is named in this scope line as
  authoritative evidence, and it remains the natural next addition to this
  stage, but the route-matching core is what the acceptance criteria require.
  Shipping a partial specification reader alongside the first semantic join
  would have added risk to the milestone that most needed care.
- **Discriminating-evidence rule.** Real-repository validation showed a bare
  catch-all route matching client calls across an entire codebase (138 false
  edges from one route) and a `/{name}` route capturing every one-segment call.
  An accepted match now requires at least one shared static segment; the root
  path discriminates by equality.
- **Ambiguity produces no edge**, and neither does an unresolved client prefix
  (M05 unlocks those) or a client call to an absolute host.

Standard exit: PR + CI green + self-review + human acceptance + checkpoint tag + state update + STOP.
