//! Offset model: the three coordinate spaces a text editor must reconcile.
//!
//! A single position in a document has three different numeric addresses
//! depending on who is asking:
//!
//! - [`ByteOffset`] — a UTF-8 **byte** index. This is what `core`, `ropey`, and
//!   any byte-oriented tokenizer speak. It is the canonical internal address.
//! - [`CharOffset`] — a Unicode **scalar value** (`char`) index. Useful as an
//!   intermediate when converting, since `ropey` indexes chars natively.
//! - [`Utf16Offset`] — a UTF-16 **code-unit** offset. This is what the DOM,
//!   `<textarea>`, and the `EditContext` API speak. Diff-based input arrives in
//!   this space and **must** be converted before touching the buffer, or the
//!   core traps (Increment 1 blocker #1).
//!
//! These are deliberately distinct newtypes rather than bare `usize`s: mixing a
//! UTF-16 code-unit count into a byte-indexed API is exactly the class of bug
//! this model exists to make unrepresentable. Conversions between the spaces
//! live on [`TextBuffer`](crate::TextBuffer), which owns the text and therefore
//! the only correct mapping.
//!
//! A *span* in byte space is a [`ByteRange`] (a pair of [`ByteOffset`]s) rather
//! than a bare `Range<usize>`, for the same reason: the [`Document`](crate::Document)
//! aggregate's delete API speaks `ByteRange` so a caller cannot accidentally hand
//! it a `char`- or UTF-16-indexed range. The raw `usize` range is recovered only
//! at the storage boundary via [`ByteRange::get`].

use std::ops::Range;

/// A UTF-8 **byte** offset into a buffer.
///
/// The canonical internal address. `core`, `ropey`, and byte-oriented
/// tokenizers all speak this space.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ByteOffset(pub usize);

impl ByteOffset {
    /// Construct a byte offset.
    #[must_use]
    pub const fn new(value: usize) -> Self {
        Self(value)
    }

    /// The underlying `usize`.
    #[must_use]
    pub const fn get(self) -> usize {
        self.0
    }
}

/// A Unicode scalar value (`char`) offset into a buffer.
///
/// `ropey` indexes by char natively, so this is the natural pivot space when
/// converting between bytes and UTF-16 code units.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CharOffset(pub usize);

impl CharOffset {
    /// Construct a char offset.
    #[must_use]
    pub const fn new(value: usize) -> Self {
        Self(value)
    }

    /// The underlying `usize`.
    #[must_use]
    pub const fn get(self) -> usize {
        self.0
    }
}

/// A UTF-16 **code-unit** offset into a buffer.
///
/// The space the DOM, `<textarea>`, and `EditContext` speak. Astral-plane
/// scalar values cost two UTF-16 code units (a surrogate pair) but one `char`,
/// so this never equals [`CharOffset`] for text containing astral characters.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Utf16Offset(pub usize);

impl Utf16Offset {
    /// Construct a UTF-16 code-unit offset.
    #[must_use]
    pub const fn new(value: usize) -> Self {
        Self(value)
    }

    /// The underlying `usize`.
    #[must_use]
    pub const fn get(self) -> usize {
        self.0
    }
}

/// A half-open span in **byte** space: `[start, end)`, both [`ByteOffset`]s.
///
/// The byte-space counterpart of a `Range<usize>`, kept as a newtype so the
/// [`Document`](crate::Document) aggregate's delete API cannot be handed a
/// `char`- or UTF-16-indexed range by mistake. Recover the raw `Range<usize>`
/// (e.g. to slice the rope) with [`get`](Self::get).
///
/// A `ByteRange` may be **inverted** (`start > end`); the value type itself does
/// not forbid this. Use [`ordered`](Self::ordered) to normalize when a caller
/// (such as a reversed selection) might produce one. Consumers that require an
/// ordered range — notably the str-slice in [`Document::delete`](crate::Document::delete)
/// — keep their own validation/panic contract; `ByteRange` does not silently
/// normalize on their behalf.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ByteRange {
    /// The start of the span (inclusive).
    pub start: ByteOffset,
    /// The end of the span (exclusive).
    pub end: ByteOffset,
}

impl ByteRange {
    /// Construct a byte range from `start` and `end` offsets.
    ///
    /// The two need not be ordered; see [`ordered`](Self::ordered).
    #[must_use]
    pub const fn new(start: ByteOffset, end: ByteOffset) -> Self {
        Self { start, end }
    }

    /// Construct a byte range from two [`ByteOffset`]s (alias of [`new`](Self::new)).
    #[must_use]
    pub const fn from_offsets(start: ByteOffset, end: ByteOffset) -> Self {
        Self::new(start, end)
    }

