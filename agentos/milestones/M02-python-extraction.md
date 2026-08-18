# M02 — Python extraction

Target: Day 5 · Branch: `feature/m02-python-extraction` · Tag on acceptance: `cartograph-m02`

Scope: tree-sitter Python. Symbols + route declarations for FastAPI decorators,
Flask `@app.route`, Django `urlpatterns` — path pattern, HTTP method, handler
symbol, location.

Acceptance: golden tests per framework; fixtures cover decorator variants;
gates pass.

Implementation note (recorded during execution): route observations carry a
`RouteDeclarationStyle` (verb decorator / route decorator / URL-conf entry)
rather than a framework name. Flask 2.x and FastAPI share verb-decorator
syntax exactly, so a framework label is not derivable from the declaration and
was measurably wrong on real Flask code. M03 canonicalises from the path
syntax itself. Declared methods are a list (Flask declares sets) and an empty
list means the source did not state one — never defaulted.

Standard exit: PR + CI green + self-review + human acceptance + checkpoint tag + state update + STOP.
