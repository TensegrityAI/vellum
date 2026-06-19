//! Cursor and selection: a grapheme-aware caret over a [`TextBuffer`].
//!
//! A [`Selection`] is a pair of byte offsets into a buffer: an `anchor` (the
//! fixed end) and a `head` (the moving end / caret). When `anchor == head` the
//! selection is empty — it is just a caret. This is the standard editor model
//! (the same one the DOM `Selection`/`Range` and CodeMirror use).
//!
//! ## Delegation, not duplication
//!
//! All grapheme- and word-boundary math is **delegated** to the [`TextBuffer`]
//! primitives ([`TextBuffer::prev_grapheme_boundary`],
//! [`TextBuffer::next_grapheme_boundary`], [`TextBuffer::prev_word_boundary`],
//! [`TextBuffer::next_word_boundary`]). The cursor never instantiates a
//! `GraphemeCursor` or calls `unicode-segmentation` itself — it owns only the
//! anchor/head bookkeeping (collapse-vs-extend semantics), the buffer owns the
//! Unicode segmentation (ADR-0001).
//!
//! ## Clamping contract
//!
//! Movement clamps at the buffer ends: the head never goes below `0` or above
//! the buffer byte length. This falls out of the buffer primitives, which
//! already clamp. Concretely, moving left at offset `0` is a no-op, and moving
//! right at the buffer end is a no-op.
//!
//! ## Collapse semantics
//!
//! [`move_left`](Selection::move_left) / [`move_right`](Selection::move_right)
//! are *collapse-or-move*: on a **non-empty** selection they collapse to the
//! near edge (left-arrow → [`start`](Selection::start), right-arrow →
//! [`end`](Selection::end)) without moving past it — the standard editor
//! behavior. On an **empty** selection (a caret) they step the caret one
//! grapheme. The `extend_*` variants instead move only the head, growing or
//! shrinking the selection while the anchor stays put.

use crate::buffer::TextBuffer;
use crate::offset::{ByteOffset, ByteRange};
use std::ops::Range;

/// A text selection: an `anchor` (fixed end) and a `head` (moving caret).
///
/// `anchor == head` means an empty selection (a bare caret). All offsets are
/// **byte** offsets into the buffer the movement methods are called against.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Selection {
    /// The fixed end of the selection.
    ///
    /// `pub(crate)` (F/G/H audit, A4): the [`Document`](crate::Document) aggregate
    /// owns the clamp-to-char-boundary invariant and reaches this field directly
    /// (disjoint-field borrows, ADR-0008). External consumers read it via
    /// [`anchor`](Self::anchor) and mutate only through the aggregate's intent
    /// methods, so the invariant cannot be bypassed from outside `core`.
    pub(crate) anchor: ByteOffset,
    /// The moving end (the caret). `pub(crate)` for the same reason as
    /// [`anchor`](Self::anchor); read it via [`head`](Self::head).
    pub(crate) head: ByteOffset,
}

impl Selection {
    /// Construct a selection from an explicit `anchor` and `head`.
    ///
    /// The two need not be ordered: `anchor` may be greater than `head` (a
    /// "reversed" selection). [`start`](Self::start)/[`end`](Self::end)
    /// normalize the order for range operations.
    #[must_use]
    pub const fn new(anchor: ByteOffset, head: ByteOffset) -> Self {
        Self { anchor, head }
    }

    /// Construct an empty selection (a caret) at `at` (`anchor == head`).
    #[must_use]
    pub const fn caret(at: ByteOffset) -> Self {
        Self {
            anchor: at,
            head: at,
        }
    }

    /// The fixed end of the selection (the `anchor`).
    ///
    /// Read accessor for the `pub(crate)` field, so external consumers can
    /// inspect the selection without the field being publicly mutable.
    #[must_use]
    pub const fn anchor(&self) -> ByteOffset {
        self.anchor
    }

    /// The moving end of the selection (the `head` / caret).
    ///
    /// Read accessor for the `pub(crate)` field (see [`anchor`](Self::anchor)).
    #[must_use]
    pub const fn head(&self) -> ByteOffset {
        self.head
    }

