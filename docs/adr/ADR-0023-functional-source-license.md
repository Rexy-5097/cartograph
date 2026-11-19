# ADR-0023 — Cartograph is licensed under the Functional Source License (FSL-1.1-ALv2)

**Status:** Proposed · 2026-11-19 · Accepted when every author listed under
[Consent](#consent) has agreed and the owner merges the pull request

(ADR-0022 is reserved by the unmerged application-identity work, so this
decision takes the next free number.)

## Context

Cartograph has been licensed under Apache-2.0. Apache-2.0 lets anyone take the
code, rebrand it, and sell it as a product or hosted service, owing the authors
nothing beyond keeping their notices. For a small team, the realistic risk of a
public repository is that a better-funded company does exactly that.

The repository is public, and the project's case to users rests on it staying
readable: "check the evidence yourself" applies to the analyser as much as to
its output. Hiding the code would remove that and work against the public
launch (M17). So the goal is to keep the source visible while removing the
right to commercialise Cartograph as a competing product.

## Decision

From the commit that merges this decision, Cartograph is licensed under the
**Functional Source License, Version 1.1, ALv2 Future License**
(SPDX `FSL-1.1-ALv2`).

- `LICENSE` holds the official template from
  [getsentry/fsl.software](https://github.com/getsentry/fsl.software)
  (`FSL-1.1-ALv2.template.md`, sha256 `36b6082235c0…dace56177`) with exactly two
  placeholders filled: the year, `2026`, and the licensor,
  `The Cartograph contributors`, matching the site's existing
  "© Cartograph contributors". No other word is changed.
- Every manifest that declares a licence says `FSL-1.1-ALv2`: the Cargo
  workspace (inherited by all ten crates), the desktop shell's Cargo manifest,
  and the `npm/`, `docs/` and `video/` package manifests.
- Contributions are accepted under the same licence (CONTRIBUTING.md).
- The vendored AgentOS framework under `agentos/` is third-party and keeps its
  MIT licence.

What the licence means in practice:

| Use | Allowed? |
|---|---|
| Running Cartograph on your own or your employer's code | Yes ("internal use and access") |
| Reading, modifying and redistributing it for a permitted purpose | Yes |
| Non-commercial education and research | Yes |
| Professional services for a licensee using it under these terms | Yes |
| Selling it, or offering a commercial product or service that substitutes for it or offers substantially similar functionality | **No** (a "Competing Use") |
| Any use under Apache-2.0, two years after a version is made available | Yes, irrevocably |

## Consent

Changing the licence of code that is already written needs the agreement of
every copyright holder. CONTRIBUTING.md accepted contributions under
Apache-2.0, and there is no contributor licence agreement that would let one
party relicense alone. The authors in the history of `main`, by git identity:

| Author (git identity) | Agreed |
|---|---|
| Soumyadeb Tripathy | pending |
| Rexy-5097 | pending |
| ronitsaha11 / Ronit Saha | pending |
| Basant Bhushan | pending |

Each author records agreement by approving or commenting on the pull request
that proposes this ADR. The pull request must not merge until every row is
agreed. If two identities belong to one person, one agreement covers both.

## Alternatives

| Option | Why not |
|---|---|
| Keep Apache-2.0 | Leaves commercial resale fully permitted, which is the risk this decision addresses |
| Make the repository private | The only way to stop people *reading* the code, but it ends the public launch and the verifiability users rely on, and what is already published stays published |
| Business Source License 1.1 | Close to FSL, but each project writes its own "Additional Use Grant" and change date, so users must read custom terms. FSL fixes both, so the terms are the same in every FSL project |
| PolyForm Noncommercial, Elastic License 2.0 | Restrict use without ever converting to open source; PolyForm Noncommercial would also stop companies using Cartograph internally, which we want to allow |
| AGPL-3.0 | Still allows a company to sell Cartograph as long as it publishes its changes. Its network clause adds little for a tool that runs on the user's own machine |

## Consequences

- **Cartograph is no longer "open source"** in the Open Source Initiative's
  sense. It is *source available*. Wording that called it open source (the
  promotional video's end card) now says "Source available". Historical
  records, such as CHECKPOINTS.md's "Open-source fork" rows, describe the past
  and are not rewritten.
- **Nothing already published changes.** Every commit before the merge, and
  every copy or fork taken from it, stays under Apache-2.0 for good. The
  change protects new work from the merge onwards.
- **Each version opens up after two years.** The future-licence grant is
  irrevocable, so the restriction is time-limited by design.
- **Dependencies allow it, and their obligations are unchanged.** Checked
  against the lockfiles on 2026-11-19:
  - The Cargo workspace's 262 dependencies are all under permissive licences
    (MIT, Apache-2.0, BSD, ISC, Zlib, Unlicense, 0BSD, Unicode-3.0,
    CDLA-Permissive-2.0). One
    offers LGPL-2.1 only as an alternative to MIT or Apache-2.0.
  - The desktop shell adds five MPL-2.0 crates through Tauri: `cssparser`,
    `cssparser-macros`, `dtoa-short`, `selectors` and `option-ext`. MPL-2.0 is
    file-level copyleft. It allows them inside a larger work under any licence,
    and its conditions apply only to those crates' own files, exactly as they
    did under Apache-2.0.
  - The desktop app's npm packages are all permissive. The docs site uses two
    OFL-1.1 fonts.
  - `video/` is tooling for rendering the promotional film, not part of the
    shipped product. Remotion's own licence and three MPL-2.0 audio encoders
    there are unaffected by this decision.
  - Every dependency's notices must still be preserved when redistributing.
- **Tooling.** `FSL-1.1-ALv2` is on the SPDX licence list (3.29.0), marked not
  OSI-approved. GitHub's licence detection may show it as "Other". The crates
  are not published to crates.io, so no registry validates the field.
- **This is not legal advice.** The terms are Sentry's published FSL, unchanged
  apart from the two placeholders; a lawyer should confirm the licensor wording
  before the first release under it.
