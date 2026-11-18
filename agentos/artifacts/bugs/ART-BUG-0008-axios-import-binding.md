---
id: ART-BUG-0008
title: "BUG-0008 A request through axios imported under another name, with an expression-headed URL, was never observed"
version: 1.0
status: in-review
owner: qa-reviewer
created: 2026-11-18
modified: 2026-11-18
related_adr: None
related_standard: standards/testing.md
related_checklist: QG-007
related_workflow: bug_fix.md
related_agent: qa-reviewer
---

# Bug Report: BUG-0008

## Description

`typescript.rs` observed `<obj>.<verb>(url)` as a request only when:
- `<obj>` was literally `axios`, `client`, `http` or `api` (by name, case-sensitive); or
- `looks_like_path(url)` held, which needs a string or a template whose **first part is text** starting with `/`, `http://` or `https://`.

A call through axios imported under any other name, with a URL that starts with an interpolation, failed both rules and was silently dropped. The result was no observation at all, not even a refusal.

## Steps to Reproduce

```ts
import Axios from 'axios';
import config from './config';
export async function about() {
    const { backendAPI } = config;
    await Axios.get(`${backendAPI}/server/about`);   // not observed
}
```

`cartograph parse <dir> --json` reports `http_calls: []`. The same call with `` `/api/server/about` `` is observed.

## Observed vs Expected Behavior

| | Observed | Expected |
|---|---|---|
| `Axios.get(\`${backendAPI}/x\`)` with `import Axios from 'axios'` | no observation | an observation, refused downstream for its unresolved prefix |

## Diagnostics

Found on `cvat-ai/cvat@b976874` in an external-validation round of the correctness-recovery programme. CVAT had never been run through Cartograph before.

- `cvat-core/src/server-proxy.ts` makes 121 `Axios.<verb>(…)` calls. The file parses completely (518 call sites recorded) with `http_calls: 0`.
- The whole repository had 1 observation in `cvat-core` and 1 in `cvat-ui`.
- The receiver rule decides by name, but `Axios` is a *binding* to axios's default export. ECMAScript import semantics make the local name irrelevant.

## Fix

`bound_to_http_client(name)`: true when a non-type import binds `name` to the default export, the namespace, or a named `default` of a module in `HTTP_CLIENT_MODULES` (`["axios"]`). `known_client` now also holds for such a receiver. Nothing else changes: the existing name list and the path rule stay as they were.

## Verification Test

`crates/cartograph-parser/tests/typescript_extraction.rs`:
- **`a_receiver_bound_to_an_axios_import_makes_requests_whatever_its_name`:** default, namespace and `{ default as X }` imports with expression-headed URLs. Fails without the fix (0 of 3), passes with it.
- **`a_lookalike_receiver_without_an_axios_binding_is_not_a_client`:** `Map.get`, a type-only import, and an `Axios` from `./axios-like` produce no observation.

## Out-of-sample measurement (release builds, `76a4c07` → fix)

| Repository | Client observations | Edges |
|---|---|---|
| CVAT `b976874` | 198 → 320 (+122, all in `cvat-core`) | 0 → 0 |
| Plane, Label Studio, Paperless-ngx | — | unchanged (HTTP-call edges 253, 400, 5) |

- The 122 new observations are exactly the 122 calls an independent scan of `cvat-core`/`cvat-ui` finds (a frozen ground truth built without Cartograph code).
- All are refused `ClientPrefixUnresolved`, because `backendAPI` (`config.backendAPI = '/api'`) is not resolved. Resolving it is separate work, so no edge forms and none is guessed.
- Ambiguous calls (a `task`/`job` resource segment) and a call to an endpoint no backend serves are observed and not joined.
