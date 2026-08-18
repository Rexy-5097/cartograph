# Parser architecture (M01–M02)

Implemented in [`crates/cartograph-parser`](../../crates/cartograph-parser/).
Tier 1 of the two-tier analysis ([ADR-0002](../adr/ADR-0002-two-tier-analysis.md)):
it answers **"what does this file say?"** and nothing else.

## Boundary

```
source text ──► Analyzer ──► FileAnalysis (facts + diagnostics)
```

- **tree-sitter is contained.** It appears in exactly one private module
  (`src/typescript.rs`); no public signature names a tree-sitter type. The
  integration tests exercise the crate purely through its own model, so a
  grammar upgrade or runtime swap cannot break consumers.
- **The fact model is language-neutral** (`src/model.rs`). The Python
  extractor (M02) produces the same `FileAnalysis` from a different grammar:
  `src/typescript.rs` and `src/python.rs` are peers behind one
  `Analyzer::dispatch`, sharing `src/syntax.rs` for span/text handling so a
  span means the same thing in both languages. Adding a language is a new
  private module plus one dispatch arm.
- **Facts are observations, not claims.** A `CallSite` is not a resolved call;
  an `HttpCallObservation` is not an `HttpCall` edge; an import specifier is a
  string, not a resolved dependency. Only the resolver (M03–M06) turns
  observations into evidenced edges. This distinction is load-bearing for the
  whole product (RULES 007–010).

## Extracted facts

| Fact | Contents |
|---|---|
| `FileAnalysis` | repo-relative path, language, `ParseStatus`, fact lists, diagnostics |
| `Import` | specifier, default/namespace/named (+aliases), type-only, span |
| `Symbol` | kind (function/class/method/variable/interface/type-alias/enum), name, `is_async`, enclosing symbol, export status, span |
| `CallSite` | callee (identifier or property chain), `is_new`, argument count, span |
| `StringFact` | literal (escapes as written) · template (text/expression parts, expression source + span preserved for M05) · concatenation (flattened `+` chain) |
| `HttpCallObservation` | callee, method hint (never defaulted), URL as literal/template/dynamic, span |
| `RouteObservation` | declaration style, declared methods (empty = unstated), path verbatim, handler, span |
| `Diagnostic` | severity, kind, structural-metadata message, span |

Spans are 1-based lines/columns (end column exclusive), matching the
`CheckoutButton.tsx:34` rendering the product promises. Paths are validated
with the domain model's rules — absolute paths and `..` are rejected at the
API boundary, so a machine layout cannot enter persisted results.

## Supported Python syntax (M02)

Imports (`import x`, `import x as y`, `from m import a`, aliases, relative
`.`/`..`, wildcard) · functions and `async def` · classes and methods · nested
functions · module-scope variables · decorators (recorded as observations) ·
calls `foo()` / `obj.foo()` / `pkg.mod.fn()` / `ClassName()` · string literals
including triple-quoted · f-strings with substitution structure · `+`
concatenation chains · HTTP-shaped calls (`requests`/`httpx`/`session`/
`client`/`http`/`aiohttp` verbs, plus any receiver with a path-like argument) ·
route declarations (verb decorators, `@app.route` with `methods=[…]`,
`path()`/`re_path()` URL-conf entries).

### Python-specific decisions

- **No invented exports.** Python has no export statement, so symbols are
  `ExportStatus::NotApplicable` rather than `NotExported` — claiming the latter
  would assert something the source never said. A module that declares
  `__all__` is different: that is a real export list, and the names in it are
  marked `Exported`. A leading underscore stays a naming convention, visible in
  the symbol's own name, and is not reinterpreted as visibility.
- **Instantiation is a call.** `ClassName()` is recorded with `is_new = false`;
  Python has no `new`, and knowing whether a name is a class requires
  resolution.
- **Decorators are not descended into.** A decorator's arguments are captured
  by the `RouteObservation` it produces. Walking into them would additionally
  register `@app.get("/orders")` as an outbound HTTP client call, giving every
  FastAPI application phantom requests — the same reasoning that keeps import
  specifiers out of `strings`.

