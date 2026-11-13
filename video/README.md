# Cartograph videos

Two films rendered with [Remotion](https://www.remotion.dev/) from code in
`src/`, with every sound synthesised by `audio/synth.py` — no samples, no
footage, nothing downloaded.

- **Walkthrough** (`src/Walkthrough.tsx`, ~2 min) — how Cartograph grew,
  milestone by milestone, then what it reads, how it resolves, the Airflow
  trace, the M08 measurements and its principles.
- **Commercial** (`src/commercial/`, ~84 s) — a 90s tape, the decades, then
  today: the problem, what Cartograph does, how, what it holds to, and the
  vision.

Every sentence and figure on screen comes from the README, the landing page
or the milestones, which trace each figure to a committed benchmark run.
Add no number that is not already there.

## Build

```bash
npm install          # also copies the Geist fonts into public/
npm run audio        # synthesises public/audio/ (Python 3 + numpy)
npm run studio       # preview and scrub in the browser
npm run render:commercial
npm run render
```

Renders land in `out/`, which is not committed.
