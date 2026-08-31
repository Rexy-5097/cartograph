# The incremental analysis engine

> Milestone M10. Status: **in progress** — this document describes the model
> and records which parts are built. Nothing here claims a latency target.

The central invariant, and the only one that matters:

```
incremental_analysis(repo_after_change) == clean_analysis(repo_after_change)
```

Semantically equivalent, for every supported analysis feature. A fast stale
graph is a defect; a slow correct one is not. Where the two could diverge, the
engine refuses and rebuilds rather than guessing.

---

## 1. The dependency model, read from the code

This is not an idealised diagram. It is what `cli/src/pipeline.rs::run` and
`::resolve` actually do, as of `cartograph-m09`.

```
                       file bytes on disk
                              │
                              │  content hash  ─── the only cache key
                              ▼
        FileAnalysis = dispatch(path, language, source)      PURE, per-file
        (imports, symbols, calls, strings, http_calls,
         routes, routers, router_inclusions,
         module_aliases, diagnostics)
                              │
   ┌──────────────┬───────────┴────────┬──────────────────┬─────────────┐
   │              │                    │                  │             │
   ▼              ▼                    ▼                  ▼             ▼
ModuleIndex   ExportedConstants   OrmAnalysis      (routes of        (http_calls
::build       ::collect           ::build           every file)       of every file)
ALL files     ALL files           ALL files              │                 │
   │              │                    │                 │                 │
   │              ▼                    ▼                 │                 │
   │        scope_for_file        add_orm_edges          │                 │
   │        (per file, but             │                 │                 │
   │         reads ALL exports)        │                 │                 │
   │              │                    │                 │                 │
   ▼              ▼                    │                 ▼                 ▼
RouterIndex   resolve_url              │        normalize_route_    normalize_client_
::build       (per http_call)          │        declaration          call
ALL files         │                    │                 │                 │
   │              │                    │                 ▼                 │
   ▼              │                    │           RouteIndex::build       │
route prefix      │                    │           ALL routes             │
composition       │                    │                 │                 │
   │              │                    │                 └────────┬────────┘
   └──────────────┴────────────────────┼──────────────────────────┘
                                       │                 │
                                       │          match_client(client, RouteIndex)
                                       │                 │
                                       ▼                 ▼
                              OrmAccess/Queries    HttpCall edges
                                    edges                │
                                       └────────┬────────┘
                                                ▼
                                       ArchitectureGraph
                              (+ add_client_call_edges, ALL files)
```

### What this means

**Parse is per-file and pure.** `Analyzer::dispatch(path, language, source)`
touches no global state. `FileAnalysis` already derives `Clone`, `PartialEq`,
`Serialize` and `Deserialize`. So `FileAnalysis` is a pure function of
`(path, language, source_bytes)` and is therefore memoizable, comparable and
cacheable with no changes to the parser at all.

**Everything above parse is a whole-project join.** There is no per-file
locality above `FileAnalysis`. Five structures each consume *every* file:

| Structure | Consumes | Why it is global |
|---|---|---|
| `ModuleIndex` | imports, symbols, module_aliases | a specifier resolves against every module in the project |
| `ExportedConstants` | strings, symbols | a dynamic URL may follow an import to any file |
| `RouterIndex` | routers, router_inclusions | an `include_router` chain spans files |
| `RouteIndex` | every canonical route | a client matches against all routes |
| `OrmAnalysis` | every file | a model's base class may be declared elsewhere |

This is the honest reason M10 is hard: the expensive part is easy to make
incremental, and the coupled part is not.

### Measured, before choosing anything

Release build, this machine, warm page cache. `hash floor` is read + hash of
every file — the unavoidable cost of validating a warm cache.

| Repository | Files | Full run | Extraction | Resolve + rest | Hash floor | Reparse 1 file |
|---|---:|---:|---:|---:|---:|---:|
| full-stack-fastapi | 156 | 148 ms | 141 ms (95.1%) | 7 ms (4.9%) | 11 ms | 0.67 ms |
| zulip | 1,539 | 16,128 ms | 12,800 ms (79.4%) | 3,328 ms (20.6%) | 647 ms | 1.93 ms |

Extraction is 79–95% of the work, so the first slice caches it.

### Measured again, after building it

`warm` re-runs an unchanged tree through a warm cache, in process. Counts come
from `CacheStats`; the graphs were compared with `assert_graphs_equal` and are
equal in both cases.

| Repository | Cold | Warm | Speedup | Parsed (warm) | Reused (warm) |
|---|---:|---:|---:|---:|---:|
| full-stack-fastapi (156) | 159 ms | 35 ms | **4.5×** | 0 | 156 |
| zulip (1,539) | 16,980 ms | 5,625 ms | **3.0×** | 0 | 1,539 |

