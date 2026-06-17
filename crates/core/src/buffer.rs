use ropey::Rope;

/// A rope-backed text buffer.
///
/// Backed by [`ropey::Rope`] (ADR-0006) so inserts/deletes and offset lookups are
/// cheap on real documents instead of O(n) memmoves over a `String`. The rope is a
/// private implementation detail behind this stable API: callers speak to
/// `TextBuffer` (byte offsets in, see below), never to the backing store, so the
/// rope can be swapped for a hand-rolled one later without caller churn.
///
/// Offsets in this API are **byte** offsets (UTF-8), matching the Increment 0
/// contract; internally they are converted to `ropey`'s char indices.
#[derive(Debug, Default, Clone)]
pub struct TextBuffer {
    rope: Rope,
}

impl TextBuffer {
    /// Create an empty buffer.
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a buffer initialized with `s`.
    ///
    /// Named `from_str` deliberately (the WASM `Editor` binding mirrors it); this
    /// is infallible construction, not the fallible `std::str::FromStr` contract.
    #[allow(clippy::should_implement_trait)]
    pub fn from_str(s: &str) -> Self {
        Self {
            rope: Rope::from_str(s),
        }
    }

    /// The buffer contents as an owned `String`.
    ///
    /// A `Rope` stores text in chunks, so it cannot hand out a `&str` borrow of the
    /// whole document cheaply; this materializes it. Callers that previously took a
    /// `&str` should borrow the returned value (`&buf.text()`).
    pub fn text(&self) -> String {
        self.rope.to_string()
    }

    /// Length of the buffer in **bytes** (UTF-8).
    pub fn len(&self) -> usize {
        self.rope.len_bytes()
    }

    /// Number of Unicode scalar values (chars) in the buffer.
    ///
    /// Differs from [`len`](Self::len) for multibyte text (e.g. "café" is 5 bytes
    /// but 4 chars). This is char count, not grapheme count — grapheme boundaries
    /// stay in `core` via `unicode-segmentation` (ADR-0001), layered separately.
    pub fn char_len(&self) -> usize {
        self.rope.len_chars()
    }

    /// Whether the buffer is empty.
    pub fn is_empty(&self) -> bool {
        self.rope.len_bytes() == 0
    }

    /// Insert `s` at byte offset `at`. Panics if `at` is not a char boundary.
    ///
    /// The byte offset is converted to a `ropey` char index. `ropey`'s
    /// `byte_to_char` rounds non-boundary indices down rather than panicking, so an
    /// explicit char-boundary assertion preserves the documented panic contract.
    pub fn insert(&mut self, at: usize, s: &str) {
        let char_idx = self.byte_to_char_strict(at);
        self.rope.insert(char_idx, s);
    }

    /// Delete the byte `range`. Panics if either bound is not a char boundary.
    ///
    /// Both bounds are converted from byte to char index with the same strict
    /// char-boundary check as [`insert`](Self::insert).
    pub fn delete(&mut self, range: std::ops::Range<usize>) {
        let char_start = self.byte_to_char_strict(range.start);
        let char_end = self.byte_to_char_strict(range.end);
        self.rope.remove(char_start..char_end);
    }

    /// Convert a byte offset to a `ropey` char index, panicking if the byte offset
    /// is not on a UTF-8 char boundary.
    ///
    /// `ropey`'s `byte_to_char` silently rounds a non-boundary index down to the
    /// previous char; that would corrupt edits, so we assert the boundary first to
    /// preserve the `TextBuffer` contract (a non-char-boundary offset panics).
    ///
    /// A byte offset is on a char boundary iff round-tripping it through
    /// `byte_to_char` → `char_to_byte` is the identity. `try_byte_to_char` returns
    /// `Err` for an out-of-bounds offset, which is treated as a contract violation.
    fn byte_to_char_strict(&self, byte: usize) -> usize {
        let char_idx = self
            .rope
            .try_byte_to_char(byte)
            .unwrap_or_else(|_| panic!("byte offset {byte} is out of bounds"));
        assert_eq!(
            self.rope.char_to_byte(char_idx),
            byte,
            "byte offset {byte} is not on a UTF-8 char boundary",
        );
        char_idx
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_buffer_is_empty() {
        let buf = TextBuffer::new();
        assert_eq!(buf.text(), "");
        assert_eq!(buf.len(), 0);
    }

    #[test]
    fn from_str_round_trips() {
        let buf = TextBuffer::from_str("Hello {{ name }}");
        assert_eq!(buf.text(), "Hello {{ name }}");
    }

    #[test]
    fn insert_at_start_middle_end() {
        let mut buf = TextBuffer::from_str("Helo");
        buf.insert(3, "l"); // byte offset
        assert_eq!(buf.text(), "Hello");
    }

    #[test]
    fn delete_range_removes_text() {
        let mut buf = TextBuffer::from_str("Hello world");
        buf.delete(5..11);
        assert_eq!(buf.text(), "Hello");
    }

    #[test]
    #[should_panic]
    fn insert_on_non_char_boundary_panics() {
        let mut buf = TextBuffer::from_str("áé"); // multibyte
        buf.insert(1, "x"); // splits 'á'
    }

    #[test]
    fn char_len_differs_from_byte_len_for_accented_text() {
        // "café": 'é' is 2 bytes → 5 bytes, 4 chars.
        let buf = TextBuffer::from_str("café");
        assert_eq!(buf.len(), 5);
        assert_eq!(buf.char_len(), 4);
    }

    #[test]
    fn char_len_differs_from_byte_len_for_cjk_text() {
        // "日本語": each char is 3 bytes → 9 bytes, 3 chars.
        let buf = TextBuffer::from_str("日本語");
        assert_eq!(buf.len(), 9);
        assert_eq!(buf.char_len(), 3);
    }

    #[test]
    fn char_len_differs_from_byte_len_for_astral_emoji() {
        // "😀" is a single astral scalar value → 4 bytes, 1 char.
        let buf = TextBuffer::from_str("😀");
        assert_eq!(buf.len(), 4);
        assert_eq!(buf.char_len(), 1);
    }

    #[test]
    fn char_len_equals_byte_len_for_ascii() {
        let buf = TextBuffer::from_str("hello");
        assert_eq!(buf.len(), 5);
        assert_eq!(buf.char_len(), 5);
    }

    #[test]
    fn large_text_insert_in_the_middle_is_correct() {
        // Build a multi-KB string to exercise the rope path.
        let block = "abcdefghij".repeat(500); // 5000 bytes
        let mut buf = TextBuffer::from_str(&block);
        assert_eq!(buf.len(), 5000);

        let mid = 2500;
        buf.insert(mid, "INSERTED");

        let mut expected = block.clone();
        expected.insert_str(mid, "INSERTED");
        assert_eq!(buf.text(), expected);
        assert_eq!(buf.len(), 5008);
    }

    #[test]
    fn large_text_delete_in_the_middle_is_correct() {
        let block = "abcdefghij".repeat(500); // 5000 bytes
        let mut buf = TextBuffer::from_str(&block);

        buf.delete(2000..3000);

        let mut expected = block.clone();
        expected.replace_range(2000..3000, "");
        assert_eq!(buf.text(), expected);
        assert_eq!(buf.len(), 4000);
    }

    #[test]
    fn large_text_insert_then_delete_round_trips() {
        let block = "0123456789".repeat(300); // 3000 bytes
        let mut buf = TextBuffer::from_str(&block);

        buf.insert(1500, "PAYLOAD");
        buf.delete(1500..1500 + "PAYLOAD".len());

        assert_eq!(buf.text(), block);
        assert_eq!(buf.len(), 3000);
    }
}
