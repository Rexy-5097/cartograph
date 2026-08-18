# Parser architecture (M01)

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
  extractor (M02) produces the same `FileAnalysis` from a different grammar.
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
| `Diagnostic` | severity, kind, structural-metadata message, span |

Spans are 1-based lines/columns (end column exclusive), matching the
`CheckoutButton.tsx:34` rendering the product promises. Paths are validated
with the domain model's rules — absolute paths and `..` are rejected at the
API boundary, so a machine layout cannot enter persisted results.

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

## Known limitations (deliberate, M01)

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
