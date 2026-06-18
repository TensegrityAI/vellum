//! The [`Document`] aggregate: an event-sourced buffer with undo/redo.
//!
//! A `Document` wraps a [`TextBuffer`] and two stacks of [`EditEvent`]s. It is
//! the **write side** of the editor: all mutations go through [`Document::insert`]
//! and [`Document::delete`], which build the corresponding [`EditEvent`], apply it,
//! and record its inverse on the undo stack. Undo/redo is then pure
//! reverse/replay of those events (ADR-0002), not a separate ad-hoc mechanism.
//!
//! ## The undo/redo bookkeeping
//!
//! Each entry on the `undo` stack is the **inverse** of an applied edit. On a new
//! edit, the redo stack is cleared (a fresh branch of history invalidates the
//! redo future). `inverse` is an involution ([`EditEvent::inverse`]), so the dance
//! is symmetric:
//!
//! - **edit `E`**: apply `E`; push `E.inverse()` to `undo`; clear `redo`.
//! - **undo**: pop `inv` from `undo`; apply `inv`; push `inv.inverse()` to `redo`
//!   (`inv.inverse()` reconstructs the original forward edit).
//! - **redo**: pop `fwd` from `redo`; apply `fwd`; push `fwd.inverse()` to `undo`.
//!
//! This guarantees `type → undo → redo` restores the typed text.
//!
//! ## `Deleted` events are well-formed by construction (F4 review invariant)
//!
//! A `Deleted { at, removed }` event whose `removed` text does not match the bytes
//! actually at `at` would silently corrupt the round-trip on inversion. The
//! `Document` is the **only** producer of `Deleted` events, so [`Document::delete`]
//! takes a byte range only and **reads `removed` out of the buffer itself** before
//! applying — it never accepts a caller-supplied `removed`. This makes every
//! `Deleted` event well-formed by construction.

use crate::buffer::TextBuffer;
use crate::cursor::Selection;
use crate::event::EditEvent;
use crate::offset::{ByteOffset, ByteRange};

/// An event-sourced text document with undo/redo.
///
/// Wraps a [`TextBuffer`] plus an `undo`/`redo` history. Every public mutation
/// ([`insert`](Self::insert), [`delete`](Self::delete)) records the inverse of the
/// applied edit so the change can be reversed; [`undo`](Self::undo) and
/// [`redo`](Self::redo) walk those stacks (ADR-0002).
#[derive(Debug, Clone)]
pub struct Document {
    buffer: TextBuffer,
    /// The single text [`Selection`] (caret) owned by this document. Defaults to
    /// a caret at byte `0`. The aggregate is the navigation entry point: cursor
    /// movement goes through the intent methods on `Document`, which delegate to
    /// `Selection` using **disjoint field borrows** (`&self.buffer`, not the
    /// `buffer()` getter — see ADR-0008). Inc 1 holds exactly one selection;
    /// multi-cursor is deferred.
    selection: Selection,
    /// Each entry is the INVERSE of an applied edit (most recent on top).
    undo: Vec<EditEvent>,
    /// Inverses of undone edits, ready to be re-applied (most recent on top).
    redo: Vec<EditEvent>,
}

impl Default for Document {
    fn default() -> Self {
        Self {
            buffer: TextBuffer::default(),
            selection: Selection::caret(ByteOffset::new(0)),
            undo: Vec::new(),
            redo: Vec::new(),
        }
    }
}

impl Document {
    /// Create an empty document with empty history and a caret at byte `0`.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a document seeded with `s` and empty history.
    ///
    /// Named `from_str` to mirror [`TextBuffer::from_str`]; this is infallible
    /// construction, not the fallible `std::str::FromStr` contract. The caret
    /// starts at byte `0`.
    #[allow(clippy::should_implement_trait)]
    #[must_use]
    pub fn from_str(s: &str) -> Self {
        Self {
            buffer: TextBuffer::from_str(s),
            selection: Selection::caret(ByteOffset::new(0)),
            undo: Vec::new(),
            redo: Vec::new(),
        }
    }

    /// The document contents as an owned `String`.
    #[must_use]
    pub fn text(&self) -> String {
        self.buffer.text()
    }

    /// Read access to the underlying buffer (for tokenize / offset conversions /
    /// cursor positioning). Mutation must go through [`insert`](Self::insert) and
    /// [`delete`](Self::delete) so history stays consistent.
    #[must_use]
    pub fn buffer(&self) -> &TextBuffer {
        &self.buffer
    }

