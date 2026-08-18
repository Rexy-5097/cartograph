# Security

Cartograph is pointed at private source code. The properties below are
architectural invariants from v1, not features to be added later — they are what
makes it safe to run against an employer's monorepo.

## Invariants

| | |
|---|---|
| **No source leaves the machine** | Default and unconditional. There is no cloud component and no telemetry that carries code. |
| **AI is opt-in** | Per repository, off by default. Cartograph is fully useful with no API key and no network. |
| **Credentials live in the OS keychain** | Never in configuration files, never in the repository. |
| **Logs never carry secrets** | No source contents, tokens, credentials or environment variable values. Redaction happens at the tracing layer. |
| **Telemetry is opt-in** | Off by default. |
| **Minimum GitHub scopes** | The GitHub App (M14) requests the least it can function with. |
| **MCP is session-scoped** | The MCP server (M15) exposes only the repository authorised in the current session. |
| **Temporary files are cleaned on exit** | |

These correspond to RULES 011–015 in [PROJECT_RULES.md](PROJECT_RULES.md) and
are recorded in [ADR-0005](docs/adr/ADR-0005-local-first-privacy.md).

## How the invariants are enforced today

Not by convention. Where possible, by construction:

- `SourceLocation` **rejects absolute paths and `..` traversal** at
  construction, so a local filesystem layout cannot enter the graph, a log line
  or an exported artefact.
- `NodeKind::EnvVar` records an environment variable **name only**. There is no
  field for a value.
- The CLI writes logs to stderr and structured output to stdout, and no code
  path logs file contents.
- Quality gate [QG-005](agentos/gates/QG-005-security-and-secrets.md) scans
  tracked files for credential patterns and machine-specific paths on every
  pull request.

At M00 there is no network code, no credential storage and no AI integration —
so most of the surface these invariants protect does not exist yet. They are
written down now because retrofitting them later is how they get compromised.

## Reporting a vulnerability

Do not open a public issue.

Report privately through
[GitHub Security Advisories](https://github.com/Rexy-5097/cartograph/security/advisories/new).

Include: affected version or commit, reproduction steps, and impact. Expect an
acknowledgement within seven days. This is a pre-release solo project; response
times reflect that and will be stated honestly rather than promised optimistically.

## Supported versions

Pre-alpha. No released version exists, so no version receives security support
yet. This section will be replaced when v0.1.0 ships (M09).

## Scope

In scope: the Cartograph binary, the core library, the desktop application, the
MCP server, the GitHub Action, and the release supply chain.

Out of scope: the vendored AgentOS framework under `agentos/`, which is
development tooling that ships no runtime code — report those upstream at
[`Rexy-5097/raptors-way`](https://github.com/Rexy-5097/raptors-way).
