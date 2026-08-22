# The `cartograph` command line

The CLI is one of three planned clients of the same core graph API. It holds no
analysis logic: every command composes library calls, and one shared pipeline
runs the same sequence for all of them, so two commands cannot report on
differently-built graphs.

Version 0.1.0 · milestone M09.

---

## The four questions

The surface is deliberately small. Each command answers one question a
developer actually asks.

| Question | Command |
|---|---|
| Can I analyse this repository? | `cartograph .` |
| What did Cartograph find? | `cartograph .` |
| Why does Cartograph believe a relationship exists? | `cartograph trace <symbol>` |
| Can another tool consume the result? | `--json` on any of them |

`parse`, `normalize` and `match` expose the pipeline's individual stages and
are how the analyser itself is debugged and benchmarked.

---

## Commands

### `cartograph <path>`

Analyses a repository and prints a summary.

```bash
cartograph .
cartograph /path/to/repo
cartograph src/api.ts        # a single file also works
```

The summary is organised as **observed → resolved → refused**, and those words
are used precisely:

- **observed** — the source says this. A route declaration; a call site that
  looks like HTTP. A syntactic fact, not a relationship.
- **resolved** — the resolver determined this from the observations. A model
  that resolved to a table; a call that matched a route.
- **refused** — the resolver declined to claim a relationship, because the
  candidates were ambiguous, absent, or outside the supported subset.

Refusals are printed at the same weight as results. A tool that shows its
successes prominently and its refusals in small type is misrepresenting its own
reliability.

The summary never uses the word *verified*. Confidence values are the
specification's uncalibrated priors, not measured probabilities — see
[the confidence policy](../benchmarks/m08-confidence-policy.md).

### `cartograph trace <symbol>`

Follows one symbol's relationships across the stack, walking the graph outward
and reporting each hop with its confidence, provenance, evidence and location.

```bash
cartograph trace CheckoutButton
cartograph trace --path ../repo CheckoutButton
cartograph trace 'app/api/orders.py:create_order'    # qualified
cartograph trace CheckoutButton --max-depth 3
cartograph trace CheckoutButton --json
```

| Flag | Default | Meaning |
|---|---|---|
| `--path <PATH>` | `.` | Repository to analyse. |
| `--max-depth <N>` | `10` | Hops to follow before stopping. |

**Ambiguity is not resolved by choosing.** If two symbols share a name, both are
listed and the command exits `6`. Qualify the name with its file to pick one —
`file.py:name`. The file part matches whole path segments from the right, so
`orders.py` and `api/orders.py` both select `app/api/orders.py`, and `ders.py`
selects nothing.

**A fork is labelled as a fork.** A node with two outgoing edges prints
`branch 1 of 2` and `branch 2 of 2`, because printing them in sequence would
read as one chain — `A → B → C` when the graph says `A → B` and `A → C`.

**A break is marked.** Where the chain stops, the output says why: no outgoing
edge, the depth limit, or a cycle. Where the chain stops in a file that also
contains refused observations, those are listed and labelled as being in the
same file rather than attributed to the symbol.

### `cartograph parse <path>`

Extracted syntactic facts and parse diagnostics. Observations only.

### `cartograph normalize <path>`

Each route and HTTP-call observation's raw form beside its canonical form.
Client calls are not matched against routes here.

### `cartograph match <path>`

Every match decision with its candidates, confidence and evidence, including
the refusals. This command's `--json` document is the input to the M07 benchmark
and the M08 calibration dataset, so its payload shape is frozen.

### `cartograph version`

Release identity: version, specification version, milestone, the commit the
binary was built from, and its target triple. One `key value` pair per line, so
`grep` and `cut` work without a parser.

---

## Global flags

| Flag | Meaning |
|---|---|
| `--json` | Emit a JSON document instead of a human summary. Valid after the subcommand or before the path. |
| `--strict` | Exit `7` when some files could not be parsed. Off by default. |
| `-v`, `-vv` | Increase log verbosity. Logs always go to stderr. |
| `-h`, `--help` | Usage. |
| `-V`, `--version` | Version string. |

`CARTOGRAPH_LOG` overrides `-v` when set. It is deliberately not `RUST_LOG`: a
developer analysing a Rust project should be able to set `RUST_LOG` for their
own program without Cartograph's logging changing underneath them.

---

## Exit codes

Scripts branch on these. They are a contract; changing one is a breaking change.

| Code | Name | Meaning |
|---:|---|---|
| `0` | success | The analysis completed. Diagnostics may still have been reported. |
| `2` | usage | Unknown flag, missing argument, bad value. Also `cartograph` with no arguments. |
| `3` | input | The path is missing, unreadable, or holds no supported source. |
| `4` | analysis | The analyser failed. A defect or an environment problem, not user error. |
| `5` | not found | `trace` was given a symbol the graph does not contain. |
| `6` | ambiguous | `trace` was given a symbol matching more than one node. Nothing was chosen. |
| `7` | partial | Some files could not be parsed, and `--strict` asked for that to be a failure. |

`1` is deliberately unused. It is what a panicking process produces, so an
unexpected `1` is always a bug report rather than an ambiguous signal.

A partial analysis is **not** an error by default. Real repositories contain
files no parser handles; the counts are reported in the summary and in
`summary.files_failed`, and the run still exits `0`.

### What `--strict` actually keys on

`--strict` fires on `files_failed` — files that could not be read or parsed
**at all**, such as one that is not valid UTF-8. It does **not** fire on syntax
errors, because tree-sitter is error-tolerant: a file full of unbalanced braces
still yields symbols, imports and call sites, and those facts are real.

The practical consequence is worth stating plainly, because it surprises
people: a directory of four hundred syntactically broken TypeScript files
analyses successfully under `--strict` and reports four hundred diagnostics.
That is the intended distinction between a **recoverable diagnostic** and a
**fatal failure** — the first cost you some facts, the second cost you a whole
file. Both are reported; only the second changes the exit code.

---

## stdout and stderr

- **stdout** carries the command's result and nothing else.
- **stderr** carries diagnostics, progress and logs.

Under `--json`, stdout is a single JSON document — on success *and* on failure,
so a consumer never has to decide whether to parse. A failing `--json` run
emits `{"schema_version": …, "command": …, "errors": [...]}`.

In text mode a failure writes nothing to stdout, so a pipe receives nothing.

**Errors** are reported as `error[<slug>]: <sentence>`, optionally followed by
candidates and a `hint:` line. The slug is stable and machine-greppable; the
sentence is for a person. Internal detail — a parser error string, an errno —
appears only under `-v`, never in the message.

**Progress** appears on stderr only when all three hold: the analysis is at
least 500 files, stderr is a terminal, and `--json` is off. It reports every
250 files — a fixed count rather than a time interval, so two runs over the
same repository produce the same stream.

**Colour is not used at all.** Plain text is what stays readable in a pipe, a
log file and a screen reader alike.

**A broken pipe is success.** `cartograph . | head -5` closes the pipe after
five lines; that is what `head` does by design, and it exits `0`.

---

## What is not here

`watch`, `serve`, `diff`, `ask`, `mcp` and any desktop UI are later milestones.
A command that exists but does not work is a documentation defect, so nothing
is present as a stub.