    /// Length of the document in **bytes** (UTF-8).
    #[must_use]
    pub fn len(&self) -> usize {
        self.buffer.len()
    }

    /// Number of Unicode scalar values (chars) in the document.
    #[must_use]
    pub fn char_len(&self) -> usize {
        self.buffer.char_len()
    }

    /// Whether the document is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.buffer.is_empty()
    }

    /// Insert `text` at byte offset `at`, recording history.
    ///
    /// Builds `Inserted { at, text }`, applies it, pushes its inverse onto the
    /// undo stack, and clears the redo stack. Panics if `at` is not a char
    /// boundary or is out of bounds (delegates to the buffer's panic contract).
    pub fn insert(&mut self, at: ByteOffset, text: &str) {
        let event = EditEvent::Inserted {
            at,
            text: text.to_string(),
        };
        self.commit(event);
    }

    /// Delete the byte `range`, recording history.
    ///
    /// The `range` is a [`ByteRange`] (a pair of [`ByteOffset`]s), not a bare
    /// `Range<usize>`: this is the aggregate **front door**, and the newtype makes
    /// it impossible to hand it a `char`- or UTF-16-indexed range by mistake (I1
    /// hardening). The raw `usize` range is recovered only at the slice boundary
    /// below, via [`ByteRange::get`].
    ///
    /// **The `removed` text is read out of the buffer here**, never accepted from
    /// the caller: this is the F4-review invariant that makes the resulting
    /// `Deleted` event well-formed by construction (see module docs). We slice the
    /// current text by the byte range — `text()` returns an owned `String`, so we
    /// slice it and `.to_string()` the removed run.
    ///
    /// **Panic contract (unchanged from the previous `Range<usize>` signature):**
    /// the byte range is validated by the **`str` slice** at the point of capture
    /// below, with standard-library semantics — it panics if either bound is not on
    /// a UTF-8 char boundary, if a bound is out of range, **or if
    /// `range.start > range.end`** (inverted range). This method does **not**
    /// silently normalize an inverted `ByteRange` (no implicit
    /// [`ordered`](ByteRange::ordered)); an inverted range still panics, exactly as
    /// before, so the behavior and H1's contract note are unchanged. Callers with a
    /// possibly-reversed span (e.g. a selection) must normalize first — see
    /// [`Selection::byte_range`](crate::Selection::byte_range), which returns an
    /// ordered range. These three panic shapes and their stdlib messages differ
    /// from `TextBuffer::delete`'s own contract (the slice short-circuits before the
    /// buffer is ever touched). Task H1, which converts these into a `Result` at the
    /// wasm boundary, must validate against the *slice* semantics here, not the
    /// buffer's.
    pub fn delete(&mut self, range: ByteRange) {
        // Capture the exact bytes being removed FROM the buffer (never caller-
        // supplied) so the Deleted event's `removed` matches what is at `at`.
        // TODO(inc1+): `text()` allocates the whole rope to slice one run (O(n)
        // per delete). Add a `TextBuffer::slice(range) -> Cow<str>` over rope
        // chunks to preserve rope locality once documents get large.
        let removed = self.buffer.text()[range.get()].to_string();
        let event = EditEvent::Deleted {
            at: range.start,
            removed,
        };
        self.commit(event);
    }

    // --- Cursor: read + intent (ADR-0008) ---------------------------------

    /// The current [`Selection`] (caret). `Selection` is `Copy`, so this returns
    /// a snapshot the view can read freely; mutation goes through the intent
    /// methods below.
    #[must_use]
    pub fn selection(&self) -> Selection {
        self.selection
    }

    /// Replace the whole selection (e.g. the view pushing a drag-select).
    ///
    /// Clamped to `0..=len()` so an out-of-range span from the view can never
    /// leave a dangling caret.
    pub fn set_selection(&mut self, sel: Selection) {
        self.selection = sel;
        self.clamp_selection();
    }

    /// Collapse the selection to a bare caret at `at` (e.g. on a click).
    ///
    /// Clamped to `0..=len()`.
    pub fn set_caret(&mut self, at: ByteOffset) {
        self.selection = Selection::caret(at);
        self.clamp_selection();
    }

    /// Drop the selection, keeping the caret at `head`.
    pub fn collapse_selection(&mut self) {
        self.selection.collapse();
    }

    /// Move the caret one grapheme left (collapse-or-move).
    pub fn move_left(&mut self) {
        // Disjoint field borrows: `&self.buffer` (the FIELD), not `self.buffer()`
        // (the getter, which borrows all of `&self`) — see ADR-0008.
        self.selection.move_left(&self.buffer);
    }

    /// Move the caret one grapheme right (collapse-or-move).
    pub fn move_right(&mut self) {
        self.selection.move_right(&self.buffer);
    }

    /// Extend the selection one grapheme left (move only `head`).
    pub fn extend_left(&mut self) {
        self.selection.extend_left(&self.buffer);
    }

    /// Extend the selection one grapheme right (move only `head`).
    pub fn extend_right(&mut self) {
        self.selection.extend_right(&self.buffer);
    }

    /// Move the caret to the previous word boundary, collapsing the selection.
    pub fn move_word_left(&mut self) {
        self.selection.move_word_left(&self.buffer);
    }

    /// Move the caret to the next word boundary, collapsing the selection.
    pub fn move_word_right(&mut self) {
        self.selection.move_word_right(&self.buffer);
    }

    /// Extend the selection to the previous word boundary (move only `head`).
    pub fn extend_word_left(&mut self) {
        self.selection.extend_word_left(&self.buffer);
    }

    /// Extend the selection to the next word boundary (move only `head`).
    pub fn extend_word_right(&mut self) {
        self.selection.extend_word_right(&self.buffer);
    }

    // --- Selection-aware editing (ADR-0008) -------------------------------

    /// Type `text` at the cursor (standard type-over-selection behavior).
    ///
    /// If the selection is non-empty, the selected range is deleted first
    /// (recorded as its own [`EditEvent`]); then `text` is inserted at the
    /// selection start. Either way the caret collapses to `start + text.len()`.
    ///
    /// **Known UX behavior (inc 1):** typing over a non-empty selection records
    /// **two** history entries — a `Deleted` then an `Inserted` — so it takes
    /// **two** undos to fully revert (one to pull the typed text back out, one to
    /// restore the replaced run). Single-step coalescing of a type-over into one
    /// undoable unit is deferred to a later increment, consistent with the
    /// `TODO(inc1+)` insert-coalescing note below.
    pub fn insert_at_cursor(&mut self, text: &str) {
        // Type-over: remove the selected run first, then insert at the start.
        if !self.selection.is_empty() {
            self.delete(self.selection.byte_range());
        }
        let at = self.selection.start();
        self.insert(at, text);
        // No `clamp_selection` needed: `at` is a clamped char boundary and exactly
        // `text.len()` bytes were inserted there, so `at + text.len()` is itself a
        // char boundary in range (it sits immediately after the inserted run).
        let caret = ByteOffset::new(at.get() + text.len());
        self.selection = Selection::caret(caret);
    }

    /// Delete the current selection, collapsing the caret to its start.
    ///
    /// Returns `true` if a (non-empty) selection was deleted, `false` if the
    /// selection was empty (a bare caret) — in which case the caller decides the
    /// backspace / forward-delete semantics.
    pub fn delete_selection(&mut self) -> bool {
        if self.selection.is_empty() {
            return false;
        }
        let start = self.selection.start();
        self.delete(self.selection.byte_range());
        self.selection = Selection::caret(start);
        true
    }

    /// Backspace: delete the selection if non-empty, else one grapheme left of
    /// the caret. Returns `true` if anything was removed.
    pub fn backspace(&mut self) -> bool {
        if self.delete_selection() {
            return true;
        }
        let head = self.selection.head;
        let prev = self.buffer.prev_grapheme_boundary(head);
        if prev == head {
            return false; // at the start, nothing to remove
        }
        self.delete(ByteRange::new(prev, head));
        self.selection = Selection::caret(prev);
        true
    }

    /// Forward-delete: delete the selection if non-empty, else one grapheme to
    /// the right of the caret. Returns `true` if anything was removed.
    pub fn delete_forward(&mut self) -> bool {
        if self.delete_selection() {
            return true;
        }
        let head = self.selection.head;
        let next = self.buffer.next_grapheme_boundary(head);
        if next == head {
            return false; // at the end, nothing to remove
        }
        self.delete(ByteRange::new(head, next));
        // Caret stays at `head` (the deletion removed text to its right, so the
        // offset itself does not move). The `delete` above already ran
        // `clamp_selection` via `commit`, so the caret is guaranteed in range and
        // on a char boundary; no extra guard is needed here.
        true
    }

    /// Snap the selection's `anchor` and `head` so each is **in range AND on a
    /// UTF-8 char boundary**.
    ///
    /// A safety floor (ADR-0008): after any edit the caret must not dangle past
    /// the buffer end *and* must not point mid-codepoint. An edit that shifts a
    /// multibyte character under a previously-valid caret can leave the stored
    /// offset interior to that codepoint; the next grapheme step would then panic
    /// inside `GraphemeCursor` (a latent WASM trap). Snapping each end via
    /// [`TextBuffer::floor_char_boundary`] subsumes the old magnitude clamp (it
    /// floors to `min(byte, len())` first), so after this the internal caret is
    /// **always** safe to feed to the grapheme primitives.
    ///
    /// This is **not** semantic rebasing — a caret that is already in range and on
    /// a boundary is left untouched, so the Inc-1 "offsets are not auto-rebased"
    /// contract still holds.
    fn clamp_selection(&mut self) {
        let snap = |b: ByteOffset| ByteOffset::new(self.buffer.floor_char_boundary(b.get()));
        self.selection.anchor = snap(self.selection.anchor);
        self.selection.head = snap(self.selection.head);
    }

    /// Apply a forward edit and record its inverse, clearing the redo branch.
    fn commit(&mut self, event: EditEvent) {
        self.buffer.apply(&event);
        self.undo.push(event.inverse());
        self.redo.clear();
        self.clamp_selection();
    }

    // TODO(inc1+): optional coalescing of consecutive single-char inserts.

    /// Undo the most recent edit. Returns `true` if an edit was undone, `false`
    /// if there was nothing to undo.
    ///
    /// Pops the inverse `inv` of the last edit, applies it, and pushes
    /// `inv.inverse()` (the reconstructed forward edit) onto the redo stack.
    pub fn undo(&mut self) -> bool {
        let Some(inv) = self.undo.pop() else {
            return false;
        };
        self.buffer.apply(&inv);
        self.redo.push(inv.inverse());
        self.clamp_selection();
        true
    }

    /// Redo the most recently undone edit. Returns `true` if an edit was redone,
    /// `false` if there was nothing to redo.
    ///
    /// Symmetric to [`undo`](Self::undo): pops the forward edit `fwd`, applies it,
    /// and pushes `fwd.inverse()` back onto the undo stack.
    pub fn redo(&mut self) -> bool {
        let Some(fwd) = self.redo.pop() else {
            return false;
        };
        self.buffer.apply(&fwd);
        self.undo.push(fwd.inverse());
        self.clamp_selection();
        true
    }

    /// Whether there is an edit available to [`undo`](Self::undo).
    #[must_use]
    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    /// Whether there is an edit available to [`redo`](Self::redo).
    #[must_use]
    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::offset::{ByteOffset, ByteRange};

    /// Convenience for tests: build a `ByteRange` from two raw byte indices.
    fn br(start: usize, end: usize) -> ByteRange {
        ByteRange::new(ByteOffset::new(start), ByteOffset::new(end))
    }

    #[test]
    fn insert_records_undo_and_undo_restores() {
        // Arrange: empty document.
        let mut doc = Document::new();
        // Act: insert "hello" at the start.
        doc.insert(ByteOffset::new(0), "hello");
        // Assert: text reflects the insert; undo reverses it.
        assert_eq!(doc.text(), "hello");
        assert!(doc.undo());
        assert_eq!(doc.text(), "");
    }

    #[test]
    fn undo_then_redo_restores_text() {
        // Arrange.
        let mut doc = Document::new();
        doc.insert(ByteOffset::new(0), "hello");
        // Act + Assert: undo clears, redo restores the typed text.
        assert!(doc.undo());
        assert_eq!(doc.text(), "");
        assert!(doc.redo());
        assert_eq!(doc.text(), "hello");
    }

    #[test]
    fn delete_captures_removed_from_buffer_and_undo_restores() {
        // Arrange: "Hello world".
        let mut doc = Document::from_str("Hello world");
        // Act: delete " world" (bytes 5..11) — caller passes only the range.
        doc.delete(br(5, 11));
        // Assert: deletion took effect and undo restores byte-exactly, proving
        // `removed` was captured from the buffer (not from the caller).
        assert_eq!(doc.text(), "Hello");
        assert!(doc.undo());
        assert_eq!(doc.text(), "Hello world");
    }

    #[test]
    fn undo_on_empty_history_is_noop() {
        // Arrange: fresh document.
        let mut doc = Document::from_str("abc");
        // Act + Assert: nothing to undo or redo; text unchanged.
        assert!(!doc.undo());
        assert_eq!(doc.text(), "abc");
        assert!(!doc.redo());
        assert_eq!(doc.text(), "abc");
    }

    #[test]
    fn new_edit_clears_redo() {
        // Arrange: insert, undo (so redo is now available).
        let mut doc = Document::new();
        doc.insert(ByteOffset::new(0), "a");
        assert!(doc.undo());
        // Act: a NEW edit after the undo must clear the redo branch.
        doc.insert(ByteOffset::new(0), "b");
        // Assert: redo is a no-op and the new text stands.
        assert!(!doc.redo());
        assert_eq!(doc.text(), "b");
    }

    #[test]
    fn multiple_edits_undo_in_reverse_order() {
        // Arrange: type "a", "b", "c" each at the end.
        let mut doc = Document::new();
        doc.insert(ByteOffset::new(0), "a");
        doc.insert(ByteOffset::new(1), "b");
        doc.insert(ByteOffset::new(2), "c");
        assert_eq!(doc.text(), "abc");
        // Act + Assert: three undos peel "c", "b", "a".
        assert!(doc.undo());
        assert_eq!(doc.text(), "ab");
        assert!(doc.undo());
        assert_eq!(doc.text(), "a");
        assert!(doc.undo());
        assert_eq!(doc.text(), "");
    }

    #[test]
    fn multibyte_delete_undo_restores() {
        // Arrange: "x😀y" — "😀" occupies byte range 1..5 (4 UTF-8 bytes).
        let mut doc = Document::from_str("x😀y");
        // Act: delete the emoji cluster by byte range.
        doc.delete(br(1, 5));
        assert_eq!(doc.text(), "xy");
        // Assert: undo restores the multibyte cluster byte-exactly.
        assert!(doc.undo());
        assert_eq!(doc.text(), "x😀y");
    }

    #[test]
    fn interleaved_edit_undo_edit_redo_sequence() {
        // Exercise the redo-rebuild / redo-clearing path across interleaved
        // edits and undos. From "": type "a", type "b", undo, type "c" (which
        // clears the redo branch), then undo back to empty.
        let mut doc = Document::new();
        doc.insert(ByteOffset::new(0), "a");
        assert_eq!(doc.text(), "a");
        doc.insert(ByteOffset::new(1), "b");
        assert_eq!(doc.text(), "ab");
        assert!(doc.undo());
        assert_eq!(doc.text(), "a");
        // A new edit after the undo clears the redo future.
        doc.insert(ByteOffset::new(1), "c");
        assert_eq!(doc.text(), "ac");
        assert!(!doc.redo());
        assert_eq!(doc.text(), "ac");
        // Walk the undo stack back to empty.
        assert!(doc.undo());
        assert_eq!(doc.text(), "a");
        assert!(doc.undo());
        assert_eq!(doc.text(), "");
        assert!(!doc.can_undo());
    }

    #[test]
    fn redo_after_partial_undo_replays_in_order() {
        // Type "a","b","c" each at the end; undo twice back to "a"; redo twice
        // forward to "abc". The replay must restore the edits in order.
        let mut doc = Document::new();
        doc.insert(ByteOffset::new(0), "a");
        doc.insert(ByteOffset::new(1), "b");
        doc.insert(ByteOffset::new(2), "c");
        assert_eq!(doc.text(), "abc");
        assert!(doc.undo());
        assert_eq!(doc.text(), "ab");
        assert!(doc.undo());
        assert_eq!(doc.text(), "a");
        assert!(doc.redo());
        assert_eq!(doc.text(), "ab");
        assert!(doc.redo());
        assert_eq!(doc.text(), "abc");
    }

    /// Pins the Increment-1 contract that `core` does **not** auto-rebase a
    /// [`Selection`](crate::Selection) after a [`Document`] edit. This is by
    /// design for Inc 1: a `Selection` is an immutable byte-offset pair that
    /// knows nothing about subsequent document mutations. Rebasing cursor offsets
    /// against an edit (so a caret after an upstream insert shifts forward) is the
    /// responsibility of the **view layer** (Task I5; H2 owns the typed mutation
    /// front door at the JS boundary), not of `core`. This test exists to catch a
    /// regression where someone wires implicit rebasing into the aggregate — that
    /// would contradict the audit and the Inc-1 contract.
    #[test]
    fn cursor_offsets_are_not_rebased_after_edit_inc1_contract() {
        use crate::cursor::Selection;
        // Arrange: doc "hello" with a caret at byte 5 (the end).
        let mut doc = Document::from_str("hello");
        let sel = Selection::caret(ByteOffset::new(5));
        // Act: insert "XX" at the very start, shifting all later text right by 2.
        doc.insert(ByteOffset::new(0), "XX");
        assert_eq!(doc.text(), "XXhello");
        // Assert: the Selection is UNCHANGED — head is still byte 5, NOT rebased
        // to 7. Core does not auto-shift offsets; the view layer (I5) owns that.
        assert_eq!(sel.head, ByteOffset::new(5));
        assert_eq!(sel.anchor, ByteOffset::new(5));
    }

    // --- H2a: cursor in the aggregate -------------------------------------

    #[test]
    fn move_right_advances_cursor_through_document() {
        use crate::cursor::Selection;
        // "ab😀c": 😀 is 4 UTF-8 bytes at 2..6. Stepping right from caret 0 must
        // land on grapheme boundaries: a(1), b(2), 😀(6), c(7).
        let mut doc = Document::from_str("ab😀c");
        assert_eq!(doc.selection(), Selection::caret(ByteOffset::new(0)));
        doc.move_right();
        assert_eq!(doc.selection().head, ByteOffset::new(1));
        doc.move_right();
        assert_eq!(doc.selection().head, ByteOffset::new(2));
        doc.move_right();
        assert_eq!(doc.selection().head, ByteOffset::new(6)); // skipped the emoji
        doc.move_right();
        assert_eq!(doc.selection().head, ByteOffset::new(7));
        // No-op at the end.
        doc.move_right();
        assert_eq!(doc.selection().head, ByteOffset::new(7));
    }

    #[test]
    fn insert_at_cursor_types_at_caret_and_advances() {
        // "ac", caret at 1, type "b" → "abc", caret collapsed at 2.
        let mut doc = Document::from_str("ac");
        doc.set_caret(ByteOffset::new(1));
        doc.insert_at_cursor("b");
        assert_eq!(doc.text(), "abc");
        assert!(doc.selection().is_empty());
        assert_eq!(doc.selection().head, ByteOffset::new(2));
    }

    #[test]
    fn insert_at_cursor_replaces_nonempty_selection() {
        use crate::cursor::Selection;
        // "aXXc", select "XX" (anchor 1, head 3), type "b" → "abc", caret at 2.
        let mut doc = Document::from_str("aXXc");
        doc.set_selection(Selection::new(ByteOffset::new(1), ByteOffset::new(3)));
        doc.insert_at_cursor("b");
        assert_eq!(doc.text(), "abc");
        assert!(doc.selection().is_empty());
        assert_eq!(doc.selection().head, ByteOffset::new(2));
    }

    #[test]
    fn delete_selection_removes_and_collapses() {
        use crate::cursor::Selection;
        // "abc", select "b" (1..2): delete removes it, collapses caret to 1.
        let mut doc = Document::from_str("abc");
        doc.set_selection(Selection::new(ByteOffset::new(1), ByteOffset::new(2)));
        assert!(doc.delete_selection());
        assert_eq!(doc.text(), "ac");
        assert!(doc.selection().is_empty());
        assert_eq!(doc.selection().head, ByteOffset::new(1));
        // On an empty selection, delete_selection is a no-op returning false.
        assert!(!doc.delete_selection());
        assert_eq!(doc.text(), "ac");
    }

    #[test]
    fn cursor_intent_methods_use_disjoint_borrows() {
        // Smoke test that the aggregate's cursor-intent methods compile (they use
        // `&self.buffer`, not the `buffer()` getter) and behave.
        let mut doc = Document::from_str("abcd");
        doc.move_right(); // 0 -> 1
        doc.move_right(); // 1 -> 2
        assert_eq!(doc.selection().head, ByteOffset::new(2));
        doc.move_left(); // 2 -> 1
        assert_eq!(doc.selection().head, ByteOffset::new(1));
        doc.extend_right(); // head 1 -> 2, anchor stays 1
        assert_eq!(doc.selection().anchor, ByteOffset::new(1));
        assert_eq!(doc.selection().head, ByteOffset::new(2));
        assert!(!doc.selection().is_empty());
        // collapse drops the selection back to a caret at head.
        doc.collapse_selection();
        assert!(doc.selection().is_empty());
        assert_eq!(doc.selection().head, ByteOffset::new(2));
    }

    #[test]
    fn selection_is_clamped_after_shortening_edit() {
        // Caret near the end, then an explicit delete shrinks the buffer below the
        // caret: the aggregate clamps the caret to the new length so it never
        // dangles past the end (a later view read would otherwise slice OOB).
        let mut doc = Document::from_str("abcdef");
        doc.set_caret(ByteOffset::new(6));
        doc.delete(br(2, 6)); // text -> "ab" (len 2), caret was 6
        assert_eq!(doc.text(), "ab");
        assert_eq!(doc.selection().head, ByteOffset::new(2));
        assert_eq!(doc.selection().anchor, ByteOffset::new(2));
    }

    #[test]
    fn insert_at_cursor_then_undo_restores_text() {
        // Cursor-aware edits flow through the same history machinery: insert at the
        // caret, then undo restores the text. (Inc 1 does NOT guarantee undo
        // restores the caret; we assert text only — caret behavior is the view's.)
        let mut doc = Document::from_str("ac");
        doc.set_caret(ByteOffset::new(1));
        doc.insert_at_cursor("b");
        assert_eq!(doc.text(), "abc");
        assert!(doc.undo());
        assert_eq!(doc.text(), "ac");
    }

    #[test]
    fn move_after_edit_that_shifts_multibyte_under_caret_does_not_panic() {
        // CRITICAL regression (H2a review): an edit that shifts a multibyte char
        // under a previously-valid caret used to leave the internal caret pointing
        // MID-CODEPOINT (the old magnitude-only clamp did not snap to a char
        // boundary), so the next grapheme step panicked inside GraphemeCursor —
        // a latent WASM trap.
        //
        // "ab😀": a=0..1, b=1..2, 😀=2..6.
        let mut doc = Document::from_str("ab😀");
        doc.set_caret(ByteOffset::new(2)); // valid boundary, just before 😀
                                           // Remove 'a' (0..1) → "b😀"; 😀 now occupies 1..5. The stored caret 2 is
                                           // now INTERIOR to 😀. `delete` runs clamp_selection, which must snap the
                                           // caret DOWN to byte 1 (the start of 😀).
        doc.delete(br(0, 1));
        assert_eq!(doc.text(), "b😀");
        assert_eq!(doc.selection().head, ByteOffset::new(1)); // snapped to boundary
        assert_eq!(doc.selection().anchor, ByteOffset::new(1));
        // The move that previously panicked: from byte 1 (before 😀) move_right
        // skips the whole emoji and lands at byte 5 (after 😀), at the end.
        doc.move_right();
        assert_eq!(doc.selection().head, ByteOffset::new(5));
        // And a second move is a clean no-op at the end (still no panic).
        doc.move_right();
        assert_eq!(doc.selection().head, ByteOffset::new(5));
    }

    #[test]
    fn insert_at_cursor_replaces_reversed_selection() {
        use crate::cursor::Selection;
        // Regression lock (H2a review): a REVERSED selection (head=1, anchor=3)
        // over "aXXc" must type-over correctly via the ordered byte_range().
        // insert_at_cursor("b") → "abc", caret collapsed at byte 2.
        let mut doc = Document::from_str("aXXc");
        doc.set_selection(Selection::new(ByteOffset::new(3), ByteOffset::new(1)));
        doc.insert_at_cursor("b");
        assert_eq!(doc.text(), "abc");
        assert!(doc.selection().is_empty());
        assert_eq!(doc.selection().head, ByteOffset::new(2));
    }

    #[test]
    fn can_undo_can_redo_reflect_state() {
        // Arrange: fresh document — nothing to undo or redo.
        let mut doc = Document::new();
        assert!(!doc.can_undo());
        assert!(!doc.can_redo());

        // After an edit: can undo, cannot redo.
        doc.insert(ByteOffset::new(0), "hi");
        assert!(doc.can_undo());
        assert!(!doc.can_redo());

        // After undo: cannot undo, can redo.
        assert!(doc.undo());
        assert!(!doc.can_undo());
        assert!(doc.can_redo());

        // After redo: can undo again, cannot redo.
        assert!(doc.redo());
        assert!(doc.can_undo());
        assert!(!doc.can_redo());
    }
}
