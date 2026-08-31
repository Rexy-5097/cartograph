# ADR-0016 — The desktop boundary: a shared pipeline crate, a testable session, a thin shell outside the workspace

**Status:** Accepted · 2026-09-01 · Implements [ADR-0001](ADR-0001-rust-core-is-the-product.md) ·
Accepted by the project owner with M11 Slice 2

## Decisions recorded on acceptance

The five decisions this ADR is accepted on, stated in one place so a reader
does not have to infer them from the discussion below:

1. **The analysis pipeline belongs in a shared Rust crate, not inside the CLI
   binary.** `cartograph-pipeline` owns discovery, parsing, caching and
   resolution, and every client calls it.
2. **The Tauri desktop shell is a *client* of that shared pipeline**, on equal
   footing with the CLI and with the MCP server that follows at M15. It
   composes nothing of its own.
3. **The desktop shell may sit outside `cargo test --workspace`** where Tauri's
   webview requirement makes that necessary, **provided a dedicated desktop CI
   job validates it.** The exemption is conditional on that job existing and
   passing, not on the shell being unimportant.
4. **Zustand is deferred**, because the session is a single state machine that
   `useReducer` holds without ceremony.
5. **shadcn/ui is deferred** until the interface actually needs the component
   and dependency overhead it brings.

Decisions 4 and 5 are deferrals, not rejections: both remain in M11's stated
stack and neither is foreclosed by anything here.

## Context

M11 adds a second client. ADR-0001 says the CLI, the desktop application and
the MCP server are *peer clients* of one core and that none of them holds
analysis logic, and `cartograph-cli`'s own crate documentation has claimed
exactly that since M09:

> One of three planned clients of the same core graph API — the desktop
> application and the MCP server are the others. The CLI holds no analysis
> logic of its own.

It was not quite true. The individual steps live in `cartograph-parser`,
`cartograph-resolver` and `cartograph-graph`, but the **sequence** that turns a
directory into a graph — discovery, parsing, caching, resolution, edge
construction — lived in `cartograph-cli`, which is a **binary** crate. A binary
cannot be depended on. So the second client had two options, and both were
wrong: duplicate the sequence, or reach into the first client.

Three questions followed, and each has consequences later milestones inherit.

## Decision

### 1. The analysis sequence moves to `cartograph-pipeline`

`discovery`, `pipeline`, `incremental` and `error` move out of
`cartograph-cli` into a new library crate, **verbatim**. The CLI depends on it
and re-exports the modules at its crate root, so its command modules still say
`crate::pipeline` and the diff stays a move rather than a rewrite.

Nothing about behaviour changed: the analyser, the M10 incremental caches, the
exit codes and the error payloads are the ones M09 and M10 accepted, and their
suites moved with them and pass at their recorded counts.

Two items became genuinely public and had to earn it. `FactCache::len` gained
`is_empty`, and `Repository::describe` gained `#[must_use]` — both were latent
API defects that a binary crate had hidden from clippy.

**`discovery::is_rooted` is now public**, and that is the point of the exercise
rather than a side effect. It is the predicate M09's Windows privacy fix
introduced: `Path::is_absolute()` is false on Windows for a drive-less rooted
path such as `\projects\secret`, and that gap is how an absolute path reached
serialised output. Every client that shows a path back to a user owes the same
redaction. A second client answering that question its own way is precisely how
the defect would return, so there is one predicate and it is shared.

**The error type keeps the name `CliError`.** It is misnamed in a shared crate,
and renaming it would have touched every command module and the JSON
conformance suite during a move whose whole value is that it changes nothing.
Recorded as a wart to fix deliberately, not paid for here.

### 2. The session model lives in `cartograph-desktop`, which does not depend on Tauri

Validating a chosen folder, running the analysis, shaping the result and
classifying a failure are ordinary Rust. Opening a window is not.

The first group lives in `cartograph-desktop`, a **workspace member with no
Tauri dependency**, so `cargo test --workspace` and QG-004 already cover it.
The second lives in `desktop/src-tauri`, which is thin enough to read in one
sitting: two commands, each a single call.