    /// The underlying raw `Range<usize>` (`start.get()..end.get()`).
    ///
    /// This is the storage-boundary escape hatch: hand the result to the rope /
    /// str slice. It preserves an inverted range verbatim (does not normalize).
    #[must_use]
    pub const fn get(self) -> Range<usize> {
        self.start.get()..self.end.get()
    }

    /// The length of the span in bytes (`end - start`).
    ///
    /// Only meaningful for an **ordered** range; for an inverted range this would
    /// underflow, so it saturates at `0` rather than panicking. Call
    /// [`ordered`](Self::ordered) first if the input direction is unknown.
    #[must_use]
    pub const fn len(self) -> usize {
        self.end.get().saturating_sub(self.start.get())
    }

    /// Whether the span is empty (`start == end`).
    #[must_use]
    pub const fn is_empty(self) -> bool {
        self.start.get() == self.end.get()
    }

    /// Normalize the range so `start <= end`, swapping the bounds if inverted.
    ///
    /// Useful for selections, whose `anchor`/`head` may be reversed. Leaves an
    /// already-ordered range untouched.
    #[must_use]
    pub fn ordered(self) -> Self {
        let lo = self.start.get().min(self.end.get());
        let hi = self.start.get().max(self.end.get());
        Self::new(ByteOffset::new(lo), ByteOffset::new(hi))
    }
}

impl From<Range<ByteOffset>> for ByteRange {
    fn from(range: Range<ByteOffset>) -> Self {
        Self::new(range.start, range.end)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_and_get_round_trip_for_each_space() {
        assert_eq!(ByteOffset::new(7).get(), 7);
        assert_eq!(CharOffset::new(7).get(), 7);
        assert_eq!(Utf16Offset::new(7).get(), 7);
    }

    #[test]
    fn tuple_field_and_get_agree() {
        assert_eq!(ByteOffset(3).0, ByteOffset::new(3).get());
        assert_eq!(CharOffset(3).0, CharOffset::new(3).get());
        assert_eq!(Utf16Offset(3).0, Utf16Offset::new(3).get());
    }

    #[test]
    fn offsets_are_ordered_by_their_value() {
        assert!(ByteOffset::new(1) < ByteOffset::new(2));
        assert!(Utf16Offset::new(5) > Utf16Offset::new(4));
    }

    #[test]
    fn byte_range_new_exposes_start_end_and_raw_range() {
        let r = ByteRange::new(ByteOffset::new(2), ByteOffset::new(5));
        assert_eq!(r.start, ByteOffset::new(2));
        assert_eq!(r.end, ByteOffset::new(5));
        assert_eq!(r.get(), 2..5);
    }

    #[test]
    fn byte_range_from_offsets_matches_new() {
        assert_eq!(
            ByteRange::from_offsets(ByteOffset::new(1), ByteOffset::new(4)),
            ByteRange::new(ByteOffset::new(1), ByteOffset::new(4)),
        );
    }

    #[test]
    fn byte_range_from_range_of_offsets() {
        let r: ByteRange = (ByteOffset::new(3)..ByteOffset::new(7)).into();
        assert_eq!(r, ByteRange::new(ByteOffset::new(3), ByteOffset::new(7)));
    }

    #[test]
    fn byte_range_len_of_ordered_range_is_end_minus_start() {
        assert_eq!(
            ByteRange::new(ByteOffset::new(2), ByteOffset::new(5)).len(),
            3
        );
    }

    #[test]
    fn byte_range_len_of_inverted_range_saturates_to_zero() {
        // end < start would underflow; len() saturates rather than panicking.
        assert_eq!(
            ByteRange::new(ByteOffset::new(5), ByteOffset::new(2)).len(),
            0
        );
    }

    #[test]
    fn byte_range_is_empty_only_when_start_equals_end() {
        assert!(ByteRange::new(ByteOffset::new(4), ByteOffset::new(4)).is_empty());
        assert!(!ByteRange::new(ByteOffset::new(4), ByteOffset::new(5)).is_empty());
    }

    #[test]
    fn byte_range_ordered_normalizes_a_reversed_range() {
        let reversed = ByteRange::new(ByteOffset::new(5), ByteOffset::new(2));
        let ordered = reversed.ordered();
        assert_eq!(
            ordered,
            ByteRange::new(ByteOffset::new(2), ByteOffset::new(5))
        );
        assert_eq!(ordered.get(), 2..5);
        assert_eq!(ordered.len(), 3);
    }

    #[test]
    fn byte_range_ordered_leaves_an_ordered_range_untouched() {
        let r = ByteRange::new(ByteOffset::new(2), ByteOffset::new(5));
        assert_eq!(r.ordered(), r);
    }
}
