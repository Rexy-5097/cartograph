# VERSION POLICY

> **Layer:** Infrastructure | **Version:** 0.1.0 | **Status:** Authoritative

---

## Semantic Versioning

AgentOS Template follows [Semantic Versioning 2.0.0](https://semver.org/).

```
MAJOR.MINOR.PATCH
```

---

## Version Definitions

### MAJOR (X.0.0)
A breaking change that requires migration for existing projects using this template.

Examples:
- Renaming a directory that existing projects depend on
- Removing a core agent or workflow
- Changing the fundamental WAT architecture
- Restructuring the `context/` layer in a breaking way

**When incrementing MAJOR:**
- Write a migration guide in `CHANGELOG.md`
- Tag the git commit
- Notify all projects using this template

---

### MINOR (0.X.0)
A backward-compatible addition of new functionality.

Examples:
- Adding a new agent
- Adding a new workflow
- Adding a new standard or checklist
- Adding a new template
- Adding a new `metrics/` file
- Adding a new `integrations/` vendor directory

**When incrementing MINOR:**
- Document in `CHANGELOG.md`
- Tag the git commit

---

### PATCH (0.0.X)
A backward-compatible bug fix or documentation improvement.

Examples:
- Fixing a typo or broken cross-reference
- Improving clarity in an existing document
- Correcting an anti-pattern
- Updating a checklist item

**When incrementing PATCH:**
- Document in `CHANGELOG.md`

---

## Compatibility Expectations

| Change Type | Compatibility | Migration Required |
|------------|--------------|-------------------|
| MAJOR | Breaking | Yes — see CHANGELOG |
| MINOR | Backward compatible | No |
| PATCH | Backward compatible | No |

---

## Pre-Release Versions

During initial development (before 1.0.0), MINOR changes may be breaking.

Current status: `0.x.x` — pre-release. Treat all changes with caution.

The template reaches `1.0.0` when:
- All 8 phases are complete
- The second architecture audit passes
- The template has been validated against at least one real project

---

## Version File

The current version is always stored in [`VERSION`](./VERSION) at the repository root.

Format: plain text, one line, no prefix.

```
0.1.0
```

---

## Branching Strategy

| Branch | Purpose |
|--------|---------|
| `main` | Latest stable release |
| `dev` | Work in progress |
| `v{MAJOR}.x` | Long-term support branch for major versions |

---

*AgentOS VERSION POLICY — Governs all template versioning decisions.*
