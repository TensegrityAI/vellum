---
status: accepted
date: 2026-06-15
tags: [core, cursor, selection, aggregate, ddd, borrow]
related: [docs/adr/0002-event-sourced-buffer.md, docs/adr/0006-rope-buffer.md, docs/plans/2026-06-15-vellum-increment-1.md]
---

# ADR-0008: The cursor lives in the `Document` aggregate

## Context

Phase F gave `core` two pieces that must cooperate:

- the [`Document`](../../crates/core/src/document.rs) aggregate (`buffer` +
  `undo`/`redo`), the event-sourced **write side** of the editor (ADR-0002); and
- [`Selection`](../../crates/core/src/cursor.rs) (`anchor`/`head`), a pure value
  object whose grapheme/word movement methods take a `&TextBuffer` and delegate
  Unicode segmentation to it (ADR-0001).

In Increment 0/F these were unrelated: a caller held a `Document` and a separate
`Selection`, and moved the cursor with `sel.move_right(doc.buffer())`. The Phase F
audit flagged this as a **borrow trap** and a DDD smell:

1. **Borrow trap.** `doc.buffer()` borrows **all of `&self`**. So the natural
   `self.selection.move_right(self.buffer())` inside a `Document` method does not
   compile — it borrows `self` (via the getter) while also mutably borrowing
   `self.selection`. The conflict is real, not cosmetic: any attempt to give the
   aggregate a cursor through the public getter fights the borrow checker.
2. **DDD smell.** With the cursor outside the aggregate, "where is the caret in
   this document" has no single source of truth, and edits that should move the
   caret (type-over-selection, delete-selection) have to be re-implemented by every
   caller, outside the consistency boundary that owns the text.

The clean resolution the audit recommends is to make the cursor a **field of the
aggregate** and reach the buffer and selection as **disjoint fields**, not through
the all-of-`self` getter.

## Decision

`Selection` becomes a field of the `Document` aggregate. Cursor movement and
selection-aware editing are **intent methods on `Document`**; the aggregate is the
single public mutation **and** navigation entry point for a document.

- `Selection` stays a **pure value object**. Its `&TextBuffer`-param movement
  methods (`move_left/right`, `extend_*`, `move_word_*`, `extend_word_*`,
  `collapse`, …) are **kept** — they remain independently unit-testable, and the
  aggregate now also calls them internally.
- The cursor-intent methods on `Document` delegate to those `Selection` methods
  using **disjoint field borrows**: `self.selection.move_left(&self.buffer)` —
  accessing the `self.buffer` **field directly**, never `self.buffer()` the
  getter. The borrow checker sees two distinct fields and accepts the borrow; this
  disjoint-field-borrow is the whole point of moving the cursor inside the
  aggregate.
- Selection-aware editing (`insert_at_cursor`, `delete_selection`, `backspace`,
  `delete_forward`) builds on the existing explicit-offset `insert`/`delete`
  history-recording mutators, so every cursor edit still flows through the
  event-sourced undo/redo machinery (ADR-0002).

The cursor lives where the text lives: the document is the consistency boundary
that owns both, and the answer to "where is the caret in this document" has one
source of truth.

## Consequences

- **The `buffer()` getter stays** — for read-only access (tokenize, measure,
  offset conversions, cursor positioning by the view). It is no longer the path for
  mutation or navigation: those go through `Document` so history and cursor stay
  consistent. The getter borrows all of `&self`, which is exactly why the internal
  intent methods must use `&self.buffer` instead.
- **Single selection in Increment 1.** The aggregate holds exactly one
  `Selection`. Multi-cursor / multi-selection is deferred (a future
  `Vec<Selection>` or selection set), out of Inc-1 scope.
- **Selection rebasing stays explicit, not automatic.** `core` does **not**
  auto-rebase the selection against an external/explicit edit (the Inc-1 contract
  pinned by `cursor_offsets_are_not_rebased_after_edit_inc1_contract`). The view
  layer (Task I5) owns rebasing a caret across an upstream insert. The **only**
  offset adjustment the aggregate makes is:
  - the cursor-aware edit methods (`insert_at_cursor`/`delete_selection`/…) set the
    caret to the edit's natural post-edit position (standard editor behavior), and
  - after **any** edit the selection is **clamped** to `0..=len()` so the caret can
    never dangle past the end of the buffer (a `clamp_selection` helper). Clamping
    is a safety floor (it prevents a later out-of-range slice panic when the view
    reads the caret), not semantic rebasing: an in-range caret is left untouched, so
    the not-auto-rebased contract holds.
- `core` stays `#![forbid(unsafe_code)]`; this is a pure ownership/borrow change,
  no unsafe and no new dependency.
- WASM exposure of the new cursor-intent surface is a **separate** task (H2b) and
  is not part of this decision's implementation.
