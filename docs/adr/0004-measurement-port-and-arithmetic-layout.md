---
status: accepted
date: 2026-06-15
tags: [core, view, layout, performance, ports, measurement]
related: [docs/plans/2026-06-15-vellum-design.md, docs/adr/0001-grapheme-segmentation.md]
---

# ADR-0004: Measurement port and pure-arithmetic layout

## Context

The dominant performance enemy in a DOM editor is **forced synchronous reflow**.
Reading `getBoundingClientRect`, `offsetHeight`, or similar in a hot path forces a
full layout recalculation (~94ms per 1000 items), the trap that bloats general
editors (design §5). Pretext.js demonstrated the way out: its edge is a
**discipline, not magic** — touch the impure/slow thing once, cache it, then do
everything as pure arithmetic, yielding roughly 500× speedups with full i18n/bidi.

That discipline maps directly onto Vellum's hexagonal split, but only if layout
never reaches into the DOM to measure.

## Decision

**No DOM reads (`getBoundingClientRect`/`offsetHeight`/etc.) in hot paths.**
Measurement is isolated behind an outbound **`MeasurePort`**:

- **`MeasurePort` (outbound, impure)** — a Canvas adapter in TS implementing
  `advance(grapheme, font) → width` via `measureText()` (no reflow). It is the
  only thing that touches the browser for sizing, and it measures **once** per
  `(segment, font)` and caches.
- **Layout = pure arithmetic in Rust `core`** over the buffer plus cached widths:
  line breaks, wrapping, caret pixel position, and visible viewport range are
  computed with zero reflow and are 100% testable without a DOM.

**Monospace fast path:** a code/prompt editor is monospaced, so the cache
collapses to one advance per grapheme class and layout becomes `column × advance`
— simpler and faster than Pretext on its own (general, proportional) turf, while
the general path remains available behind the same port.

Layout consumes graphemes, not bytes (see ADR-0001), so the caret never splits a
cluster.

**Increment scope:** the `MeasurePort` and arithmetic layout land in **Increment
1** alongside virtualized rendering. Increment 0 renders simple line DOM without a
measurement port; this ADR fixes the rule before any layout code is written so the
"never measure via the DOM" contract is honored from the first layout commit.

## Consequences

- Layout is fast (no reflow) and fully unit-testable in pure Rust, satisfying the
  performance discipline and the high-coverage-on-core goal.
- The browser-specific, impure measurement is a single small adapter that can be
  faked in tests, keeping the dependency direction outward.
- The monospace fast path keeps the common case trivial; the proportional path
  exists but is not paid for until needed.
- Contributors and agents have a bright-line rule: a DOM size read in a hot path
  is a boundary violation, catchable in review.