## Route observations, and what they are not

A `RouteObservation` says: *this file contains syntax declaring a route of this
shape.* It does not say the route is reachable, that the framework is mounted,
that the handler is the one that runs, or that anything calls it. It is never
an edge. Matching routes to calls is M04; canonicalising paths is M03.

`RouteDeclarationStyle` names the **syntax matched**, not a framework —
`VerbDecorator`, `RouteDecorator`, `UrlConfEntry`. That was a correction forced
by evidence: running the extractor over Flask's own repository produced twenty
routes labelled "FastAPI", because Flask 2.x supports `@app.get(...)` with
identical syntax — one of them `@app.get("/result/<id>")`, carrying Flask's
dialect under a FastAPI label. M03 should therefore canonicalise from the path
syntax itself (`{id}` vs `<int:id>` vs `:id` are distinguishable directly),
not from a framework label the parser cannot honestly supply.

Declared methods are a **list**, because Flask genuinely declares sets
(`methods=["GET", "POST"]`). An empty list means the source did not state a
method — it is never defaulted to GET, since Flask's implicit GET is framework
semantics, i.e. inference.

## Supported TypeScript/TSX syntax (M01)

Imports (default, namespace, named, aliased, type-only, side-effect, mixed) ·
function/generator declarations (+`async`) · classes and methods · interfaces ·
type aliases · enums · module-scope `const`/`let`/`var` · export forms
(inline, `export {…}` clauses, `export default` incl. anonymous expressions) ·
call sites `foo()` / `a.b.c()` / `new Foo()` · string literals · template
literals with substitution structure · `+` concatenation chains · HTTP-shaped
calls (`fetch`, `axios.*`, known client verbs, path-like heuristic).

## Error tolerance

tree-sitter yields a tree for any input. Error regions become structured
diagnostics (capped at 25/file, then a `Truncated` marker); extraction
continues on well-formed regions (`CompleteWithErrors`). Unreadable and
non-UTF-8 files yield `Failed` with a diagnostic. **No file can abort a
repository walk**, and diagnostic messages carry structural metadata only —
never source text (RULE 015).

## Known limitations (deliberate)

**Python (M02)**

- Route recognition requires a recognisable receiver (`app`, `router`, `api`,
  `blueprint`, `bp`, `*_app`, `*_router`) or the `.route` form; a route
  registered on an unusually named object is not observed.
- Django `path(prefix, view)` with a non-literal pattern is not recorded — the
  pattern is not observable, and recording it would be a guess.
- Class-based Django views record the callable as written
  (`LegacyView.as_view`); resolving it is later work.
- Test-client calls (`self.client.get("/x")`, `factory.post("/y")`) are
  observed as HTTP, because syntactically they are HTTP requests. This inflates
  observation counts in test-heavy repositories; distinguishing test from
  production traffic needs context M02 does not have.
- Decorator arguments other than the route path are not extracted.
- Tuple/attribute assignment targets are not recorded as symbols.
- `async` is detected on the definition; `functools.wraps`-style indirection is
  not followed.

**TypeScript (M01)**

- Arrow functions bound to `const` are recorded as `Variable`, not `Function`.
- Destructuring declarations are not recorded as symbols.
- Local (non-module-scope) variables are not recorded.
- Individual strings inside a concatenation chain live in the chain fact only.
- The grammar (tree-sitter-typescript 0.23) trips on some advanced type-level
  syntax (call-signature overloads with defaulted tuple type params,
  `typeof import(…)`); those regions become diagnostics and the rest of the
  file is still extracted — observed on real repositories, behaviour as
  designed.
- Sequential file processing; parallelism arrives when a milestone measures
  the need.

## What M01 does NOT do

No symbol resolution, no route matching, no URL evaluation (template
structure is preserved for M05, not evaluated), no ORM analysis, no graph
edges, no LSP, no Python.
