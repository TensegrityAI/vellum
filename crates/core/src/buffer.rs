use crate::offset::{ByteOffset, CharOffset, Utf16Offset};
use ropey::Rope;
use unicode_segmentation::GraphemeCursor;

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

    /// Number of UTF-16 code units in the buffer.
    ///
    /// This is what the DOM / `<textarea>` / `EditContext` report as the value
    /// length. Astral-plane scalar values count as two code units (a surrogate
    /// pair), so this differs from [`char_len`](Self::char_len) for emoji and
    /// other astral text (e.g. "😀" is 1 char but 2 UTF-16 code units).
    pub fn utf16_len(&self) -> usize {
        self.rope.len_utf16_cu()
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

    // --- Offset conversions (Task F3) -------------------------------------
    //
    // CONTRACT: the conversion methods (`byte_to_char`, `char_to_byte`,
    // `byte_to_utf16`, `utf16_to_byte`) take a **valid boundary** in their input
    // space and PANIC on an out-of-range or non-boundary offset. This mirrors
    // the existing `byte_to_char_strict` contract: a bad offset is a programmer
    // error, not a recoverable condition, so it fails loudly rather than
    // silently corrupting the mapping. (Fallible, `Result`-returning variants at
    // the WASM boundary are Task H1/H2, not here.)
    //
    // The grapheme-boundary methods (`prev_grapheme_boundary`,
    // `next_grapheme_boundary`) instead **clamp** at the buffer ends: stepping
    // before the start yields offset 0 and stepping past the end yields the
    // buffer length. Clamping is their natural, expected behavior (a cursor at
    // the edge simply does not move), so it is not treated as an error.

    /// Convert a UTF-8 byte offset to a Unicode scalar (`char`) offset.
    ///
    /// Panics if `b` is out of bounds or not on a char boundary (see CONTRACT).
    pub fn byte_to_char(&self, b: ByteOffset) -> CharOffset {
        CharOffset::new(self.byte_to_char_strict(b.get()))
    }

    /// Convert a Unicode scalar (`char`) offset to a UTF-8 byte offset.
    ///
    /// Panics if `c` is out of bounds (see CONTRACT). Every char index is, by
    /// definition, on a char boundary, so there is no boundary check here.
    pub fn char_to_byte(&self, c: CharOffset) -> ByteOffset {
        let char_idx = c.get();
        assert!(
            char_idx <= self.rope.len_chars(),
            "char offset {char_idx} is out of bounds",
        );
        ByteOffset::new(self.rope.char_to_byte(char_idx))
    }

    /// Convert a UTF-8 byte offset to a UTF-16 code-unit offset.
    ///
    /// Routes through the char space: `ropey` exposes `char_to_utf16_cu`
    /// directly, and a validated byte→char conversion guarantees the input is a
    /// real boundary. Panics on a bad offset (see CONTRACT).
    pub fn byte_to_utf16(&self, b: ByteOffset) -> Utf16Offset {
        let char_idx = self.byte_to_char_strict(b.get());
        Utf16Offset::new(self.rope.char_to_utf16_cu(char_idx))
    }

    /// Convert a UTF-16 code-unit offset to a UTF-8 byte offset.
    ///
    /// The inverse of [`byte_to_utf16`](Self::byte_to_utf16), via the char
    /// space. Panics if `u` is out of bounds or falls inside a surrogate pair
    /// (i.e. is not on a scalar-value boundary): `ropey`'s `utf16_cu_to_char`
    /// would round a mid-pair index, so we assert the round-trip is exact.
    pub fn utf16_to_byte(&self, u: Utf16Offset) -> ByteOffset {
        let utf16_idx = u.get();
        assert!(
            utf16_idx <= self.rope.len_utf16_cu(),
            "utf-16 offset {utf16_idx} is out of bounds",
        );
        let char_idx = self.rope.utf16_cu_to_char(utf16_idx);
        assert_eq!(
            self.rope.char_to_utf16_cu(char_idx),
            utf16_idx,
            "utf-16 offset {utf16_idx} falls inside a surrogate pair",
        );
        ByteOffset::new(self.rope.char_to_byte(char_idx))
    }

    /// The byte offset of the grapheme-cluster boundary at or before `b`.
    ///
    /// Steps left by one user-perceived grapheme cluster (ADR-0001), so a ZWJ
    /// emoji family or a base+combining-mark pair moves as a unit, not by byte
    /// or `char`. Clamps to `0` at the start of the buffer (see CONTRACT).
    ///
    /// Implementation note: this materializes the buffer text via
    /// [`text`](Self::text) and uses a [`GraphemeCursor`] over it. For Inc 1
    /// that is correct and simple; a chunk-streaming cursor over the rope (to
    /// avoid the full materialization on very large documents) is a later
    /// optimization and does not change this method's contract.
    pub fn prev_grapheme_boundary(&self, b: ByteOffset) -> ByteOffset {
        let text = self.text();
        let mut cursor = GraphemeCursor::new(b.get(), text.len(), true);
        match cursor.prev_boundary(&text, 0) {
            // `None` means we were already at the start: clamp to 0.
            Ok(Some(prev)) => ByteOffset::new(prev),
            Ok(None) => ByteOffset::new(0),
            // The cursor only needs the (single) provided chunk for an
            // in-memory string, so it never asks for more context here.
            Err(_) => ByteOffset::new(0),
        }
    }

    /// The byte offset of the grapheme-cluster boundary at or after `b`.
    ///
    /// The forward counterpart of
    /// [`prev_grapheme_boundary`](Self::prev_grapheme_boundary): steps right by
    /// one grapheme cluster, clamping to the buffer length at the end (see
    /// CONTRACT). Same materialization tradeoff applies.
    pub fn next_grapheme_boundary(&self, b: ByteOffset) -> ByteOffset {
        let text = self.text();
        let len = text.len();
        let mut cursor = GraphemeCursor::new(b.get(), len, true);
        match cursor.next_boundary(&text, 0) {
            // `None` means we were already at the end: clamp to len.
            Ok(Some(next)) => ByteOffset::new(next),
            Ok(None) => ByteOffset::new(len),
            Err(_) => ByteOffset::new(len),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::offset::{ByteOffset, CharOffset, Utf16Offset};

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

    #[test]
    fn delete_empty_range_is_noop() {
        let mut buf = TextBuffer::from_str("Hello");
        buf.delete(3..3); // start == end
        assert_eq!(buf.text(), "Hello");
    }

    #[test]
    fn insert_at_end_of_buffer_appends() {
        let mut buf = TextBuffer::from_str("Hello");
        buf.insert(buf.len(), "!"); // at == len()
        assert_eq!(buf.text(), "Hello!");
    }

    #[test]
    fn char_len_of_empty_buffer_is_zero() {
        let buf = TextBuffer::new();
        assert_eq!(buf.char_len(), 0);
    }

    #[test]
    #[should_panic]
    fn insert_out_of_bounds_panics() {
        // Past the end of the buffer: exercises the `try_byte_to_char` Err / OOB
        // arm, distinct from the non-char-boundary assert.
        let mut buf = TextBuffer::from_str("Hi");
        buf.insert(999, "x");
    }

    #[test]
    #[should_panic]
    fn delete_out_of_bounds_panics() {
        // The delete path runs the same OOB arm of `byte_to_char_strict`.
        let mut buf = TextBuffer::from_str("Hi");
        buf.delete(0..999);
    }

    // --- Offset conversions (Task F3) -------------------------------------

    #[test]
    fn cafe_byte_char_utf16_lengths() {
        // "café": 'é' is 2 UTF-8 bytes / 1 char / 1 UTF-16 code unit.
        let buf = TextBuffer::from_str("café");
        assert_eq!(buf.len(), 5); // bytes
        assert_eq!(buf.char_len(), 4); // chars
        assert_eq!(buf.utf16_len(), 4); // utf-16 code units
    }

    #[test]
    fn cafe_byte_to_utf16_round_trips_at_every_char_boundary() {
        // No astral chars, so utf16 offsets equal char counts here, but the
        // point is the byte_to_utf16 → utf16_to_byte identity at each boundary.
        let buf = TextBuffer::from_str("café");
        for b in [0usize, 1, 2, 3, 5] {
            // byte 4 is mid-'é' (not a boundary); skip it.
            let u = buf.byte_to_utf16(ByteOffset(b));
            assert_eq!(buf.utf16_to_byte(u), ByteOffset(b), "byte {b}");
        }
        // Specific expected utf16 offsets.
        assert_eq!(buf.byte_to_utf16(ByteOffset(0)), Utf16Offset(0));
        assert_eq!(buf.byte_to_utf16(ByteOffset(3)), Utf16Offset(3)); // after 'caf'
        assert_eq!(buf.byte_to_utf16(ByteOffset(5)), Utf16Offset(4)); // after 'é'
    }

    #[test]
    fn byte_to_char_and_char_to_byte_round_trip() {
        let buf = TextBuffer::from_str("café");
        assert_eq!(buf.byte_to_char(ByteOffset(0)), CharOffset(0));
        assert_eq!(buf.byte_to_char(ByteOffset(3)), CharOffset(3));
        assert_eq!(buf.byte_to_char(ByteOffset(5)), CharOffset(4));
        assert_eq!(buf.char_to_byte(CharOffset(4)), ByteOffset(5));
        assert_eq!(buf.char_to_byte(CharOffset(0)), ByteOffset(0));
    }

    #[test]
    fn astral_emoji_is_four_bytes_one_char_two_utf16_units() {
        // "😀" U+1F600: 4 UTF-8 bytes, 1 char, 2 UTF-16 code units (surrogate pair).
        let buf = TextBuffer::from_str("😀");
        assert_eq!(buf.len(), 4);
        assert_eq!(buf.char_len(), 1);
        assert_eq!(buf.utf16_len(), 2);
        assert_eq!(buf.byte_to_utf16(ByteOffset(4)), Utf16Offset(2));
        assert_eq!(buf.utf16_to_byte(Utf16Offset(2)), ByteOffset(4));
    }

    #[test]
    fn cjk_byte_char_utf16_lengths_and_round_trip() {
        // "日本語": 3 chars × 3 bytes = 9 bytes, 3 chars, 3 utf-16 units (BMP).
        let buf = TextBuffer::from_str("日本語");
        assert_eq!(buf.len(), 9);
        assert_eq!(buf.char_len(), 3);
        assert_eq!(buf.utf16_len(), 3);
        for b in [0usize, 3, 6, 9] {
            let u = buf.byte_to_utf16(ByteOffset(b));
            assert_eq!(buf.utf16_to_byte(u), ByteOffset(b), "byte {b}");
        }
        assert_eq!(buf.byte_to_utf16(ByteOffset(6)), Utf16Offset(2));
    }

    #[test]
    fn utf16_byte_round_trip_holds_at_every_char_boundary() {
        // Property: for each char boundary, utf16_to_byte(byte_to_utf16(b)) == b.
        let buf = TextBuffer::from_str("a😀é本z");
        let text = buf.text();
        for (b, _) in text
            .char_indices()
            .map(|(i, _)| (i, ()))
            .chain(std::iter::once((text.len(), ())))
        {
            let u = buf.byte_to_utf16(ByteOffset(b));
            assert_eq!(buf.utf16_to_byte(u), ByteOffset(b), "byte {b}");
        }
    }

    #[test]
    fn next_grapheme_boundary_skips_entire_zwj_family_cluster() {
        // "a👨‍👩‍👧b": the family is ONE grapheme made of many chars/bytes joined
        // by ZWJ. next_grapheme from just after 'a' must land right before 'b'.
        let buf = TextBuffer::from_str("a👨‍👩‍👧b");
        let text = buf.text();
        let family_bytes = "👨‍👩‍👧".len();
        let after_a = ByteOffset(1);
        let before_b = ByteOffset(1 + family_bytes);
        assert_eq!(buf.next_grapheme_boundary(after_a), before_b);
        // Sanity: the cluster is genuinely multi-byte/multi-char.
        assert!(family_bytes > 4);
        assert_eq!(text.len(), 1 + family_bytes + 1);
    }

    #[test]
    fn prev_grapheme_boundary_skips_entire_zwj_family_cluster() {
        let buf = TextBuffer::from_str("a👨‍👩‍👧b");
        let family_bytes = "👨‍👩‍👧".len();
        let before_b = ByteOffset(1 + family_bytes);
        // Stepping back from just before 'b' lands right after 'a'.
        assert_eq!(buf.prev_grapheme_boundary(before_b), ByteOffset(1));
    }

    #[test]
    fn next_grapheme_boundary_steps_one_astral_char() {
        let buf = TextBuffer::from_str("😀x");
        assert_eq!(buf.next_grapheme_boundary(ByteOffset(0)), ByteOffset(4));
    }

    #[test]
    fn grapheme_boundaries_clamp_at_buffer_ends() {
        let buf = TextBuffer::from_str("abc");
        // prev at the start clamps to 0.
        assert_eq!(buf.prev_grapheme_boundary(ByteOffset(0)), ByteOffset(0));
        // next at the end clamps to len.
        assert_eq!(buf.next_grapheme_boundary(ByteOffset(3)), ByteOffset(3));
    }

    #[test]
    fn grapheme_boundaries_on_empty_buffer_stay_at_zero() {
        let buf = TextBuffer::new();
        assert_eq!(buf.prev_grapheme_boundary(ByteOffset(0)), ByteOffset(0));
        assert_eq!(buf.next_grapheme_boundary(ByteOffset(0)), ByteOffset(0));
    }
}
