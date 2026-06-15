---
status: proposed
date: 2026-06-15
tags: [packaging, oss, governance, release]
related: [docs/plans/2026-06-15-vellum-design.md, docs/adr/0002-event-sourced-buffer.md]
---

# ADR-0005: Repository and packaging strategy

## Context

Vellum is built as a **separate, domain-agnostic, publishable engine** rather than
code embedded in the Nexum admin app, precisely so it can be iterated in isolation
and eventually opened to the world for Akaisys visibility (design §1, §7). That
requires up-front decisions on repository boundary, license, npm scope, and the
moment of public release, so the history stays OSS-clean from commit one.

Open questions from the design (§8 ADR-0005): which repo boundary, which npm
scope (`@nexum/*` vs a neutral scope), and which forge.

## Decision

- **Separate OSS repository.** Vellum lives in its own repo, not inside
  `nexum-rag-client` or the backend, so it is consumable standalone and not
  coupled to `agentId`/admin-store the way the legacy editor was.
- **License: Apache-2.0** — permissive, patent-granting, coherent with the
  `kineticrs` house signature and suitable for external adoption.
- **npm scope: leaning `@vellum/*`** (ratified at the public flip) — a neutral,
  product-named scope (`@vellum/core`, `@vellum/view`, `@vellum/react`) rather than
  `@nexum/*`, keeping the engine domain-agnostic and unbranded by its first consumer.
- **Private → public flip after Increment 0.** The repo is authored OSS-ready
  (clean history, no secrets, full governance) but stays **private** until the
  Increment 0 walking-skeleton demo is judged to "shine" — typing Jinja2 shows
  live highlighting with every edit flowing through WASM. Only then do we flip it
  public.

This ADR is `proposed`: the npm scope and the forge choice (GitHub vs GitLab) are
ratified at the public-flip moment, when the first published package and the
public remote are actually created.

## Consequences

- The engine is reusable and publishable, free of Nexum coupling, matching the
  motivation for building rather than embedding.
- Apache-2.0 and a clean history make the eventual open-sourcing low-friction.
- A neutral `@vellum/*` scope avoids re-branding packages later and signals the
  engine is not Nexum-specific.
- The private-until-it-shines gate protects against publishing a half-proven bet;
  the cost is that external visibility waits on the demo landing.
- Final npm-scope and forge commitments remain open until the flip, so this record
  is revisited (and moved to `accepted`) at public release.
