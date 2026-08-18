# Security documentation

Policy: [SECURITY.md](../../SECURITY.md). Decision record:
[ADR-0005](../adr/ADR-0005-local-first-privacy.md). Rules 011–015 in
[PROJECT_RULES.md](../../PROJECT_RULES.md).

## Enforcement map (M00)

| Invariant | Mechanism today |
|---|---|
| No machine paths in graphs/artifacts | `SourceLocation` rejects absolute paths and `..` at construction |
| Env var values never stored | `NodeKind::EnvVar` has no value field |
| No secrets in logs | Logs to stderr, structured fields only; no code path logs file text |
| No secrets/machine paths committed | QG-005 scans every tracked file on every PR |
| No credential files tracked | `.gitignore` + QG-005 |

## Arrives later

| Invariant | Milestone |
|---|---|
| Tracing-layer redaction (mechanical, not by convention) | M04, with the first LSP/network code |
| OS keychain storage | M16 (first credential: opt-in AI key) |
| MCP session scoping | M15 |
| Temp file cleanup audit | M09 |
| GitHub App minimum scopes | M14 |
