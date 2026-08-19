# Dynamic URL resolution (M05)

Implemented in [`cartograph-resolver`](../../crates/cartograph-resolver/)
(`symbolic.rs`, `evaluator.rs`, `dynamic.rs`).

M04 refused any client URL it could not read literally. M05 resolves as much of
one as the source actually determines — and refuses the rest.

```
project files ─► exported constants ─► per-file Scope
                                            │
HTTP call URL ─────────────────────────────┴─► SymbolicValue
                                                       │
                                    partial reconstruction
                                                       │
                                         UrlObservation ─► M03 ─► M04 matcher
```

## The symbolic value model

| Variant | Meaning |
|---|---|
| `Literal` | fully determined |
| `Unknown { name }` | a value this analysis cannot determine |
| `EnvVar { name }` | an environment read — **never performed** |
| `Unsupported { source }` | a form this analysis does not interpret |
| `Concat { parts }` | an ordered sequence |

`Unknown` and `Unsupported` are deliberately distinct: a value not determined
versus a form not interpreted. The difference decides whether a gap is worth
closing.

**There is no way to flatten an unresolved value into text.** `as_literal()`
returns `Option`, so a caller wanting a string must confront the case where
there isn't one. Unresolved values render as `{unknown}`, `{env:NAME}` and
`{unsupported}` — markers chosen to be impossible to mistake for a path
segment. A test asserts none can render as `undefined`, `null`,
`[object Object]` or an empty string, which is the failure mode that would put
a plausible-looking URL into the graph.

## Supported expression grammar

| Form | Result |
|---|---|
| `"literal"` | `Literal` |
| `` `text ${expr} text` `` / f-string | `Concat` of parts |
| `a + b` | `Concat` of both sides |
| a bound module constant | that constant's value, resolved transitively |
| `process.env.NAME`, `import.meta.env.NAME`, `os.environ["NAME"]` | `EnvVar` |
| an unbound identifier, `a.b` | `Unknown` |
| a call, an index, an operator | `Unsupported` |

Everything else is `Unsupported`. **This is not a JavaScript engine**: it does
not execute code, call functions, index objects, read the environment, touch
the network, or consult a model. An interpreter that handled "most"
expressions by approximating the rest would produce URLs that look right and
are not.

Only **module-scope** constants bind: a local's value depends on control flow
this analysis does not model. Constant chains resolve transitively through a
depth limit, so a cycle terminates as `Unknown`.

## Cross-file constants

Only **exported** constants are visible to importers — resolving a private one
would assert what the importing module cannot see. Import resolution is a
restricted relative-path join: no `node_modules` walk, no `tsconfig` aliases.
An unresolved specifier contributes no bindings, so its constants stay unknown.

## Environment variables

`process.env.API_URL` becomes `{env:API_URL}`. **The variable is never read.**
Substituting the analysing machine's environment would leak configuration into
the graph, make results depend on where the analysis ran, and assert a
deployment fact the source does not contain. A test uses `PATH` — set on every
machine — to prove no value appears.

## Integration with the M04 matcher

A resolved URL becomes an ordinary `UrlObservation` and flows through M03
canonicalisation and the M04 matcher **unchanged**. Every M04 rule therefore
applies to dynamic URLs automatically: static segments compare exactly,
ambiguity produces no edge, and a route with no discriminating static segment
is never accepted.

That is the guard against this milestone's obvious failure mode — resolving
more URLs producing more matches, including wrong ones. A dynamic URL earns an
edge under exactly the same evidence a literal one would. A test pins that a
resolved URL still cannot be swallowed by a bare catch-all.

A URL whose **prefix** is unresolved is still refused, because its segment
count is unknown and no alignment is safe.

## Confidence and evidence

M05 adds no new confidence tier. A resolved URL is matched on its merits, so it
receives the M04 prior its evidence earns — 0.98 when fully resolved and
exactly matching, 0.80 when a position stays undetermined. These remain
uncalibrated priors until M08.

Evidence records what resolved and what did not, naming the constant and the
reason for each gap — never a value that was not determined.

## Known limitations

- **Runtime-configured SDK clients are not resolvable.** Generated clients that
  take a base URL from `import.meta.env.VITE_API_URL ?? ""` at construction
  time have no statically determined prefix. This is the dominant real-world
  pattern in the corpora tested, and refusing it is correct: the value is
  genuinely unknown.
- Object and config-field bases (`config.baseUrl`) are `Unknown`; resolving
  them needs object tracking.
- No `tsconfig` path aliases, no `node_modules` resolution, no index-file
  resolution beyond the common extensions.
- Local variables and function parameters are never bound.
- Conditional expressions (`a ?? b`, `a ? b : c`) are `Unsupported` rather than
  explored as alternatives.