    /// Whether the selection is empty (just a caret, `anchor == head`).
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.anchor.get() == self.head.get()
    }

    /// The lower (earlier) of `anchor` and `head` — the normalized range start.
    #[must_use]
    pub fn start(&self) -> ByteOffset {
        ByteOffset::new(self.anchor.get().min(self.head.get()))
    }

    /// The higher (later) of `anchor` and `head` — the normalized range end.
    #[must_use]
    pub fn end(&self) -> ByteOffset {
        ByteOffset::new(self.anchor.get().max(self.head.get()))
    }

    /// The selection as a byte `Range`, normalized (`start..end`).
    ///
    /// Useful directly for render or any consumer that wants the raw ordered
    /// `usize` range regardless of selection direction. For the typed mutation
    /// path (`doc.delete(...)`) prefer [`byte_range`](Self::byte_range), which
    /// returns a [`ByteRange`] the aggregate front door accepts without a cast.
    #[must_use]
    pub fn range(&self) -> Range<usize> {
        self.start().get()..self.end().get()
    }

    /// The selection as an **ordered** [`ByteRange`] (`start..end`).
    ///
    /// The typed counterpart of [`range`](Self::range): it normalizes a reversed
    /// selection (`anchor > head`) and yields the newtype span so the editor flow
    /// can do `doc.delete(sel.byte_range())` end-to-end in byte space, with no
    /// bare `usize` range in sight. Always ordered, so it never trips
    /// [`Document::delete`](crate::Document::delete)'s inverted-range panic.
    #[must_use]
    pub fn byte_range(&self) -> ByteRange {
        ByteRange::new(self.start(), self.end())
    }

    /// Drop the selection, keeping the caret at `head` (`anchor = head`).
    pub fn collapse(&mut self) {
        self.anchor = self.head;
    }

    /// Move the caret one grapheme left (collapse-or-move).
    ///
    /// If the selection is non-empty, collapse to [`start`](Self::start) (the
    /// left edge) without stepping past it. Otherwise step the caret one
    /// grapheme left via [`TextBuffer::prev_grapheme_boundary`]. Either way the
    /// result is an empty selection (anchor == head). No-op at offset `0`.
    pub fn move_left(&mut self, buf: &TextBuffer) {
        if self.is_empty() {
            self.head = buf.prev_grapheme_boundary(self.head);
        } else {
            self.head = self.start();
        }
        self.collapse();
    }

    /// Move the caret one grapheme right (collapse-or-move).
    ///
    /// The mirror of [`move_left`](Self::move_left): on a non-empty selection
    /// collapse to [`end`](Self::end) (the right edge); otherwise step one
    /// grapheme right via [`TextBuffer::next_grapheme_boundary`]. No-op at the
    /// buffer end.
    pub fn move_right(&mut self, buf: &TextBuffer) {
        if self.is_empty() {
            self.head = buf.next_grapheme_boundary(self.head);
        } else {
            self.head = self.end();
        }
        self.collapse();
    }

    /// Extend the selection one grapheme left: move only `head`, keep `anchor`.
    ///
    /// Grows or shrinks the selection depending on direction. No-op at `0`.
    pub fn extend_left(&mut self, buf: &TextBuffer) {
        self.head = buf.prev_grapheme_boundary(self.head);
    }

    /// Extend the selection one grapheme right: move only `head`, keep `anchor`.
    ///
    /// The mirror of [`extend_left`](Self::extend_left). No-op at the buffer end.
    pub fn extend_right(&mut self, buf: &TextBuffer) {
        self.head = buf.next_grapheme_boundary(self.head);
    }

    /// Move the caret to the previous word boundary, collapsing the selection.
    ///
    /// Delegates to [`TextBuffer::prev_word_boundary`] (UAX #29 segment edges;
    /// see that method for the boundary convention). The result is an empty
    /// selection at the boundary. No-op at offset `0`.
    pub fn move_word_left(&mut self, buf: &TextBuffer) {
        self.head = buf.prev_word_boundary(self.head);
        self.collapse();
    }

    /// Move the caret to the next word boundary, collapsing the selection.
    ///
    /// Delegates to [`TextBuffer::next_word_boundary`]. No-op at the buffer end.
    pub fn move_word_right(&mut self, buf: &TextBuffer) {
        self.head = buf.next_word_boundary(self.head);
        self.collapse();
    }

    /// Extend the selection to the previous word boundary (move only `head`).
    pub fn extend_word_left(&mut self, buf: &TextBuffer) {
        self.head = buf.prev_word_boundary(self.head);
    }

    /// Extend the selection to the next word boundary (move only `head`).
    pub fn extend_word_right(&mut self, buf: &TextBuffer) {
        self.head = buf.next_word_boundary(self.head);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::buffer::TextBuffer;
    use crate::offset::ByteOffset;

    #[test]
    fn move_right_across_emoji_moves_one_grapheme() {
        // "a👨‍👩‍👧b": the ZWJ family is ONE grapheme. From just after 'a' (byte 1),
        // move_right must skip the WHOLE cluster and land right before 'b'.
        let buf = TextBuffer::from_str("a👨‍👩‍👧b");
        let cluster_len = "👨‍👩‍👧".len();
        let mut sel = Selection::caret(ByteOffset(1));
        sel.move_right(&buf);
        assert_eq!(sel.head, ByteOffset(1 + cluster_len));
        assert!(sel.is_empty());
        // Sanity: it jumped by the full multi-char cluster, not one char.
        assert!(cluster_len > 4);
    }

    #[test]
    fn move_left_across_emoji_moves_one_grapheme() {
        // Symmetric: from just before 'b', move_left lands right after 'a'.
        let buf = TextBuffer::from_str("a👨‍👩‍👧b");
        let cluster_len = "👨‍👩‍👧".len();
        let mut sel = Selection::caret(ByteOffset(1 + cluster_len));
        sel.move_left(&buf);
        assert_eq!(sel.head, ByteOffset(1));
        assert!(sel.is_empty());
    }

    #[test]
    fn move_right_over_cjk() {
        // "日本語": each CJK char is 3 bytes; each move_right advances one char.
        let buf = TextBuffer::from_str("日本語");
        let mut sel = Selection::caret(ByteOffset(0));
        sel.move_right(&buf);
        assert_eq!(sel.head, ByteOffset(3));
        sel.move_right(&buf);
        assert_eq!(sel.head, ByteOffset(6));
        sel.move_right(&buf);
        assert_eq!(sel.head, ByteOffset(9));
    }

    #[test]
    fn extend_right_grows_selection_keeping_anchor() {
        // Caret at 0; extend_right twice over ASCII → anchor 0, head 2.
        let buf = TextBuffer::from_str("abcd");
        let mut sel = Selection::caret(ByteOffset(0));
        sel.extend_right(&buf);
        sel.extend_right(&buf);
        assert_eq!(sel.anchor, ByteOffset(0));
        assert_eq!(sel.head, ByteOffset(2));
        assert!(!sel.is_empty());
        assert_eq!(sel.range(), 0..2);
    }

    #[test]
    fn extend_left_grows_selection_keeping_anchor() {
        // Caret at the end of "abc" (byte 3); extend_left twice grows the
        // selection leftward while the anchor stays pinned at 3.
        let buf = TextBuffer::from_str("abc");
        let mut sel = Selection::caret(ByteOffset(3));
        sel.extend_left(&buf);
        sel.extend_left(&buf);
        assert_eq!(sel.anchor, ByteOffset(3));
        assert_eq!(sel.head, ByteOffset(1));
        assert!(!sel.is_empty());
        assert_eq!(sel.range(), 1..3);
    }

    #[test]
    fn extend_left_can_shrink_then_cross_anchor() {
        // Start with a forward selection 2..4 (anchor 2, head 4). extend_left
        // shrinks the head back toward the anchor, then crosses below it: the
        // head ends up < anchor and start/end re-normalize to the new order.
        let buf = TextBuffer::from_str("abcdef");
        let mut sel = Selection::new(ByteOffset(2), ByteOffset(4));
        sel.extend_left(&buf); // head 4 -> 3
        sel.extend_left(&buf); // head 3 -> 2 (now empty, at anchor)
        sel.extend_left(&buf); // head 2 -> 1 (crosses the anchor)
        assert_eq!(sel.anchor, ByteOffset(2));
        assert_eq!(sel.head, ByteOffset(1));
        assert!(sel.head < sel.anchor);
        assert_eq!(sel.start(), ByteOffset(1));
        assert_eq!(sel.end(), ByteOffset(2));
    }

    #[test]
    fn extend_word_right_extends_selection_by_word() {
        // "foo bar": caret at 0; extend_word_right moves only the head to the
        // first segment edge after 0 (byte 3, end of "foo"), anchor stays 0.
        let buf = TextBuffer::from_str("foo bar");
        let mut sel = Selection::caret(ByteOffset(0));
        sel.extend_word_right(&buf);
        assert_eq!(sel.anchor, ByteOffset(0));
        assert_eq!(sel.head, ByteOffset(3));
        assert!(!sel.is_empty());
    }

    #[test]
    fn extend_word_left_from_end_extends_by_word() {
        // "foo bar": caret at 7; extend_word_left moves only the head left by a
        // word boundary (to byte 4, start of "bar"), anchor stays pinned at 7.
        let buf = TextBuffer::from_str("foo bar");
        let mut sel = Selection::caret(ByteOffset(7));
        sel.extend_word_left(&buf);
        assert_eq!(sel.anchor, ByteOffset(7));
        assert_eq!(sel.head, ByteOffset(4));
        assert!(!sel.is_empty());
    }

    #[test]
    fn extend_left_at_start_is_noop() {
        // At byte 0 there is nowhere left to go: the head stays at 0.
        let buf = TextBuffer::from_str("abc");
        let mut sel = Selection::caret(ByteOffset(0));
        sel.extend_left(&buf);
        assert_eq!(sel.head, ByteOffset(0));
        assert_eq!(sel.anchor, ByteOffset(0));
    }

    #[test]
    fn extend_word_left_at_start_is_noop() {
        // At byte 0 the word boundary clamps: the head stays at 0.
        let buf = TextBuffer::from_str("foo bar");
        let mut sel = Selection::caret(ByteOffset(0));
        sel.extend_word_left(&buf);
        assert_eq!(sel.head, ByteOffset(0));
        assert_eq!(sel.anchor, ByteOffset(0));
    }

    #[test]
    fn move_left_on_nonempty_selection_collapses_to_start() {
        // Selection 2..5; left-arrow collapses to the LEFT edge (2).
        let buf = TextBuffer::from_str("abcdefg");
        let mut sel = Selection::new(ByteOffset(2), ByteOffset(5));
        sel.move_left(&buf);
        assert_eq!(sel, Selection::caret(ByteOffset(2)));
    }

    #[test]
    fn move_right_on_nonempty_selection_collapses_to_end() {
        // Fresh selection 2..5; right-arrow collapses to the RIGHT edge (5).
        let buf = TextBuffer::from_str("abcdefg");
        let mut sel = Selection::new(ByteOffset(2), ByteOffset(5));
        sel.move_right(&buf);
        assert_eq!(sel, Selection::caret(ByteOffset(5)));
    }

    #[test]
    fn move_left_at_start_is_noop() {
        let buf = TextBuffer::from_str("abc");
        let mut sel = Selection::caret(ByteOffset(0));
        sel.move_left(&buf);
        assert_eq!(sel, Selection::caret(ByteOffset(0)));
    }

    #[test]
    fn move_right_at_end_is_noop() {
        let buf = TextBuffer::from_str("abc");
        let mut sel = Selection::caret(ByteOffset(3));
        sel.move_right(&buf);
        assert_eq!(sel, Selection::caret(ByteOffset(3)));
    }

    #[test]
    fn start_end_normalize_reversed_selection() {
        // anchor > head: start/end/range must normalize the order.
        let sel = Selection::new(ByteOffset(5), ByteOffset(2));
        assert_eq!(sel.start(), ByteOffset(2));
        assert_eq!(sel.end(), ByteOffset(5));
        assert_eq!(sel.range(), 2..5);
        assert!(!sel.is_empty());
    }

    #[test]
    fn byte_range_is_ordered_even_for_a_reversed_selection() {
        // A reversed selection (anchor > head) must still yield an ordered
        // ByteRange so `doc.delete(sel.byte_range())` never hits the
        // inverted-range panic.
        use crate::offset::ByteRange;
        let sel = Selection::new(ByteOffset(5), ByteOffset(2));
        assert_eq!(
            sel.byte_range(),
            ByteRange::new(ByteOffset(2), ByteOffset(5)),
        );
        // And matches the raw range() for the common forward case too.
        let fwd = Selection::new(ByteOffset(1), ByteOffset(4));
        assert_eq!(fwd.byte_range().get(), fwd.range());
    }

    #[test]
    fn move_word_right_jumps_to_next_word_boundary() {
        // "foo bar": from caret 0, move_word_right lands on the first segment END
        // after 0, which is byte 3 (end of "foo"). See prev/next_word_boundary's
        // documented split_word_bound_indices convention.
        let buf = TextBuffer::from_str("foo bar");
        let mut sel = Selection::caret(ByteOffset(0));
        sel.move_word_right(&buf);
        assert_eq!(sel.head, ByteOffset(3));
        assert!(sel.is_empty());
    }

    #[test]
    fn move_word_left_from_end() {
        // From the end of "foo bar" (7), move_word_left lands at the start of the
        // final word "bar" (byte 4).
        let buf = TextBuffer::from_str("foo bar");
        let mut sel = Selection::caret(ByteOffset(7));
        sel.move_word_left(&buf);
        assert_eq!(sel.head, ByteOffset(4));
        assert!(sel.is_empty());
    }

    #[test]
    fn caret_is_empty_and_collapse_drops_selection() {
        let buf = TextBuffer::from_str("abcd");
        let caret = Selection::caret(ByteOffset(2));
        assert!(caret.is_empty());

        let mut sel = Selection::new(ByteOffset(1), ByteOffset(3));
        sel.collapse();
        assert!(sel.is_empty());
        assert_eq!(sel.head, ByteOffset(3));
        assert_eq!(sel.anchor, ByteOffset(3));
        // collapse keeps the head, no buffer needed (buf unused on purpose).
        let _ = buf;
    }
}
