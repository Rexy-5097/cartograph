---
id: ART-BUG-0009
title: "BUG-0009 Django models without db_table never got a table, and models with a model-class base were not models"
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

# Bug Report: BUG-0009

## Description

`orm.rs` had two gaps in its handling of Django.

1. **No default table.** A Django model without `Meta.db_table` always resolved to `TableName::Unresolved` ("Django's default needs the app label"). Django's default is `<app_label>_<modelname>`. The app label is in the app's `AppConfig`, in the package's `apps.py`. That file is already parsed, and its class-level `label` and `name` assignments are recorded, but the resolver never read it.
2. **Model recognition stopped at `models.Model`.** A class was a Django model only when one of its bases was literally `models.Model` (or an imported `Model`). A class inheriting an abstract `TimestampedModel` from the same file, or `AbstractUser`, was not a model at all, so it had no table and no ORM-access edges.

## Steps to Reproduce

```python
# engine/apps.py
from django.apps import AppConfig
class EngineConfig(AppConfig):
    name = "engine"

# engine/models.py
from django.db import models
class TimestampedModel(models.Model):
    created = models.DateTimeField()
    class Meta:
        abstract = True
class Project(TimestampedModel):   # not recognised as a model
    name = models.CharField(max_length=10)
class Label(models.Model):         # table Unresolved, expected engine_label
    name = models.CharField(max_length=10)
```

## Observed vs Expected Behavior

| | Observed | Expected |
|---|---|---|
| `Label`, `apps.py` names `engine` | Unresolved | `engine_label` |
| `Project(TimestampedModel)` | not a model | a model, table `engine_project` |
| `TimestampedModel` (`abstract = True`) | a model, Unresolved | a model with no table of its own |
| `User(AbstractUser)` with `db_table = "auth_user"` | not a model | a model, table `auth_user` |

## Diagnostics

Found on `cvat-ai/cvat@b976874` in an external-validation round of the correctness-recovery programme. CVAT had never been run through Cartograph before.

- **Tables.** None of CVAT's 44 concrete tables resolved; every one had the reason "Django's default needs the app label".
- **Models.** 18 of the 44 concrete models were not detected, including `Task`, `Job` and `Project`, because they inherit an abstract model in the same file. `trace Task` returned `symbol-not-found`.
- **A false lead.** A first correction derived a table for every model. It gave 7 abstract models tables they do not have, so abstract and proxy models now refuse.

## Fix

- **`derive_django_default_table`** runs for a Django model whose table is still Unresolved and which declares no `db_table`.
  - It refuses when `Meta` declares `abstract` or `proxy`. The parser records that the attribute is assigned but not its value, so presence alone refuses.
  - It refuses when a `db_table` is declared but is not a literal.
  - Otherwise it reads the app label through `django_app_label`.
- **`django_app_label`** reads the label from the `apps.py` in the model's package (or in the parent of a `models/` package).
  - It uses the `label` its `AppConfig` subclass states, else the last component of its `name`. That component must be the package's own directory.
  - It refuses on a missing file, no `AppConfig`, a non-literal value, a `name` for another package, or two configurations that disagree.
- **`discover_models`** adds a fixpoint over same-file classes.
  - A class whose base is a Django model class in the same file is a model.
  - So is a class inheriting `AbstractUser` or `AbstractBaseUser` imported from `django.contrib.auth.models`.
  - Cross-file model bases are not covered: they stay as before.

An explicit `db_table` still wins over everything, and a name is still never derived from the class name alone.

## Verification Test

`crates/cartograph-resolver/tests/orm.rs`, after the `BUG-0009` marker. The five positive tests fail without the fix; the two controls pass either way.

- `a_django_default_table_is_derived_from_the_label_its_apps_py_states`
- `an_explicit_app_label_wins_over_the_package_name`
- `models_inheriting_an_abstract_base_in_the_same_file_are_models`
- `an_abstract_or_proxy_model_gets_no_table_of_its_own`
- `a_custom_user_model_is_a_django_model_and_keeps_its_declared_table`
- `a_same_named_base_not_imported_from_django_auth_is_not_a_model` (control)
- `the_default_table_is_refused_when_apps_py_does_not_describe_this_package` (control: no `apps.py`, a `name` for another app, a non-literal label)

## Out-of-sample measurement (release builds, `76a4c07` → fix)

**CVAT `b976874`:**
- tables 0 → 44 of 44, all correct against a frozen ground truth built without Cartograph code;
- no table for any abstract model;
- models recognised 33 → 51;
- ORM edges 37 → 188; 25 of 25 sampled new edges correct.

**Repositories exposed in earlier rounds:**

| Repository | Edges | ORM-access | `queries` | Tables |
|---|---|---|---|---|
| Plane | 253 → 383 | +129 | +1 | 0 → 1 |
| Label Studio | 727 → 880 | +131 | +22 | 5 → 27 |
| Paperless-ngx | 490 → 1,590 | +1,082 | +18 | 0 → 18 |

- No edge was lost, and HTTP-call edges are unchanged.
- All 41 new table edges were checked against each package's `apps.py` and model source; 24 of 24 sampled access edges were correct.
