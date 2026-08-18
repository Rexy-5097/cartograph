# Contributing

Thanks for looking. Cartograph is pre-alpha and moving through a frozen
milestone plan, so please read this before opening a pull request — it will save
you work.

## What is being built right now

See [ROADMAP.md](ROADMAP.md). Only one milestone is active at a time and the
build order is deliberate: **resolver → graph → CLI → benchmark → app → blast
→ diff → MCP → ask.** Pull requests that jump ahead of the current milestone
will be asked to wait, however good they are.

The most useful contributions today:

- **Real repositories that break the eventual resolver.** A public full-stack
  repository with an interesting HTTP or ORM pattern is worth more than a
  feature, because it becomes a benchmark case.
- **Route dialects we have not handled.** Framework route declarations that do
  not normalise cleanly.
- **Bug reports with a reproduction.**

Not useful yet: new language support, UI, cloud features, or anything on the
[non-goals list](PROJECT_RULES.md#non-goals).

## Before you write code

Read [PROJECT_RULES.md](PROJECT_RULES.md). Three rules reject more pull
requests than the rest combined:

1. **No language model constructs graph structure** (RULE 007). Not as a
   fallback, not behind a flag.
2. **Unknown stays unknown** (RULES 009, 010). If the analysis cannot resolve a
   URL segment, it is `{unknown}`. It is never a plausible guess.
3. **Every dependency must earn its place.** "We might need it later" is not a
   justification. New dependencies need a line in the pull request explaining
   the concrete problem they solve now.

## Setup

Requires Rust 1.85 or later. No Docker, no local models.

```bash
git clone https://github.com/Rexy-5097/cartograph.git
cd cartograph
make check
```

`make check` runs formatting, clippy with warnings denied, the test suite and
the AgentOS validator — the same things CI runs. `make help` lists everything.

If you are working from an external drive, read
[docs/development/external-storage.md](docs/development/external-storage.md)
first; there are filesystem caveats worth knowing about.

## Making a change

- Branch from the latest `main`: `feature/short-slug`, `fix/short-slug` or
  `adr/short-slug`.
- Conventional commits: `feat:`, `fix:`, `refactor:`, `test:`, `docs:`,
  `perf:`, `build:`, `ci:`, `chore:`.
- Commit messages explain **intent**, not mechanics. "changes", "work",
  "update" and "stuff" will be sent back.
- Atomic commits, one pull request per coherent unit of work.
- Never force-push a shared branch. Never rewrite `main`.

## Tests

Every behaviour change needs a test. Cartograph's tests are named as sentences
describing the property being protected — `an_edge_to_a_node_that_was_never_found_is_refused`
rather than `test_add_edge_error` — because a failing test should tell you what
broke without reading it.

Integration tests exercise crates from **outside**. That is not a preference:
testing `cartograph-graph` from outside is what proves `petgraph` has not leaked
into its public API.

## Architecture changes

Anything that changes a domain boundary, adds a dependency outside the frozen
stack, or alters an invariant needs an
[ADR](docs/adr/) — context, decision, alternatives, consequences, status. Do not
write an ADR for a routine implementation detail.

## Publishing standard

Never add a performance or accuracy number to documentation that has not been
measured on this codebase. Benchmark failures get published alongside successes.
This matters more than it sounds: the project's entire claim is that its output
is trustworthy.

## Code of conduct

By participating you agree to the [Code of Conduct](CODE_OF_CONDUCT.md).

## Licence

Contributions are licensed under [Apache-2.0](LICENSE).
