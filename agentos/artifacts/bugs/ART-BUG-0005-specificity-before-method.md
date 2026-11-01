---
id: ART-BUG-0005
title: "BUG-0005 Method evidence outranked path specificity when choosing a route"
version: 1.0
status: completed
owner: qa-reviewer
created: 2026-11-01
modified: 2026-11-01
related_adr: None
related_standard: standards/testing.md
related_checklist: QG-007
related_workflow: bug_fix.md
related_agent: qa-reviewer
---

# Bug Report: BUG-0005

## Description

`match_client` (`crates/cartograph-resolver/src/matching.rs`) ordered
candidates by their prior alone. `confidence_for` multiplies the path prior by
0.8 when a method is undeclared. A static route without methods therefore
scored 0.784, below a parameter route with a declared method at 0.90.

Found by the correctness-recovery programme (F-016), while probing the
matcher's rules.

## Steps to Reproduce

1. A Flask blueprint with `@bp.route("/orders/latest")` and
   `@bp.route("/orders/<order_id>", methods=["GET"])`.
2. A client calls `GET /orders/latest`.
3. The result is `Strong` to `read_order`. Flask serves the call with
   `latest_order`, because Werkzeug ranks static rules first.

## Diagnostics

- **Affected file:** `crates/cartograph-resolver/src/matching.rs`
  (`match_client` sort, `classify` tie).
- **Severity:** Major (precision). The edge picks the wrong handler with
  ordinary confidence.

## Fix

Candidates rank by path specificity (exact static > parameter bound >
unknown aligned > wildcard), then prior, then path. A tie needs equal
specificity and equal prior.

## Verification Test

- **Regression tests:** three in `tests/matching.rs`, section "F-016". The
  Flask shape fails on the unchanged code.
- **Full suite:** 1,199 passed, 0 failed, 32 ignored. Gates 9/9, validator
  98/100.

## Out-of-sample measurement (release builds, `7df7058` → `6c95ed5`)

Seventeen assessment repositories, with the same command and limits. Every
run exited 0. The result was predicted exactly beforehand from the candidate
lists in `7df7058`'s output, which this change only re-orders.

| Repository | `http-call` edges | Lost | Gained |
|---|---|---|---|
| langgenius/dify | 39 → 51 | 0 | 12 |
| mlflow/mlflow | 163 → 168 | 0 | 5 |
| Agenta-AI/agenta | 34 → 35 | 0 | 1 |
| the other 14, including Polar and Airflow | unchanged | 0 | 0 |

Each gained edge was a call whose exact static route had been outranked by a
non-discriminating `/{name}` parameter route, which forced `Ambiguous`.

- **Dify (12).** `tests/.../explore/test_installed_app.py` calls
  `/installed-apps`, declared by `InstalledAppsListApi`
  (`@console_ns.route("/installed-apps")`).
- **MLflow (4 of 5).** Test calls reach stubs declared in the same test file.
- **Two are wrong, the F-004 class.** Each joins production code to a route
  declared only in a test:
  - MLflow `mlflow/pyfunc/scoring_server/__init__.py` `GET /ping` → `_ping`
    in `tests/server/test_workspace_middleware.py`;
  - Agenta's SDK reference `GET /spans/` → a route in
    `api/oss/tests/pytest/unit/utils/test_heartbeat.py`.

  Both were hidden before only because the call was ambiguous. Removing them
  needs application identity (ADR-0022 draft), not a matcher rule.
- **Lost.** No edge was lost.
