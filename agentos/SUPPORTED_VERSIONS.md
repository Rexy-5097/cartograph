# Supported Versions

AgentOS follows [Semantic Versioning](https://semver.org).

## Currently Supported

| Version | Supported | Notes |
|---------|-----------|-------|
| `1.0.x` | ✅ Active | Current stable release |
| `0.12.x` | ⚠️ Security only | RC series — upgrade recommended |
| `< 0.12` | ❌ Unsupported | End of life |

## Support Policy

- **Major releases (1.x.0):** Full support — bug fixes, security patches, and improvements
- **Minor releases (1.x.y):** Security and critical bug fixes only
- **Pre-release (0.x.x):** Best-effort support — upgrade to 1.0.0 strongly recommended

## Upgrade Guidance

See [COMPATIBILITY.md](./COMPATIBILITY.md) for upgrade paths and backward compatibility guarantees.

## Framework Stability

AgentOS v1.0.0 is declared **architecture-frozen**. The following guarantees apply:

| Component | Stability |
|-----------|-----------|
| `agents/` contracts | Stable — breaking changes require major version bump |
| `workflows/` SOPs | Stable |
| `standards/` definitions | Stable |
| `checklists/` gates | Stable |
| `runtime/harness/` | Stable |
| `runtime/loop/` | Stable |
| `runtime/kernel/` | Stable |
| `tools/scripts/` CLI flags | Stable — new flags added without removal |
| `profiles/` schema | Stable |
| `.agentos/` config schema | Stable |

> **Note:** New features will be introduced in v1.1.0 under a new roadmap without breaking v1.0.0 interfaces.
