---
id: ART-BUG-0004
title: "BUG-0004 A client-named receiver with a bare-word argument was observed as a request"
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

# Bug Report: BUG-0004

## Description

`observe_http` (`crates/cartograph-parser/src/python.rs`) accepted any
verb-named method whose receiver's last name is in `HTTP_CLIENT_OBJECTS`
(`requests`, `httpx`, `session`, `client`, `http`, `aiohttp`), without looking
at the argument. Session-dictionary lookups therefore became HTTP observations:

- Open WebUI `f7ac7ee`, `backend/open_webui/socket/main.py:255`:
  `session.get('chat_ids')`. `session` is `SESSION_POOL.get(sid)`, and the file
  imports no HTTP library.
- Odoo: `request.session.get('pre_uid')`.
- Sentry: `self.client.session.get("invite_token")`.

Found by the correctness-recovery programme (F-014a) in validation batch 1.

## Steps to Reproduce

Parse `chat_ids = session.get('chat_ids')`. The result is an
`HttpCallObservation` with URL `chat_ids`, which then meets every
single-parameter route as an ambiguous candidate.

## Observed vs Expected Behavior

- **Observed:** 205 such observations across the corpus and the validation
  repositories.
- **Expected:** a literal with no `/` is a key, or a path relative to a base
  URL this analysis cannot see. Either way, the request path is not `/key`.

## Diagnostics

- **Affected file:** `crates/cartograph-parser/src/python.rs` (`observe_http`).
- **Why tests missed it:** `http_calls.py` covered client-named receivers with
  paths and unknown receivers with non-paths, but not a client-named receiver
  with a non-path literal.
- **Cost measured before choosing a rule:**
  - Of 25 sampled observations, 23 were dictionary lookups and 2 were
    relative requests.
  - One produced an edge, and it was wrong: `airflowctl`'s
    `client.get("jobs")` was joined to the edge3 provider's `GET /jobs`.
  - The alternative rule, "any receiver, file imports no HTTP library", would
    remove 114 accepted edges, so it was rejected.

## Fix

A client-named receiver with a string literal containing no `/` is not
observed. Variables, f-strings, paths and URLs still are.

Not fixed: any other receiver with a literal starting with `/`
(`stream.get('/Subtype')`). It needs receiver evidence, which ADR-0022's
client bindings would give.

## Verification Test

- **Regression test:** `crates/cartograph-parser/tests/python_extraction.rs`,
  `a_client_named_receiver_with_a_bare_word_literal_is_not_a_request`. It has
  4 negatives and 6 positives, and fails on the unchanged code.
- **Full suite:** `cargo test --workspace` — 1,187 passed, 0 failed, 32
  ignored.
- **Other gates:**
  - fmt and clippy `-D warnings`: clean.
  - Validator: 98/100.
  - `run_gates.py`: 9/9.
  - Docs: clean.

## Out-of-sample measurement (release builds, `908e289` → `3dc67cc`)

Seventeen assessment repositories, plus Open WebUI and Redash, with the same
`match --json` command and limits. Every run exited 0.

| Repository | Observed client calls | `http-call` edges | Lost |
|---|---|---|---|
| odoo/odoo | 253 → 174 | 0 → 0 | 0 |
| getsentry/sentry | 12,670 → 12,626 | 17 → 17 | 0 |
| open-webui/open-webui | 710 → 687 (ambiguous 25 → 2) | 4 → 4 | 0 |
| Agenta-AI/agenta | 1,489 → 1,476 | 35 → 34 | 1 |
| apache/airflow | 2,464 → 2,453 | 175 → 174 | 1 |
| makeplane/plane | 497 → 484 | unchanged | 0 |
| 13 others | −1 to −7 each, or unchanged | unchanged | 0 |

- **Both lost edges were wrong.**
  - Airflow: `airflowctl`'s `client.get("jobs")` → the edge3 provider's
    `GET /jobs`.
  - Agenta: `api/oss/tests/legacy/conftest.py:858` `client.post("secrets")`
    → a `POST /secrets/` stub in an unrelated test file,
    `test_secret_validation_never_echoes.py`.
- **A cost.** Agenta's call is a genuine httpx request relative to a base URL
  this analysis cannot see. It is no longer observed at all, which is the
  cost of this rule for base-URL-relative requests.
- **Gained.** No edge was gained.
- **Left.** Open WebUI's 2 remaining ambiguous observations are F-014b
  (`stream.get('/Subtype')`).
