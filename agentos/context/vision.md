# Vision — Cartograph

> Layer: project · Read before every task · Source of truth: Frozen Engineering Specification V3 (August 2026)

## One sentence

Compute the architecture. Prove the relationships.

## What Cartograph is

A local-first architectural intelligence engine that continuously derives a
verified graph of a software system from source code, language semantics, HTTP
boundaries, database schemas and Git history.

## The differentiator

Not a pretty dependency graph. **Trustworthy cross-stack resolution**: a React
component, an HTTP route, a Python handler and a database table are four
artifacts in three languages with no shared symbol table. Cartograph connects
them and shows its working:

```
CheckoutButton.tsx → POST /api/orders → create_order() → Order → orders table
```

Every hop annotated with how it was derived and how much to trust it.

## Governing principle

Deterministic analysis first. Evidence second. Inference third. The model last,
**and never in the graph**. A language model never constructs graph structure.

## Success criteria

1. **M07**: `cartograph analyze .` derives a verified cross-stack chain on a
   real repository.
2. **M08**: CrossStack-Bench published with real precision/recall/F1 and
   calibration error — failures included.
3. **M09**: v0.1.0 public; 100 real users within 30 days or reposition.
4. **M13**: DIFF creates ≥15% seven-day return or MAP investment is cut.

## Product surfaces (build order)

MAP (M11) → BLAST (M12) → DIFF (M13) → ASK (M16).
Resolver → graph → CLI → benchmark → app. Never the reverse.

## Non-goals

Mobile · accounts/billing · >3 languages before M09 · complete LSP coverage ·
settings panels/dashboards/marketing sites · custom GPU renderer before
Sigma.js is measurably the bottleneck · any cloud component before a user asks
and offers to pay · a language model anywhere inside graph construction.

Binding rules: [../../PROJECT_RULES.md](../../PROJECT_RULES.md)
