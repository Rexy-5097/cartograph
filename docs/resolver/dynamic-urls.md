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
| `a + b`, as a constant's value or written directly as the request argument | `Concat` of the operands, each a literal or one of the forms in this table |
| a bound module constant | that constant's value, resolved transitively |
| `process.env.NAME`, `import.meta.env.NAME`, `os.environ["NAME"]` | `EnvVar` |
| an unbound identifier, `a.b` | `Unknown` |
| a call, an index, parentheses, any other operator | `Unsupported` |

Everything else is `Unsupported`. **This is not a JavaScript engine**: it does
not execute code, call functions, index objects, read the environment, touch
the network, or consult a model. An interpreter that handled "most"
expressions by approximating the rest would produce URLs that look right and
are not.

Only **module-scope** constants bind: a local's value depends on control flow
this analysis does not model. Constant chains resolve transitively through a
depth limit, so a cycle terminates as `Unknown`.

A constant binds only when it is a top-level TypeScript `const` or a Python
assignment at module level, and its value *is* one of the forms above, and
nothing else in the file is seen to bind the name:
- **another assignment, `def` or `class`**;
- **an import** of the name. In Python, an unconditional import written
  before the assignment does not count unless a module-level read that names
  the constant (another string constant's expression, a module-level
  request's URL) lies between them, or on the import's line;
- **a Python star import** written after it, or before it with such a read
  between or with the assignment conditional (a TypeScript `export * from`
  binds nothing locally).

A request in a class body written at module level (or in such a class body)
counts as a module-level read, because that body runs while the module loads.
In a Python f-string, a substitution with a format spec, a conversion or a
self-documenting `=` (`{n:03d}`, `{x!r}`, `{x=}`) is not evaluated, and an
f-string operand of a `+` chain keeps its substitutions. An implicitly
concatenated string (`"/api/a/" "sub/"`) is not bound. A lambda in a class body
or at module level counts as a load-time read. After a Python star import, an
assignment not in column 1 (under `if`, `try`, `with`, or after `;`) is not a
constant.

A Python import inside a function or class body does not refuse the constant;
it hides it for requests in the innermost body containing the import (an
import in a class body also hides it from that class's methods, which is
conservative).

A name the file declares at module level is never taken from an import. A
read that names the constant between its import and its assignment refuses
the constant for every use, including requests made after the module loads.
Bindings by a tuple assignment, a `for` target, `with … as`, `:=`, `global`
or `nonlocal` (including an import under them) and `+=` are not seen, so they do not
refuse the constant. A function-local assignment or parameter of the same
name is read as the constant (the lookup ignores scope); a function-local
import is not. The parser
records each variable's value span, with parentheses, `as` and `satisfies`
looked through, and the string must cover it exactly: `'/api' as const`
binds, while `isProd ? PROD : '/api'`, `base ?? '/api'` and
`'/api'.replace(…)` contain a string and do not.

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