Reproduce with:

```bash
CARTOGRAPH_BENCH_REPO=/path/to/repo cargo test --release --bin cartograph -- --ignored --nocapture warm_cache
```

The zulip result is **below** the ~4.0 s / 4× this document predicted before
the slice was built. The prediction assumed a warm run costs
`hash + resolve ≈ 647 + 3,328 ms`; it actually costs 5,625 ms. The gap is
resolution and the per-file `FileAnalysis` clone, both of which the earlier
subtraction attributed too little. Recorded as measured rather than quietly
restated: the prediction was optimistic by about 40%.

The shape of the conclusion survives. On zulip a warm run now spends
essentially all of its time above the parse layer, so **resolve is the
bottleneck** and further gains require making the joins incremental — which is
the next slice, not this one.

---

## 2. Architecture options

**A. Custom demand-driven memoization at the parse boundary.**
A map from repository-relative path to `(content_hash, FileAnalysis)`. On each
run, hash every discovered file; reuse the cached analysis on a hit, reparse on
a miss; drop entries for paths that no longer exist. Then run the **existing,
unmodified** `resolve` over the resulting analyses.

Correctness argument, which is the point: `resolve` is a pure function of the
analyses slice, and the cache is only consulted when the content hash matches,
in which case a fresh parse would by construction produce an equal
`FileAnalysis`. So `incremental == clean` reduces to two checkable properties —
hash agreement implies analysis agreement, and `resolve` is deterministic —
rather than to an invalidation argument over a dependency graph. There is no
class of stale-edge bug available to it, because no derived fact is cached.

Cost: resolve is still recomputed in full.

**B. Salsa-style query architecture.** Model every derived fact as a query and
let the framework track dependencies and invalidate. Correct in principle and
the direction rust-analyzer took.

Rejected *for this slice*, on evidence rather than taste: the five joins above
each read every file, so under Salsa their dependency fingerprints would
include essentially the whole project, and almost any edit would invalidate
almost everything. Salsa would add a framework, a rewrite of the resolver into
queries, and a new persistence story, in exchange for invalidation that the
current data flow cannot exploit. Adopting it before the joins are decomposed
would buy the machinery and not the benefit. The M10 milestone definition also
records salsa as deliberately deferred.

**C. Fact-level dependency tracking with explicit invalidation.** Hand-built
dependency edges between derived facts, with transitive invalidation.

This is where the remaining 3.3 s lives, and it is the honest content of the
rest of M10. It is deliberately *not* attempted in the same slice as the cache:
it requires first decomposing `ModuleIndex`, `RouteIndex` and `OrmAnalysis`
into per-file contributions with a merge step, so that a changed file
invalidates its own contribution rather than the whole index.

**Chosen: A now, C next, B only if C proves insufficient — with an ADR and
measurements, never by reputation.**

---

## 3. Identity

**File identity** is the repository-relative path with forward slashes, exactly
as `discovery` produces it. Not the absolute path: that names the machine
(PART 12, RULE 015) and would make a cache non-portable.

**Content identity** is a hash of the file's bytes. Bytes, not the parsed
string, so an encoding change is a change. The hash covers content only — no
path, no timestamp, no machine identifier — so the same content anywhere hashes
the same.

**Cache key** is `path → (content_hash, FileAnalysis)`. The path is the map key
rather than part of the hash because `FileAnalysis` embeds its own path, so a
move must miss.

**Node identity** across runs remains graph-local (ADR-0011 deferred stable
identity to M10 and this slice does not deliver it). This is why canonical
equality below compares semantic identity and never `NodeId`.

### Hash choice

`std::hash::DefaultHasher` (SipHash-1-3), no new dependency.

This is a cache-validation hash, not a security boundary: the input is the
user's own working tree, there is no adversary supplying files to force a
collision, and a collision costs a stale parse rather than a privilege breach.
Measured cost is 647 ms for 21 MB across 1,539 files, which is already the
dominant term in a warm run, so a cryptographic hash would make the common path
slower for no correctness gain. `DefaultHasher`'s instability across Rust
releases is acceptable and in fact desirable for an in-process cache — a
toolchain change should invalidate it. **It would not be acceptable for an
on-disk cache**, and if persistence is added, this decision must be revisited
and the hash pinned; that is recorded as a limitation rather than papered over.

---

## 4. Canonical equality

`incremental == clean` needs a defined relation. Comparing `ArchitectureGraph`
values directly is wrong: `NodeId`/`EdgeId` are insertion-ordered and
graph-local, so two runs that produce identical architecture can number their
nodes differently.

