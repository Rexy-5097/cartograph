<div align="center">

# Cartograph

**See how your frontend, backend and database really connect, with proof for every link.**

[![CI](https://github.com/Rexy-5097/cartograph/actions/workflows/ci.yml/badge.svg)](https://github.com/Rexy-5097/cartograph/actions/workflows/ci.yml)
[![License: FSL-1.1-ALv2](https://img.shields.io/badge/license-FSL--1.1--ALv2-blue.svg)](LICENSE)
![Version 0.1.0](https://img.shields.io/badge/version-0.1.0-orange.svg)

[Website](https://cartograph.co.in) · [Quick start](#quick-start) · [How it works](#how-it-works) · [Docs](docs/)

</div>

---

## The problem

You click **Place order** in a web app. Behind that button:

1. a TypeScript function sends `POST /api/orders`,
2. a Python handler receives it,
3. the handler creates an `Order` model,
4. and the model writes to the `orders` table.

Those four pieces live in different files, in different languages, often maintained by different people. No single tool sees the whole chain. So when you need to know *"what does this button actually touch?"* or *"what breaks if I change this table?"*, you end up grepping and guessing.

## What Cartograph does

Cartograph reads your source code and draws that chain for you, from the button all the way to the database table.

Every link it draws comes with **evidence**: the file and line it was found at, the rule that matched it, and how confident that rule is. When it isn't sure, it **draws nothing** rather than guessing.

It runs entirely on your machine. Your code never leaves it.

---

## See it in 60 seconds

Take a tiny app with three files:

```ts
// web/checkout.ts
export async function placeOrder(order) {
  return fetch('/api/orders', { method: 'POST', body: JSON.stringify(order) });
}
```

```python
# api/routes.py
from fastapi import FastAPI
from .models import Order

app = FastAPI()


@app.post("/api/orders")
def create_order(payload: dict):
    return Order(**payload)
```

```python
# api/models.py
from sqlalchemy.orm import DeclarativeBase


class Base(DeclarativeBase):
    pass


class Order(Base):
    __tablename__ = "orders"
```

Ask Cartograph where the frontend function leads:

```console
$ cartograph trace placeOrder

placeOrder  (function)  web/checkout.ts:2
  │
  ↓ HTTP-CALL   confidence 0.98   provenance route-matcher
  │   evidence   POST /api/orders matched POST /api/orders at api/routes.py:7 (create_order); exact HTTP method; every path segment static and equal
  │   observed   web/checkout.ts:2
  │
create_order  (function)  api/routes.py:7
  │
  ↓ ORM-ACCESS   confidence 0.80   provenance orm-resolution
  │   evidence   Order(...) in create_order at api/routes.py:9
  │   observed   api/routes.py:9
  │
Order  (class)  api/models.py:8
  │
  ↓ QUERIES   confidence 0.80   provenance orm-resolution
  │   evidence   Order maps to table "orders"; __tablename__ = "orders"
  │   observed   api/models.py:8
  │
orders  (table)

This path crosses the HTTP boundary and reaches a database table.
```

Or ask the reverse question, *"if I change the `orders` table, what depends on it?"*:

```console
$ cartograph blast orders

blast radius of Table orders

  3 artefacts depend on it, up to 3 hops away

  depth   conf  kind       artefact                           reached through
      1  0.800  Class      Order                              Queries api/models.py:8
      2  0.800  Function   create_order                       OrmAccess api/routes.py:9
      3  0.800  Function   placeOrder                         HttpCall web/checkout.ts:2
```

Both outputs are real: copy the three files, run the commands, and you get the same result (each command also prints a short footer, trimmed here). On real projects the chain can run across thousands of files. The same `trace` works on [Apache Airflow](https://github.com/apache/airflow), for example, from its generated TypeScript client to the `connection_test_request` table.

---

## Quick start

You need [Rust](https://rustup.rs). The exact version is pinned in the repo, and `rustup` picks it up automatically.

```bash
cargo install --git https://github.com/Rexy-5097/cartograph cartograph-cli
```

Then, from inside any project:

```bash
cartograph .
```

That prints a summary of what was found: routes, HTTP calls, database models, and the links between them. It also shows what Cartograph *refused* to link, and why.

> Prebuilt binaries and an npm package are not published yet, so installing from source is currently the only way. (Packages named `cartograph` on npm belong to other projects.)

### The four questions it answers

| You want to know… | Run |
|---|---|
| What's in this codebase and how is it wired? | `cartograph .` |
| How does this function reach the database? | `cartograph trace <symbol>` |
| What breaks if I change this? | `cartograph blast <symbol>` |
| What did my branch change in the architecture? | `cartograph diff ../main-checkout .` |

Add `--json` to any command for machine-readable output. The [full command reference](docs/development/cli.md) covers every flag and exit code.

---

## How it works

Cartograph works in three steps. None of them uses AI.

**1. Read.** It parses your TypeScript, TSX and Python files and records what they *say*: functions, imports, `fetch(...)` calls, `@app.post(...)` route decorators, ORM model classes.

**2. Connect.** It joins the pieces across languages:
- **Requests to routes.** `` fetch(`/api/orders/${id}`) `` on the frontend is matched to the route that serves it, even though each framework writes routes differently: `/orders/:id`, `/orders/{id}` and `/orders/<int:id>` all mean the same thing.
- **Handlers to models.** A handler is linked to the ORM models it uses.
- **Models to tables.** Each model is linked to the database table it maps to.

**3. Prove.** Every link stores *why* it exists. If two routes could both match a request, Cartograph draws neither and reports the request as ambiguous. A missing link is easy to spot; a wrong one quietly misleads you.

### Words you'll see in the output

| Term | Meaning |
|---|---|
| **node** | One thing in your system: a function, a class, a route, a database table |
| **edge** | A link between two nodes, such as "this function calls that route" |
| **evidence** | Where and how the edge was found: file, line, matching rule |
| **confidence** | How strong the matching rule is. It is a fixed score per kind of evidence, **not** a probability, so don't treat 0.80 as "80% likely" |
| **refused** | A link Cartograph could have guessed but didn't, because the evidence was ambiguous or missing |

---

## What it supports today

| Layer | Supported |
|---|---|
| **Frontend** | TypeScript and TSX: `fetch`, `axios` (under any import name), clients named `client` / `http` / `api`, and generated OpenAPI clients |
| **Backend** | Python: FastAPI routes (including router prefixes), Flask routes, and Django URL configs (including `include(...)`) |
| **Database** | SQLAlchemy and Django ORM models, linked to their tables |

**Not supported yet:**
- other languages;
- JavaScript backends such as Express;
- Django REST Framework routers (`router.register`, view sets, `@action`);
- the methods of a class-based view: a request is followed to the view class,
  not into its `get` or `post`;
- an axios instance's `baseURL`, and base URLs set at runtime;
- queries written as `session.query(Model)` or raw SQL.

See the [complete list of limitations](docs/resolver/README.md#known-limitations).

---

## Ways to use it

| | |
|---|---|
| **CLI** | Everything above. Start here. |
| **Desktop app** | Draws your architecture as an interactive graph; click any edge to see its evidence. Lives in [`desktop/`](desktop/) and must be built from source for now. |
| **AI assistants (MCP)** | `cartograph-mcp` lets an assistant such as Claude ask `trace`, `blast`, `map` and `diff` questions about one repository you choose when you launch it. The assistant reads the graph; it never builds it. |
| **Pull-request review** | Run `cartograph diff --markdown` in CI to comment on each PR with what changed in the architecture. This repo does that for its own PRs ([workflow](.github/workflows/architecture-review.yml)); there is no packaged GitHub Action yet. |

To set up the MCP server:

```bash
cargo install --git https://github.com/Rexy-5097/cartograph cartograph-mcp
cartograph-mcp --repository /path/to/your/repo --repository-identity my-repo
```

---

## How accurate is it?

The edge figures come from seven pinned commits of large open-source projects: Superset, PostHog, Zulip, Onyx, Airflow, AutoGPT, and the FastAPI full-stack template. Every edge Cartograph produced there was labelled, and every one that could be was re-checked against the source by tools that share no code with Cartograph.

| | |
|---|---|
| Edges produced | 14,932 |
| Edges that could be independently checked | 11,221 |
| Wrong edges among those | **1** |
| Edges that could not be checked | 3,711 (24.9%) |

Read that last row too. A quarter of the edges could not be checked, and they cluster where the evidence is weakest. So treat the result as a best case, not a guarantee.

Coverage, meaning how many real connections it finds, was measured on one project only, Airflow. Of the 129 API operations that Airflow's generated frontend client declares and its backend serves, Cartograph linked 117.

The method and every caveat are in the [benchmark report](docs/benchmarks/m08-report.md).

---

## Private by design

- **Your code stays on your machine.** Always, with no setting to change it.
- **No AI in the graph.** Every link comes from static analysis. This is a permanent design rule ([ADR-0007](docs/adr/ADR-0007-no-llm-graph-construction.md)), not a temporary limitation.
- **AI explanations (desktop app) are opt-in**, per repository and off by default. Cartograph is fully useful with no API key and no network.
- **Secrets stay secret.** Credentials go in your OS keychain, and source code, tokens and environment variables are never logged.

More detail: [SECURITY.md](SECURITY.md).

---

## FAQ

**Why didn't it link two things I know are connected?**
Usually because the evidence was ambiguous, or the URL is built at runtime from something it can't see, such as an environment variable. Run `cartograph match .` to see every refusal and its reason.

**What do the confidence numbers mean?**
Which kind of evidence was used. An exact route match scores 0.98, and an ORM link scores 0.80. They are not probabilities.

**Is it production-ready?**
It's an early release (v0.1.0). It covers a focused set of TypeScript and Python web stacks, the ones listed above, and tells you plainly when something falls outside them.

**Does it look at Git history or my database?**
No. It reads only the source files under the path you give it.

---

## Contributing

Contributions are welcome. To build and test from a clone:

```bash
cargo build --release -p cartograph-cli
cargo test --workspace
```

| Where | What |
|---|---|
| [`crates/`](crates/) | The engine and CLI, in Rust |
| [`desktop/`](desktop/) | The desktop app (Tauri + React); no analysis code lives here |
| [`docs/`](docs/) | Architecture, design decisions and benchmarks; also published at [cartograph.co.in](https://cartograph.co.in) |

Start with [CONTRIBUTING.md](CONTRIBUTING.md), then [ARCHITECTURE.md](ARCHITECTURE.md) for the big picture. Planned work is in [ROADMAP.md](ROADMAP.md) and past releases are in [CHANGELOG.md](CHANGELOG.md).

<sub>For maintainers: [project rules](PROJECT_RULES.md) · [design decisions](docs/adr/) · [checkpoints](CHECKPOINTS.md) · [instructions for AI coding assistants](AGENTS.md) · [development process](AGENTOS.md). The `agentos/` folder holds that process (milestones, quality gates, review checklists); you don't need it to use Cartograph.</sub>

---

## License

Cartograph is released under the [Functional Source License](LICENSE) (FSL-1.1-ALv2). In plain words:

- **You can** use it for free at work or at home, read the code, change it, and share your changes. That includes running it on your company's own codebases.
- **You can't** sell it, or offer it as a product or service that competes with Cartograph.
- **Every version becomes fully open source** (Apache-2.0) two years after it is released.

Versions published before this licence was adopted remain under Apache-2.0 ([why and how it changed](docs/adr/ADR-0023-functional-source-license.md)). The vendored `agentos/` framework keeps its own MIT licence ([its licence](agentos/LICENSE)).
