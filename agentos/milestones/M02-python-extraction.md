# M02 — Python extraction

Target: Day 5 · Branch: `feature/m02-python-extraction` · Tag on acceptance: `cartograph-m02`

Scope: tree-sitter Python. Symbols + route declarations for FastAPI decorators,
Flask `@app.route`, Django `urlpatterns` — path pattern, HTTP method, handler
symbol, location.

Acceptance: golden tests per framework; fixtures cover decorator variants;
gates pass.

Standard exit: PR + CI green + self-review + human acceptance + checkpoint tag + state update + STOP.