A node's semantic identity is `(kind, name, file, line)`. An edge's is
`(source semantic identity, target semantic identity, kind, confidence,
provenance, evidence, file, line)`. Canonical form is the sorted multiset of
each. Two graphs are equal when both multisets are equal.

Deliberately excluded: `NodeId`, `EdgeId`, insertion order, and `created_at`
(a wall-clock timestamp, nondeterministic by construction). `elapsed` is
likewise excluded from analysis comparison. Everything else — confidence,
provenance, evidence text, source locations — **is** compared, because those
are the product's claims and a mismatch in any of them is a defect.

Multiset rather than set: a duplicate edge is a real difference.

---

## 5. Invalidation rules for this slice

| Event | Rule |
|---|---|
| content hash matches | reuse the cached `FileAnalysis` |
| content hash differs | reparse, replace the entry |
| file absent from discovery | drop the entry; its facts vanish with it |
| file newly discovered | parse, insert |
| rename | delete + create, since discovery reports paths, not filesystem rename events. The old path is absent and is dropped; the new path is new and is parsed. Semantically exact; it costs one reparse of a file whose bytes did not change, which is a deliberate trade against tracking inodes across platforms |
| anything above parse | recomputed in full, every run |

Every derived fact — imports resolution, routes, HTTP matches, dynamic URLs,
ORM relationships, diagnostics, edges — is recomputed from the analyses on
every run. That is why invariants 2, 3, 5, 6, 7 and 10 hold trivially for this
slice: there is nothing derived to go stale. The cache holds only outputs of a
pure function, keyed by that function's complete input.

Diagnostics are carried inside `FileAnalysis`, so they are cached and
invalidated with it and cannot outlive their file.

---

## 6. Failure and recovery

The cache is in-process and holds no derived state, which bounds the failure
modes sharply:

- **A cache miss is always safe** — it costs a reparse.
- **A corrupted entry cannot be silently wrong**, because an entry is only used
  when the hash of the current bytes matches; a corrupted hash misses, and a
  corrupted `FileAnalysis` under a matching hash is the one case the in-memory
  design cannot produce (nothing writes to it except a completed parse).
- **A failed parse is a `FileAnalysis` with `status: Failed` and a diagnostic**,
  not an error — this is existing M01 behaviour and it is cached like any other
  result.
- **An unreadable file** yields the same, so a permissions change is visible
  rather than silently reusing the last good parse.

Atomicity: the analyses vector is assembled in full before `resolve` runs and
the graph is built from scratch each time, so a failure during update leaves
the previous `Analysis` untouched and publishes nothing partial. There is no
window in which half-updated facts are observable.

**Persistence is not implemented in this slice.** The cache lives in one
process. Cross-process reuse needs an on-disk format, a pinned hash, a schema
version and a corruption story — and is only worth building once resolve is
incremental, since a warm start that still pays 3.3 s of resolve is not the
product goal. Recorded as a limitation, not as done.

---

## 7. What is built, and what is not

Built in this slice:

- content hashing of file bytes
- an in-process parse cache keyed by path and content hash
- canonical graph equality as a first-class test utility
- a deterministic repository-mutation harness
- differential tests: incremental vs clean, per mutation class
- instrumentation counting files hashed, reused and reparsed

Not built, and not claimed:

