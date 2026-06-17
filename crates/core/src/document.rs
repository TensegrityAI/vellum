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
use crate::event::EditEvent;
use crate::offset::{ByteOffset, ByteRange};

/// An event-sourced text document with undo/redo.
///
/// Wraps a [`TextBuffer`] plus an `undo`/`redo` history. Every public mutation
/// ([`insert`](Self::insert), [`delete`](Self::delete)) records the inverse of the
/// applied edit so the change can be reversed; [`undo`](Self::undo) and
/// [`redo`](Self::redo) walk those stacks (ADR-0002).
#[derive(Debug, Default, Clone)]
pub struct Document {
    buffer: TextBuffer,
    /// Each entry is the INVERSE of an applied edit (most recent on top).
    undo: Vec<EditEvent>,
    /// Inverses of undone edits, ready to be re-applied (most recent on top).
    redo: Vec<EditEvent>,
}

impl Document {
    /// Create an empty document with empty history.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a document seeded with `s` and empty history.
    ///
    /// Named `from_str` to mirror [`TextBuffer::from_str`]; this is infallible
    /// construction, not the fallible `std::str::FromStr` contract.
    #[allow(clippy::should_implement_trait)]
    #[must_use]
    pub fn from_str(s: &str) -> Self {
        Self {
            buffer: TextBuffer::from_str(s),
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

    /// Apply a forward edit and record its inverse, clearing the redo branch.
    fn commit(&mut self, event: EditEvent) {
        self.buffer.apply(&event);
        self.undo.push(event.inverse());
        self.redo.clear();
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
