---
status: accepted
date: 2026-06-15
tags: [core, unicode, segmentation]
related: [docs/plans/2026-06-15-vellum-design.md, docs/adr/0004-measurement-port-and-arithmetic-layout.md]
---

# ADR-0001: Grapheme segmentation in core via `unicode-segmentation`

## Context

The classic editor bug — emoji, combining marks, and CJK breaking the caret —
comes from moving the cursor by byte or by `char` instead of by user-perceived
grapheme cluster (design §5.4). To place a caret, move a selection, or measure a
line correctly, the engine must segment text into grapheme clusters and find
Unicode line-break opportunities.

There are two places this segmentation could live:

- In Rust `core`, using the `unicode-segmentation` crate.
- In the TS view, using the browser's native `Intl.Segmenter`.

`core` is meant to be pure, autonomous, and fully testable without a browser or a
WASM runtime (design §3, §7). Pushing segmentation into the view would make a
foundational text operation depend on the DOM environment and would split the
text model across the language boundary.

## Decision

Segment graphemes **in `core`** using the `unicode-segmentation` crate (tiny,
vetted, no transitive weight), so cursor movement, selection, and layout
arithmetic remain pure Rust and unit-testable with no browser present.

Leave a **clean seam at the view boundary**: the `MeasurePort` and view input
adapters are the only place that could later defer to the browser's
`Intl.Segmenter` if a concrete need appears (e.g. locale-tailored segmentation
the crate does not cover). We do not build that adapter now (YAGNI); we only keep
the segmentation contract narrow enough that swapping it stays a localized change.

This is `proposed` until Increment 1 exercises the grapheme cursor for real
(Increment 0 uses a simple `String` buffer with char-boundary-safe ops and does
not yet move by grapheme).

## Consequences

- `core` owns the correctness-critical Unicode logic and proves it in isolation,
  matching the "pure, no browser" posture and the performance discipline of
  measuring/segmenting once then doing arithmetic.
- One justified dependency enters `core` (`unicode-segmentation`), consistent with
  the minimal-deps rule — each crate enters with a documented reason.
- The view stays free of text-model responsibility; it renders and measures, it
  does not segment.
- If a future locale need outgrows the crate, the swap to `Intl.Segmenter` is a
  view-boundary adapter change, not a `core` rewrite — but it remains unproven
  until that need is real.
