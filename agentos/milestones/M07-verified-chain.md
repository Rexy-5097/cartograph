# M07 — Full verified chain on a real repository

Target: Day 14 · Branch: `feature/m07-verified-chain` · Tag on acceptance: `cartograph-m07`

Scope: run the whole pipeline against a real public full-stack repository; gix
for commit stamping. HARD GATE 1: if this slips past day 21, narrow to
TypeScript→FastAPI only.

Acceptance: at least one complete 4-hop chain (component→route→handler→model→
table) derived from a repo not authored for the test, every edge carrying real
evidence; failure modes documented; gates pass.

Standard exit: PR + CI green + self-review + human acceptance + checkpoint tag + state update + STOP.
