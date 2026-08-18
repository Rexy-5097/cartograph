# Support

> **AgentOS Version:** 1.0.0 | **Type:** Private Internal Engineering Platform

---

## Where to Get Help

AgentOS is a private internal engineering template. The following channels are available for your team.

### 1. Documentation (Start Here)

Before opening an issue, check the documentation:

| Resource | Purpose |
|----------|---------|
| [AGENTOS.md](./AGENTOS.md) | Full AI initialization protocol |
| [README.md](./README.md) | Architecture overview and quick start |
| [ARCHITECTURE.md](./ARCHITECTURE.md) | Visual diagrams of every system |
| [BOOTSTRAP.md](./BOOTSTRAP.md) | Bootstrap reference |
| [TEAM_QUICKSTART.md](./TEAM_QUICKSTART.md) | 5-minute getting started guide |
| [DOCUMENTATION_INDEX.md](./DOCUMENTATION_INDEX.md) | Full documentation registry |
| [FAQ](./README.md#faq) | Common questions answered |
| [Troubleshooting](./README.md#troubleshooting) | Problem resolution table |

### 2. GitHub Issues

If the documentation doesn't resolve your issue, open a GitHub issue using the appropriate template:

- **Bug:** Use the [Bug Report template](./.github/ISSUE_TEMPLATE/bug_report.md)
- **Feature:** Use the [Feature Request template](./.github/ISSUE_TEMPLATE/feature_request.md)
- **Question:** Use the [Question template](./.github/ISSUE_TEMPLATE/question.md)

### 3. Internal Team Channels

For private team deployments, use your organization's internal communication channels (Slack, Teams, email) to reach the repository maintainer.

---

## Common Issues

| Symptom | Likely Cause | Resolution |
|---------|-------------|------------|
| Validator score < 100 | Missing files or stale references | Read warning details in validator output |
| Bootstrap fails | Missing profile YAML or Python deps | Run `python3 tools/scripts/bootstrap_project.py --self-test` |
| AI assistant doesn't load AgentOS | AGENTOS.md not at repo root | Confirm file exists: `ls AGENTOS.md` |
| Profile not found | Profile name typo | Run `make bootstrap` and select from list |
| Loop exits too early | Loop mode too aggressive | Use `--loop-mode Exhaustive` |
| Pre-commit hook fails | YAML/Markdown lint error | Fix the flagged file; re-commit |
| Dev container doesn't start | Docker not running | Start Docker Desktop first |

---

## Response Times (Private Team Use)

This is an internal engineering tool. Response time expectations depend on your team's internal norms. We recommend:

- **P0 — Production blocked:** Immediate escalation to lead engineer
- **P1 — Feature broken:** Same-day response
- **P2 — Documentation gap:** Within 3 business days
- **P3 — Enhancement request:** Next planning cycle

---

## Self-Diagnosis

Before requesting support, run the full diagnostic:

```bash
make validate       # Should show 100/100
make test           # Should show 21/21 PASS
make self-test      # Should show All 8 profiles verified — PASS
```

Include the output of all three commands in any support request.
