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
}
