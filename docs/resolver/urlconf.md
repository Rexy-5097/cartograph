# Django URL-conf composition

```text
path("api/instances/", include("plane.license.urls"))   plane/urls.py
path("admins/", InstanceAdminEndpoint.as_view())        plane/license/urls.py
                                             ─► api/instances/admins/
```

Added by the correctness-recovery programme (BUG-0002). The 2026-10-30
large-codebase assessment found that `path("api/", include("app.urls"))` was
recorded as a route whose handler is `include`. A frontend call to a path under
the mount matched the mount itself, and the view behind it matched nothing.
Both of Plane's HTTP edges, and 54 of Label Studio's, pointed at `include`.

## The model

A Django mount is not a route, so the parser records it separately, as a
`UrlConfInclusion`: the prefix, and what is mounted. `UrlConfIndex`
(`crates/cartograph-resolver/src/urlconf.rs`) then composes the prefix an entry
is served under, the way [routers.md](routers.md) composes a `FastAPI` router's.
The unit is different: a Django entry names no receiver, so what gets mounted
is a module or a list inside one, and entries are placed by file and position.

## What composes

| Mount | Example | Mounted |
|---|---|---|
| A dotted module | `include("plane.license.urls")`, `include(("x.urls", "x"))` | every entry in that module |
| An imported module | `from proj.shop import urls as shop_urls` … `include(shop_urls)` | every entry in that module |
| An imported `urlpatterns` | `from .analytic import urlpatterns as analytic_urls` | every entry in that module |
| An imported or local list | `include(extra_patterns)`, `include((_api_urlpatterns, app_name))` | the entries inside that list |
| An inline list | `include([path(...), ...])` | the entries written inside it |
| A spread into `urlpatterns` | `urlpatterns = [*analytic_urls]`, `urlpatterns += [*more]` | as above, with an empty prefix |

Composition is plain string concatenation, as Django does it: `"api/"` +
`"instances/"`, and `"api"` + `"x/"` really is `"apix/"`. Nothing adds or drops
a separator.

A `re_path` mount composes only when its pattern is an **anchored literal**:
`^` (consumes nothing) or `^api-auth/` (consumes exactly `api-auth/`). Django
searches for the pattern and passes the rest of the path on, so that reading is
exact. Any other regex is not interpreted.

## What refuses

| Outcome | When |
|---|---|
| `Unresolved` | the mount's pattern is not a plain string, or is a regex other than an anchored literal |
| `Unresolved` | mounts cycle, or nest deeper than twelve |
| `Ambiguous` | a module or list is mounted in more than one place |
| — | the target cannot be found: DRF's `router.urls`, an attribute of an object, a dotted path that matches no file or several |

Every refusal leaves the entry **exactly as written**, as in
[routers.md](routers.md): an unknown prefix never becomes an empty one. A
`re_path` entry is never composed, because it is never interpreted.

A module that nothing mounts is treated as a root and served as written. That is
how the root URL conf (`ROOT_URLCONF`) is reached, since settings are not read.
It is also what happens to a module mounted by a mechanism this analysis does
not see.

## What is out of scope

- DRF routers and their `router.urls`.
- `path("admin/", admin.site.urls)`, which is still recorded as an entry whose
  handler is `admin.site.urls`.
- `ROOT_URLCONF` and `urlconf=` overrides read from settings.
- `i18n_patterns`.
- A module mounted under two prefixes is ambiguous, although Django serves it
  at both. This matches the router policy, and it is a known recall cost.
