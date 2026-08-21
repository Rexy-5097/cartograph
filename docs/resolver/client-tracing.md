# Client wrapper tracing

```text
ParseDagButton.tsx  ──Call──►  useDagParsing  ──Call──►  useDagParsingServiceReparseDagFile
                                                                      │
                                                                    Call
                                                                      ▼
                                                              reparseDagFile
                                                                      │
                                                    PUT /api/v2/parseDagFile/{file_token}
```

Added by the M07 remediation, decided in
[ADR-0012](../adr/ADR-0012-the-client-wrapper-is-a-node.md).

Real frontends do not write URLs at call sites. Every frontend in the M07
corpus reaches its backend through a generated or hand-written client, and only
the innermost file contains a URL. M07 measured the consequence: twelve of
1,458 `HttpCall` edges had a TypeScript client.

## What is recorded

**A request described by a configuration object** is an HTTP observation:

```ts
__request(OpenAPI, { method: 'POST', url: '/api/v2/pools' })
(options.client ?? client).get({ url: '/api/v1/items/' })
SupersetClient.get({ endpoint: '/api/v1/chart/' })
```

Only `url` and `endpoint` are read as the URL key. **`path` is not**: React
Router, Vue Router and every routing table in the corpus describe screens with
`{ path: "/orders" }`, and reading those would turn a frontend's own route
table into outbound requests — a larger false-positive class than the true
positives this finds.

The value must be a literal or template whose text begins a URL path. A method
comes from a literal `method` property, or failing that from a callee named for
a verb; `.get({url})` is a GET because that is what it says.

**The function containing the request is the edge's source.** A generated
client wraps every request in a function that components import and call, and
that function is a `Function` node in its own right. A request at module scope
still produces a `File` node.

**Calls that reach such a function become `Call` edges**, with provenance
`static-import-resolution`. Callees are resolved through
[import resolution](imports.md), never matched by name.

## What refuses

- A call whose callee resolves to nothing, to a package, or ambiguously.
- A call whose enclosing function is unknown — an edge needs both ends.
- A call that reaches no request. This is **not a general call graph**: without
  that rule Airflow alone would contribute hundreds of thousands of edges,
  burying the few that matter.
- More than four call hops. Beyond that the path stops being something a
  reader could check by hand.

## Known limit

A route registered with a named handler — `router.get('/items', handleItems)` —
is still indistinguishable from a client call with a configuration argument.
Only an inline handler marks a registration, and only for a receiver that is
not a known HTTP client, because Node's `http.get(url, callback)` is a real
request.