The type system carries the guarantee rather than a convention. `session::analyze`
accepts only a `ValidatedRepository`, and the only way to construct one is
`repository::validate`, so an unchecked path cannot reach the analyser.

### 3. The Tauri crate is excluded from the cargo workspace

`tauri` links a system webview — WebKitGTK on Linux. Making it a workspace
member would make `cargo check --workspace`, and therefore QG-002 through
QG-004, depend on system packages that no M00–M10 job has ever needed. A
contributor without GUI development libraries could no longer run the gates.

So it is excluded, and gets its own CI job that installs what it needs. The
cost is real and worth stating: **`cargo test --workspace` does not build the
shell**, so a mistake there is caught by the desktop CI job rather than by the
gates. That is the reason the shell is kept as small as it is — everything that
can be wrong is deliberately on the other side of the line.

### 4. The frontend takes the smallest dependency set that does the job

M11's scope names React 19, Vite, shadcn/ui and Zustand. Two of those are
**not** added in this slice, because the rule is that every dependency answers
"what concrete Cartograph problem does this solve *now*?"

| Package | Decision |
|---|---|
| React 19, `react-dom`, TypeScript, Vite | **added** — the named frontend and build tool |
| `@tauri-apps/api` | **added** — required to call a command at all |
| `@tauri-apps/plugin-dialog` | **added** — without a native folder picker the user must paste a path |
| **Zustand** | **not added** — the state is one discriminated union; `useReducer` holds it. It earns its place when there is cross-panel state to share |
| **shadcn/ui** | **not added** — it brings Tailwind and Radix to render one button and three states. Plain CSS is smaller and no worse here |

Both are deferred, not rejected, and neither is load-bearing for the renderer
slice.

### 5. There is no cancellation, and the application says so

`pipeline::run` is synchronous with no interruption point. "Cancel" therefore
means the window stops waiting and discards the result when it arrives; the
analysis runs to completion and keeps a core busy while it does.

This is safe because a session holds no state a discarded result could corrupt —
`analyze` is a pure function of the path, and a test asserts that abandoning a
result leaves the next analysis identical. Real interruption would mean
threading a cancellation token through M10's accepted pipeline, which does not
belong in a shell slice.

## Alternatives

- **Give `cartograph-cli` a `[lib]` target and depend on that.** Rejected: the
  desktop would depend on a crate named "cli", contradicting the peer-client
  model in ADR-0001 and this crate's own documentation. Cheaper, and wrong in a
  way that becomes permanent.
- **Duplicate the pipeline in the desktop.** Rejected outright: two clients
  composing the same steps differently is the drift ADR-0001 exists to prevent,
  and it would show up as two clients disagreeing about one repository.
- **Make the Tauri crate a workspace member.** Rejected: it puts a system
  webview dependency in front of every gate for every contributor.
- **Put validation and orchestration in the Tauri commands.** Rejected: it
  would place the only code that can be wrong in the only crate the gates do
  not build.
- **Extract into `cartograph-core` instead of a new crate.** Rejected: `core`
  is the type foundation and deliberately depends on nothing; the pipeline
  depends on the parser, the resolver and the graph.

## Consequences

- There is one analysis sequence, and two clients call it. A third — the MCP
  server at M15 — has somewhere to attach.
- `cargo test --workspace` covers the desktop's logic and not its shell. The
  desktop CI job covers the shell on Linux and macOS.
- Windows is the development platform and is **not** covered by the desktop CI
  job. The shell is checked locally on Windows; that is stated rather than
  implied, and no cross-platform claim is made beyond what actually ran.
- `CliError` is misnamed in its new home. Recorded, not fixed.
- The frontend can adopt Zustand and shadcn/ui later without rework; nothing
  here forecloses them.
- A `dist/` build is required before the Tauri crate compiles, because
  `generate_context!` reads it. CI builds the frontend first, and a contributor
  who skips it gets a build error rather than a stale window.