- incremental resolve — the joins are still whole-project
- on-disk persistence and cross-process reuse
- stable node identity across runs (ADR-0011's deferral stands)
- file watching
- any latency target

---

## Security

The cache holds `FileAnalysis` values, which are the existing fact model:
structural facts, symbol names, canonical routes and source locations. It does
not hold source text, environment values, credentials or absolute paths — the
same guarantees the serialised output already makes. Instrumentation counts
files and facts; it never logs file contents. Nothing is written to disk in
this slice, so there is no on-disk cache to audit; if persistence is added,
this section must state exactly what is stored before it ships.

---

## 8. Next slice — localizing `OrmAnalysis`

Per the incremental-design rule, this is the inspection and design; implementation
follows separately. `OrmAnalysis` is chosen first because its ambiguity rule is
explicit, which makes its non-local behaviour easy to state rather than easy to
miss.

### Inputs, outputs, dependencies

`OrmAnalysis::build(&[FileAnalysis])` does four things:

| Step | Reads | Locality |
|---|---|---|
| 1. filter to Python files | every file's `language` | whole project |
| 2. `ModuleIndex::build(files)` | imports, symbols, aliases of **all** files, TypeScript included | whole project |
| 3. `discover_models(file)` | one file's `symbols` | **per file, pure** |
| 4. merge: count names, drop duplicates into `ambiguous` | every file's contribution | whole project |
| 5. `discover_accesses(file, &merged_models, &index)` | one file's `calls`, **plus** the merged map and the module index | per file, but reads global state |

Outputs: `models: HashMap<name, OrmModel>`, `ambiguous: Vec<String>`,
`accesses: Vec<OrmAccessSite>`.

Only **step 3** is a pure per-file function. That is the contribution to cache.

### The finding that governs the design

**Model identity is a whole-project property, because ambiguity is.** A name
declared by two classes resolves to nothing, so:

- **Creating** a file that declares `Order` removes an *existing* `Order` model
  that no edited file mentions.
- **Deleting** one of two conflicting files *restores* the survivor's model —
  a deletion that adds a node.
- **Renaming** a class both un-ambiguates its old name and may collide on the new one.

So a per-file cache of `discover_models` is sound, but the **merge must be
redone whenever any contribution changes**, and step 5 must be redone whenever
the merged map changes, because an access resolves against it.

### Invalidation boundaries

| Change | Cached | Recomputed |
|---|---|---|
| a Python file's bytes change | other files' `discover_models` contributions | that file's contribution, the merge, all accesses |
| a Python file added or deleted | other contributions | the merge, all accesses |
| a TypeScript file changes | all model contributions | `ModuleIndex`, all accesses |
| nothing changes | everything | nothing |

Conservative by construction: whenever the merge output differs at all, every
access is recomputed rather than reasoned about individually. Accesses are cheap
relative to parsing, and an access that resolves against a stale model map is
exactly the silent-staleness failure this milestone exists to prevent.

### Honest bound on the benefit

This slice caches step 3 only. Steps 2, 4 and 5 stay whole-project, and
`ModuleIndex` still reads every file including TypeScript. So the expected win
is small — `discover_models` is a filter over already-parsed symbols, not a
parse. **It is worth doing for what it establishes, not for what it saves:** it
is the first derived fact to get a real dependency rule, and the ambiguity
behaviour above is the first genuine invalidation hazard in the codebase.

If measurement afterwards shows the win is negligible, the correct conclusion is
that `ModuleIndex` — read by steps 2 and 5 and by the router and dynamic-URL
paths — is the structure that actually needs localizing, and this slice will
have bought that knowledge cheaply and safely.

### Tests required before it lands

Beyond the seventeen already passing: two classes claiming one name across two
files; deleting one of them and asserting the survivor's model *reappears*;
creating a colliding declaration and asserting the existing model *disappears*;
and renaming a class into and out of a collision. Each compared against a clean
rebuild.

---

## 9. Governance change, and what replaced the byte-identical rule

M10 originally required the analyser to stay byte-identical to
`cartograph-m09`. That rule blocked the milestone once tracing proved a Stage C
result depends on recorded reads and on the repository's path set — neither
predictable from a content hash, and both produced by code inside
`cartograph-resolver`.

[ADR-0013](../adr/ADR-0013-m10-resolver-semantic-compatibility.md) replaces it:
**the implementation may change; M09's semantics may not.** The verification
step is now canonical graph equality against the clean-rebuild oracle plus the
full regression suite, rather than an empty diffstat. `cartograph-m09` itself
is untouched.

## 10. Recorded read dependencies

`ResolutionContext` (`cartograph-resolver::dependencies`) records what a
resolution actually consulted. It stores three things and nothing else:

| Field | Meaning | Why it is separate |
|---|---|---|
| `files` | repository-relative paths whose **contents** were read | a change to any of them can change the answer |
| `path_set_consulted` | whether the answer depended on **which files exist** | a file creation or deletion can change the answer with no content change anywhere |
| `complete` | whether every dependency was tracked | a result whose dependencies are not fully known must never be reused |

Ordering is a `BTreeSet`, so two runs over the same repository produce
byte-identical dependency sets and a future key cannot depend on hash iteration
order. Nothing else is stored: no source text, no absolute paths, no
timestamps, no environment values.

`resolve_python` now delegates to `resolve_python_tracked`, so there is one
implementation and tracking cannot drift from the behaviour it describes. The
reads recorded are the asking file (its symbols decide locality, its imports
decide the next hop), the path set (whenever a module specifier is matched),
and every module walked in a re-export chain.

`complete` is a one-way switch, and `absorb` propagates incompleteness. This
matters before any cache exists: the invariant that an untracked dependency
disqualifies reuse has to be built in from the start, not retrofitted.

### What this implies for a future cache key

A Stage C entry would need: the file's own content identity, the merged model
fingerprint, the content identities of every recorded read, **and** a
fingerprint of the repository's Python path set whenever
`path_set_consulted` is true. A key without the last term is unsound — proven
by fixture, not argued.

**No cache is implemented.** This slice records dependencies and nothing more.
