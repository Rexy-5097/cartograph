# M15 — MCP server

Target: Week 21 · Branch: `feature/m15-mcp-server` · Tag on acceptance: `cartograph-m15`

Scope: rmcp-based MCP server exposing graph queries (map/trace/blast/diff) to
Claude Code and Cursor. Session-scoped repository authorization only.

Acceptance: MCP client can query the graph of an authorized repo and nothing
else; privacy gate extended to MCP; gates pass.

Standard exit: PR + CI green + self-review + human acceptance + checkpoint tag + state update + STOP.
