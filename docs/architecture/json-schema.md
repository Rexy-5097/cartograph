# The `--json` output contract

**Schema:** [`cartograph-output-1.0.schema.json`](cartograph-output-1.0.schema.json)
· **Version:** `1.0` · **Since:** M09 / v0.1.0

`--json` is the integration boundary. Anything that consumes a Cartograph graph
without parsing terminal prose — an editor plugin, a CI check, a future desktop
client or MCP server — reads this shape. Those consumers cannot be updated in
lockstep with the analyser, so the contract is versioned and the schema file is
**executed** by `crates/cartograph-cli/tests/schema.rs` rather than merely
described. The document and the program cannot disagree without a test failing.

## Versioning

`schema_version` is present in every document.

- **Minor** (`1.0` → `1.1`) for additive changes: a new optional field, a new
  enum member. A consumer written against `1.0` keeps working.
- **Major** (`1.0` → `2.0`) for anything a consumer could break on: a removed
  field, a renamed field, a changed type or meaning.

## Shape

```jsonc
{
  "schema_version": "1.0",
  "command": "summary",            // summary | trace | parse | normalize | match
  "repository": {
    "name": "my-app",              // the repository's own directory name
    "requested": "."               // as typed; "<absolute>" for an absolute path
  },
  "languages": { "typescript": 39, "tsx": 70, "python": 47 },
  "summary": { "files": 156, "edges": 663, /* … */ "duration_ms": 5300 },
  "nodes":  [ /* … */ ],
  "edges":  [ /* … */ ],
  "diagnostics": [ /* … */ ],
  "errors": [],                    // present and empty on success
  "result": { /* trace only */ }
}
```

`errors` is **always present** so a consumer checks one field unconditionally
rather than testing for the key. `result` is **omitted** rather than emitted
empty, so "this command produces no traces" is distinguishable from "this trace
found nothing".

### A run that fails before producing a graph

still emits JSON on stdout, so a consumer never has to decide whether to parse:

```json
{
  "schema_version": "1.0",
  "command": "trace",
  "errors": [
    {
      "code": "symbol-not-found",
      "message": "no symbol named `Nope` is in the graph",
      "hint": "…"
    }
  ]
}
```

`code` is a stable slug — see [exit codes](../development/cli.md#exit-codes) for
the corresponding process status.

## Nodes

| Field | Type | Meaning |
|---|---|---|
| `id` | integer | Stable **within one run**. Referenced by `edges[].source` / `.target`. |
| `kind` | enum | `file`, `module`, `class`, `function`, `method`, `route`, `table`, … |
| `name` | string | The symbol, route, model or table name. |
| `file` | string? | Repository-relative, forward slashes. Absent when the node has no location. |
| `line` | integer? | One-based. |

Ids are **not** stable across runs or versions. Identity is
`(kind, name, file)` — see [ADR-0011](../adr/ADR-0011-node-identity.md). Two
edges naming the same handler are part of one chain only if they share a node
id, which is why ids are exposed at all: no name-based encoding can express it.

Nodes and edges are emitted in id order, so two runs over the same tree produce
byte-identical output.

## Edges

Cartograph never stores `A → B`. Every edge carries the case for believing it,
and the domain model will not construct one without all four attributes.

| Field | Type | Meaning |
|---|---|---|
| `id` | integer | Stable within one run. |
| `source`, `target` | integer | Node ids. |
| `kind` | enum | `import`, `call`, `inherits`, `implements`, `references`, `http-call`, `orm-access`, `queries`. |
| `confidence` | number 0–1 | **Uncalibrated.** See below. |
| `provenance` | enum | Which analysis produced it. |
| `evidence` | string | A structural description of the observation. |
| `file`, `line` | string, integer | Where the claim was observed. |

### `confidence` is not a probability

It is an uncalibrated prior selected by evidence class — seven discrete values,
not a learned score. Across the M08 corpus, 79% of edges sit at exactly `0.80`,
and grouping by confidence is the same partition as grouping by evidence class.
Measured ECE is 0.18, **in the direction of under-confidence**, and the three
lowest bands have zero verified observations.

Do not threshold on it as if it were calibrated. Read
[the confidence policy](../benchmarks/m08-confidence-policy.md) before using
this field for a decision.

### `evidence` never contains source text

It is a structural description — `POST /api/orders matched POST /api/orders at
app/api/orders.py:12 (create_order); exact HTTP method; every path segment
static and equal` — built from file paths, line numbers, canonical routes and
grammar facts. Source text is never placed into persisted evidence (RULE 015),
which is what makes a serialised graph safe to attach to a ticket.

### `provenance: "model-inference"` is not a language model

It denotes a statistical classifier over structural features. No language model
may propose, construct or modify graph structure — RULE 007 and
[ADR-0007](../adr/ADR-0007-no-llm-graph-construction.md). Nothing in the
supported subset currently emits it.

## Paths never name the machine

Every path in the document is repository-relative with forward slashes. There
is no absolute path anywhere, including in `repository`: an absolute path is a
fact about the laptop that ran the analysis, not about the repository, and
serialised output is meant to be shareable.

## The trace `result`

```jsonc
"result": {
  "symbol": "enqueueConnectionTest",
  "from":   { "id": 12, "kind": "function", "name": "…", "file": "…", "line": 834 },
  "steps": [
    {
      "edge": { /* a full edge, as above */ },
      "to":   { /* a full node */ },
      "next": [ /* further steps, recursively */ ],
      "stop": "chain-end",     // chain-end | max-depth | cycle — omitted if the walk continued
      "refusals": [ /* observations refused in the same file */ ]
    }
  ],
  "reaches_table": true,
  "max_depth": 10
}
```

`steps` is a **tree**, not a list: a node with two outgoing edges has two steps
at the same level. `stop` is how a consumer distinguishes "the chain genuinely
ends here" from "we stopped looking" — never infer completeness from an empty
`next`.

## Stability of the per-stage commands

`parse`, `normalize` and `match` carry the same envelope, but their bodies
predate this contract and are consumed by the frozen M07 benchmark and M08
calibration dataset. Their `totals`, `graph`, `matches` and `orm` sections are
extended but never reshaped; `benchmarks/run_benchmark.py` reads those field
names directly, and renaming one would silently invalidate an accepted
measurement.
