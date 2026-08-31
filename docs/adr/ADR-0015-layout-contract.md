# ADR-0015 — The layout contract: directory clusters, per-cluster relaxation, determinism without stability

**Status:** Proposed · 2026-09-01 · Implements [ADR-0001](ADR-0001-rust-core-is-the-product.md)
and [ADR-0006](ADR-0006-sigma-before-custom-renderer.md) · Requires the project
owner's acceptance

## Context

M11 says layout is computed in the Rust core and the frontend receives
`{id, x, y, cluster}`. ADR-0001 and ADR-0006 both say the same thing, and
ADR-0006 explains why it matters: *"the renderer swap is contained because the
frontend only draws `{id, x, y, cluster}`."*

What none of them say is **how**. Three questions had to be answered before a
single coordinate could be produced, and each has consequences a later
milestone will inherit:

1. what a cluster *is*;
2. what forces act on a node;
3. what "deterministic" is being promised.

Answering them inside the implementation and moving on would leave M12 and M13
to rediscover the answers from the code.

## Decision

### 1. A cluster is a directory

A node with a source location belongs to the cluster named by its
repository-relative parent directory. A node **without** one — a database
table, an external service, an environment variable — belongs to a cluster
named for its kind, `<Table>` and so on.

Directory structure is information the analyser already proves and the reader
already recognises. It requires no new analysis, cannot disagree with the
graph, and is deterministic by construction.

Community detection — Leiden, Louvain — was rejected **for now**, not
permanently. It would answer a different and better question ("what is
actually coupled?") rather than ("what did the authors file together?"), but it
is a research-grade component whose output is unstable under small graph
changes, and M11's acceptance criterion is that MAP answers *"what is this
system?"* — which directory structure already addresses. Revisit it when there
is a renderer and a reader to judge the difference.

Unlocated nodes are bucketed by kind rather than swept into one bin because
they are real parts of the system: a database is a thing a reader looks for.

### 2. Forces act within a cluster, not between them

Each cluster is relaxed independently with Fruchterman–Reingold inside a frame
whose area equals its population, so every cluster is drawn at the same
density rather than at the same size. Cluster centres are placed on a
phyllotaxis spiral, spaced relative to the largest cluster so a big package
cannot grow into its neighbour.

**Cross-cluster edges pull on nothing.** That is a real loss of information —
a frontend calling a backend does not draw those two closer — and it is
accepted for two reasons. Cost is the sum of squared cluster sizes rather than
the square of the node count, which is what makes 10k nodes tractable at all;
and coarse structure is already carried by cluster placement. The alternative,
a global force layout, needs Barnes–Hut or a grid approximation before it is
affordable at the target size, and that is a substantial component to build
before anything can be seen on screen.

Measured consequence: 2,000 nodes across 200 directories lay out in 10.6 ms;
the same 2,000 in one directory take 758 ms. The distribution of a repository
matters more than its size, and that is a property of this decision.

### 3. Determinism is promised; stability is not

**The same graph and the same parameters produce the same coordinates.** The
implementation earns this rather than hoping for it: ordering is by semantic
identity `(kind, name, file:line)` and never by `NodeId`; every map is a
`BTreeMap`; edge pairs are de-duplicated into a sorted set so that neither
insertion order nor a repeated observation can change a position; the
iteration count is fixed rather than convergence-tested; and initial positions
come from an FNV-1a hash written out in the module rather than from a random
number generator or `DefaultHasher`, whose output may change between Rust
releases.

Seeding from semantic identity has a further consequence worth stating: two
*independent analyses* of the same tree lay out identically, even though their
`NodeId`s differ. **This is not stable cross-run node identity.** Nothing here
assigns a durable handle and nothing correlates a node across two analyses;
the layout simply declines to depend on the numbering. ADR-0014's requirement
therefore remains where it was, and M11 does not trip it.

**Determinism is not stability under change.** Adding one node moves every node
in its cluster, and adding a cluster re-indexes the ones after it. Preserving a
reader's mental map across two *different* graphs is
`layout(prev_graph, new_graph, prev_positions)` — M13's problem, and one that
does additionally require the cross-run correspondence ADR-0014 defers.

### 4. The payload carries layout data only

`LayoutNode` is `{id, x, y, cluster}` and nothing else — no name, kind,
confidence, provenance, evidence or location. A `Cluster` is `{id, label}`.
A test asserts the serialised key set, so a field added later fails the build
rather than quietly becoming a channel for analysis data in the client
(RULE 002).

## Alternatives

- **Global force layout with Barnes–Hut** — rejected for this slice: a
  substantial component built before anything is visible, and cluster placement
  already carries the coarse structure. The measured concentration curve says
  what it would buy.
- **Community detection for clusters** — deferred rather than rejected; see
  above.
- **A grid or circular packing instead of relaxation** — rejected: deterministic
  and fast, but it shows no structure at all, and M11 is judged on whether the
  drawing answers a question.
- **Emitting a cluster id with no label** — rejected: the client would have to
  derive a name from Cartograph's data model, putting analysis in the frontend.
- **Positions as `f32`** — rejected: `f64` costs nothing at this scale and
  avoids a second rounding story on top of the one the JSON boundary already
  has.

## Consequences

- MAP can be built against a stable payload, and the renderer stays swappable.
- Layout cost is governed by cluster size. A repository that keeps everything in
  one directory is the pathological case, and it is measured rather than
  assumed.
- Cross-stack proximity is not expressed in the drawing. If MAP fails to answer
  *"what is this system?"* for that reason, this is the decision to revisit
  first.
- 10k nodes take roughly one second to lay out on the development machine. That
  is a per-analysis pause, not a per-frame cost, and it is **not** a claim about
  M11's 60 FPS criterion, which concerns an application that does not exist yet.
