# Security Policy

## Supported Versions

See [SUPPORTED_VERSIONS.md](./SUPPORTED_VERSIONS.md) for which versions of AgentOS receive security updates.

## Reporting a Vulnerability

AgentOS is typically deployed as a private internal engineering template. However, if you discover a security issue — particularly in the validation scripts, bootstrap tooling, or runtime kernel — please follow responsible disclosure.

### How to Report

1. **Do not** open a public GitHub issue for security vulnerabilities.
2. Email the maintainer directly or use the repository's private security advisory feature.
3. Include:
   - Description of the vulnerability
   - Steps to reproduce
   - Potential impact
   - Suggested fix (if known)

### What to Expect

- **Acknowledgement:** Within 48 hours
- **Initial assessment:** Within 5 business days
- **Fix timeline:** Depends on severity — critical issues within 7 days

## Security Design Principles

AgentOS follows these security principles by design:

- **No network calls in core framework** — All runtime scripts operate locally
- **No credentials in repository** — `PROJECT_CONFIG.yaml` contains no secrets
- **No execution of untrusted code** — Validation scripts are read-only analyzers
- **Vendor isolation** — AI provider credentials stay in `integrations/` only
- **Policy-driven access** — All routing decisions are governed by YAML policies

## Scope

Security issues in these areas are in scope:

| Area | Risk Level |
|------|-----------|
| `tools/scripts/` execution | High — runs Python with filesystem access |
| `runtime/` kernel modules | Medium — state machine and scheduler logic |
| `.agentos/config.yml` | Medium — routing configuration |
| `integrations/` adapters | High — AI provider credentials may be present |
| `validation/runner/` | Low — read-only analysis |

## Contact

For security concerns, contact the project maintainer through your organization's internal channels.
