# M18 experiment — browser architecture explorer

**Experimental. Not accepted architecture.** See the proposed [ADR-0024](../docs/adr/ADR-0024-browser-client-on-loopback.md).

This is Cartograph's real analysis of a randomly drawn repository (Flagsmith at `899aa38`), served to a browser tab on 127.0.0.1 and explored with the desktop's own renderer.

## Read in this order

1. [final-experiment-assessment.md](final-experiment-assessment.md): the verdict and the success criteria
2. [repository-selection.md](repository-selection.md): the pool, the seeded draw, the pin
3. [output-analysis.md](output-analysis.md): what Cartograph actually found, and what it did not
4. [existing-frontend-audit.md](existing-frontend-audit.md): the 13 code-inspection answers
5. [browser-architecture-design.md](browser-architecture-design.md)
6. [api-contract.md](api-contract.md)
7. [implementation-report.md](implementation-report.md): commits, files, reproduction commands
8. [visual-usability-review.md](visual-usability-review.md): screenshots and findings
9. [performance-results.md](performance-results.md)
10. [security-validation.md](security-validation.md)
11. [regression-test-results.md](regression-test-results.md)
12. [known-limitations.md](known-limitations.md)

## Code

| Location | What it is |
|---|---|
| `browser-explorer/server/` | `cartograph-serve`, an excluded package |
| `browser-explorer/verify/` | the API-vs-CLI check and the edge-case probe |
| `../desktop/src/browser/` | the browser front end |
| `../crates/cartograph-desktop/src/{report,trace,inspect,source}.rs` | the backend additions |

## Outside the repository

`D:/WorkSpace/Projects/cartograph-reviews/2026-11-25-m18-browser-explorer/` holds the material deliberately kept out of the repository:

- the pinned clone;
- the CLI and server binaries, with their SHA-256 sums;
- the raw Cartograph output;
- the screenshots;
- the selection working files.
