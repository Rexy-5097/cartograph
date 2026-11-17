---
id: ART-BUG-0003
title: "BUG-0003 An unknown client segment was accepted as equal to a route literal"
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

# Bug Report: BUG-0003

## Description

- **What `compare_paths` does.** In `crates/cartograph-resolver/src/matching.rs`
  it records an unknown client segment against a server literal as
  `UnknownValueAgainstLiteral` and yields `VariableAligned`. The module itself
  calls this "possible, never verified".
- **What `classify` did.** It refused only non-discriminating candidates and
  confidence ties. A lone such candidate was therefore `Strong` (0.80 × 0.8 =
  0.64 when the route declares no method), and `Strong` builds an edge.
- **The test that pinned it.**
  `an_unknown_client_value_against_a_literal_is_possible_but_never_exact`
  asserted `Strong`.

Found by the correctness-recovery programme (F-013). The 2026-10-30
large-codebase corpus held 21 accepted matches that rest on this rule:

- 16 on `908e289`: Agenta 1, InvokeAI 4, Langflow 1, MLflow 1, Label Studio 9;
- 5 more once PR #96 composed Django paths: Paperless-ngx 4, Plane 10.

## Steps to Reproduce

1. Backend route `/api/token/` with no declared method.
2. Client template `` `/api/${endpoint}/` ``, `POST`.
3. Run `match_client`. The result is `Strong`, and the call becomes an edge to
   the token view.

Paperless-ngx `e240c89`, `src/documents/tests/test_api_documents.py:3442`,
loops `f"/api/{endpoint}/"` over `correspondents`, `tags` and
`document_types`; none of these is `token`.

## Observed vs Expected Behavior

- **Observed:** an edge whenever the unknown-against-literal candidate is the
  only one the index holds.
- **Expected:** no edge. The value is unknown, and the routes the index cannot
  see are exactly the alternatives. Each of the 21, read against source,
  rests on a loop variable, a test parameter or an enum-typed field (Plane's
  `this.serviceType`).

## Diagnostics

- **Affected file:** `crates/cartograph-resolver/src/matching.rs`
  (`classify`).
- **Execution path:**

  ```text
  HttpCallObservation (template) → normalize_client_call (Segment::Dynamic)
    → match_client → assess → compare_paths → classify → Strong
    → pipeline::resolve → add_edge_for_match
  ```

- **Why tests missed it:** the only test of this shape asserted `Strong`. It
  checked "never exact", not "never an edge".
- **Severity:** Major (precision). The edges carried ordinary confidence.

## Fix

In `classify`, a best candidate carrying `UnknownValueAgainstLiteral` is
`Ambiguous`. It is kept with its reasons and produces no edge. No prior or
path rule changed; an unknown value into a declared parameter still matches.

## Verification Test

- **Regression tests:** `crates/cartograph-resolver/tests/matching.rs`,
  section "F-013". There are 10 tests:
  - literal at the start, middle, and after a composed prefix;
  - Plane's crossed alignment;
  - unknown into a parameter (still an edge);
  - unknowns around shared literals (still an edge);
  - a parameter and a literal competing (`Ambiguous`);
  - a known literal (`Exact`);
  - a different literal, length or method (no match);
  - a trailing slash.

  The pinning test now asserts `Ambiguous`.
- **Mutation:** with the new `classify` block absent, 4 of the new tests fail.
- **Full suite:** `cargo test --workspace` — 1,196 passed, 0 failed, 32
  ignored.
- **Other gates:**
  - fmt and clippy `-D warnings`: clean.
  - AgentOS validator: 98/100.
  - `run_gates.py`: 9/9.
  - Docs build and link check: clean.

## Out-of-sample measurement (release builds, `908e289` → `7df7058`)

Seventeen 2026-10-30 assessment repositories at manifest v3 SHAs, with the
same `match --json` command and limits. Compared by edge identity as a
multiset. Every run exited 0.

| Repository | `http-call` edges | Lost | Gained |
|---|---|---|---|
| HumanSignal/label-studio | 188 → 179 | 9 | 0 |
| invoke-ai/InvokeAI | 142 → 138 | 4 | 0 |
| Agenta-AI/agenta | 35 → 34 | 1 | 0 |
| langflow-ai/langflow | 2,520 → 2,519 | 1 | 0 |
| mlflow/mlflow | 164 → 163 | 1 | 0 |
| the other 12, including Polar and Airflow | unchanged | 0 | 0 |

- **Lost.** All 16 lost edges are the unknown-against-literal matches listed
  above, each read against source.
- **Label Studio.** Its 9 are the crossed alignments joining
  `GET /api/tasks/{id}/` to `TasksBulkCreateAPI`.
- **InvokeAI.** Its 4 joined `f"/api/v1/images/{route}"` to a stub named
  `other` in another test file.
- **Langflow.** Its one was right for one of the two values `route` takes.
  This is the cost of not guessing.
- **Gained.** No edge was gained.
- **With PR #96 applied.** The Paperless-ngx (4) and Plane (10) cases go too:
  see the recovery programme's `round-02/f013/`.
