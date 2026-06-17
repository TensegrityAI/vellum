---
status: accepted
date: 2026-06-15
tags: [core, buffer, rope, dependency]
related: [docs/adr/0001-grapheme-segmentation.md, docs/adr/0002-event-sourced-buffer.md, docs/plans/2026-06-15-vellum-increment-1.md]
---

# ADR-0006: Back `TextBuffer` with the `ropey` rope crate

## Context

Increment 0 shipped a deliberately simple `String`-backed buffer to prove the
core → WASM → view pipeline (ADR-0002). Increment 1 turns that scaffold into a
real, event-sourced, editable buffer. A `String` backing store makes every
insert/delete an O(n) memmove and offset arithmetic over the whole text, which
does not scale to real documents and makes the event-sourced model expensive to
exercise.

The correct data structure for an editable text buffer is a **rope** — balanced,
with cheap edits and offset lookups. Two paths exist:

- **Hand-roll a rope in `core`** now, for full control and zero dependencies.
- **Adopt a vetted rope crate** and abstract it behind the existing `TextBuffer`
  API.

Hand-rolling a correct, performant rope (UTF-8/char-boundary handling, balancing,
line indexing) is a substantial, bug-prone effort that is not where Increment 1's
value lies. The buffer's job in this increment is to be correct and fast enough
so the event-sourced model, undo/redo, and offset conversions can be built on a
solid base.

## Decision

Back `TextBuffer` with the **`ropey`** crate (vetted, minimal, used by Helix)
rather than hand-rolling a rope now. `ropey` gives correctness and speed today
with a small, well-exercised footprint.

The `TextBuffer` API already abstracts the storage: callers (`Document`, the
offset conversions, the WASM bindings, the view) speak to `TextBuffer`, not to
the backing store. A hand-rolled rope can therefore replace `ropey` later — if it
ever constrains us — **without touching callers**. This keeps the dependency a
private implementation detail behind a stable port.

Grapheme and word boundaries are **not** taken from `ropey`. They continue to come
from `unicode-segmentation` (ADR-0001), layered on top of `ropey`'s char/byte
indexing. `ropey` provides the storage and byte/char/line offsets; segmentation
stays in `core` as the correctness-critical Unicode logic.

`ropey` is **MIT**-licensed, which is already in the `deny.toml` `[licenses]`
allow-list — no policy change is required and `cargo deny check` accepts it.

## Consequences

- Increment 1 gets a correct, fast editable buffer immediately, focusing effort on
  the event-sourced model, undo/redo, and offset conversions rather than on
  rope internals.
- One more justified dependency enters `core` (`ropey`), consistent with the
  minimal-deps rule — it earns its place by replacing hand-rolled rope complexity.
- The dependency is replaceable: because `TextBuffer` is the boundary, swapping in
  a hand-rolled rope later is a localized change behind the same API, with no
  caller churn — matching the deferred "hand-rolled rope (if `ropey` ever
  constrains us)" item on the roadmap.
- `core` stays `#![forbid(unsafe_code)]`; `ropey` is used as a normal safe
  dependency and does not weaken that guarantee.
- Grapheme/word segmentation remains owned by `core` via `unicode-segmentation`
  (ADR-0001), keeping Unicode correctness in one place independent of the storage.
