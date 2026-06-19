use crate::edit_error::EditError;
use crate::offset::{ByteOffset, CharOffset, Utf16Offset};
use ropey::Rope;
use std::borrow::Cow;
use unicode_segmentation::{GraphemeCursor, UnicodeSegmentation};

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

    /// The text of the byte `range` as a [`Cow<str>`], without materializing the
    /// whole document (P1: preserves rope locality).
    ///
    /// Reads the range via `ropey`'s `byte_slice` (O(log n) descent + the slice
    /// length), not [`text`](Self::text) (which allocates the entire document).
    /// When the slice falls inside a single rope chunk it is returned **borrowed**
    /// (zero-copy); when it straddles chunks it is materialized into an owned
    /// `String`. Either way only the requested range is touched, not the whole
    /// buffer — the win that lets [`Document::delete`](crate::Document::delete)
    /// capture the removed text cheaply.
    ///
    /// # Panics
    ///
    /// Panics if either bound is out of range or not on a UTF-8 char boundary
    /// (the same trusted-caller contract as [`delete`](Self::delete); `ropey`'s
    /// `byte_slice` enforces it).
    pub fn slice(&self, range: std::ops::Range<usize>) -> Cow<'_, str> {
        let slice = self.rope.byte_slice(range);
        match slice.as_str() {
            Some(borrowed) => Cow::Borrowed(borrowed),
            None => Cow::Owned(slice.to_string()),
        }
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
    ///
    /// This low-level storage primitive speaks a **raw `Range<usize>`** by design,
    /// not the [`ByteRange`](crate::ByteRange) newtype (I1 decision: option a). The
    /// newtype discipline belongs at the **aggregate front door**
    /// ([`Document::delete`](crate::Document::delete)), which is the public mutation
    /// API; the buffer is the storage layer (like `ropey` itself), where raw byte
    /// ranges are appropriate and pushing the newtype down would be noise. The
    /// `Document` converts `ByteRange` → `usize` at the call site via
    /// [`ByteRange::get`](crate::ByteRange::get).
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

    // --- Non-panicking boundary validation (Task H1) ----------------------
    //
    // CONTRACT: these helpers VALIDATE offsets without mutating and WITHOUT
    // panicking for ANY input (including offsets far past `len()`). They are the
    // untrusted-boundary guards: the WASM `Editor` (and the H2 `Document` path)
    // call them BEFORE the panicking `insert`/`delete`, so a rejected op returns
    // an `Err` and leaves the instance fully usable (no partial apply, no
    // poisoning). The panicking primitives above stay as-is for trusted
    // in-process callers.

    /// Whether `byte` is a valid offset to edit at: in bounds (`0..=len()`) AND
    /// on a UTF-8 char boundary.
    ///
    /// Never panics for any input — `byte > len()` simply returns `false`. Uses
    /// `ropey`'s non-panicking `try_byte_to_char` and a `char_to_byte`
    /// round-trip: an offset is a real boundary iff that round-trip is the
    /// identity (`ropey` otherwise rounds a non-boundary index down).
    pub fn is_char_boundary(&self, byte: usize) -> bool {
        if byte > self.rope.len_bytes() {
            return false;
        }
        match self.rope.try_byte_to_char(byte) {
            Ok(char_idx) => self.rope.char_to_byte(char_idx) == byte,
            Err(_) => false,
        }
    }

    /// Snap `byte` **down** to the largest valid UTF-8 char boundary that is
    /// `<= min(byte, len())`. Never panics for any input.
    ///
    /// This is the non-panicking counterpart used to make a possibly-interior
    /// byte offset safe to hand to the grapheme primitives: a value far past the
    /// end snaps to `len()` (always a boundary), and a value that splits a
    /// multibyte scalar walks back to the START of that codepoint. Conventional
    /// editor choice: floor TOWARD the start of the codepoint, so a caret that an
    /// edit left mid-codepoint lands just before the affected character rather
    /// than after it.
    ///
    /// The loop runs at most 3 times for well-formed UTF-8 (a scalar value is at
    /// most 4 bytes, so an interior offset is at most 3 bytes past its boundary)
    /// and uses the non-panicking [`is_char_boundary`](Self::is_char_boundary)
    /// check, so it is safe for `core`'s `forbid(unsafe_code)` posture.
    pub fn floor_char_boundary(&self, byte: usize) -> usize {
        let mut b = byte.min(self.rope.len_bytes());
        while !self.is_char_boundary(b) {
            // `b == 0` is always a boundary, so this never underflows.
            b -= 1;
        }
        b
    }

    /// Validate an insertion point without mutating or panicking.
    ///
    /// Returns [`EditError::OutOfBounds`] if `at > len()`, or
    /// [`EditError::NotCharBoundary`] if `at` splits a multibyte scalar value;
    /// otherwise `Ok(())`. After `Ok`, [`insert`](Self::insert) at `at` cannot
    /// panic.
    pub fn validate_insert(&self, at: usize) -> Result<(), EditError> {
        let len = self.rope.len_bytes();
        if at > len {
            return Err(EditError::OutOfBounds { offset: at, len });
        }
        if !self.is_char_boundary(at) {
            return Err(EditError::NotCharBoundary { offset: at });
        }
        Ok(())
    }

    /// Validate a deletion range without mutating or panicking.
    ///
    /// Checks, in order: `start <= end` ([`EditError::InvertedRange`] else),
    /// `end <= len()` ([`EditError::OutOfBounds`] else), and that both bounds sit
    /// on char boundaries ([`EditError::NotCharBoundary`] else). After `Ok`,
    /// [`delete`](Self::delete) over `start..end` cannot panic.
    pub fn validate_delete(&self, start: usize, end: usize) -> Result<(), EditError> {
        if start > end {
            return Err(EditError::InvertedRange { start, end });
        }
        let len = self.rope.len_bytes();
        // `end` is the larger bound (start <= end), so an in-bounds `end`
        // implies an in-bounds `start`; check the binding bound.
        if end > len {
            return Err(EditError::OutOfBounds { offset: end, len });
        }
        if !self.is_char_boundary(start) {
            return Err(EditError::NotCharBoundary { offset: start });
        }
        if !self.is_char_boundary(end) {
            return Err(EditError::NotCharBoundary { offset: end });
        }
        Ok(())
    }

    /// Validate a deletion expressed as a start offset plus a **byte length**,
    /// computing the end with [`usize::checked_add`] so an adversarial
    /// `start + len` cannot wrap (F-1 defense — the Phase F audit flagged the
    /// `at + removed.len()` arithmetic in `event.rs` as an overflow risk).
    ///
    /// Returns [`EditError::Overflow`] if `start + len` overflows `usize`,
    /// otherwise defers to [`validate_delete`](Self::validate_delete) over
    /// `start..start + len`. This is the natural front door for the diff-based
    /// input path (Task I2) and the event-apply path, both of which derive an
    /// end from a start and a removed length.
    pub fn validate_delete_len(&self, start: usize, len: usize) -> Result<(), EditError> {
        let end = start.checked_add(len).ok_or(EditError::Overflow)?;
        self.validate_delete(start, end)
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

    // --- Non-panicking UTF-16 ↔ byte conversions (Task H2) ----------------
    //
    // CONTRACT: these `try_*` variants VALIDATE an untrusted offset and return a
    // typed [`EditError`] instead of panicking. The DOM / `<textarea>` /
    // `EditContext` hand the view UTF-16 code-unit offsets that are untrusted (a
    // mid-surrogate or out-of-range index must NOT trap across the WASM boundary —
    // Increment 1 blocker #1). The panicking `byte_to_utf16`/`utf16_to_byte`
    // above stay for trusted in-process callers.

    /// Convert a UTF-8 byte offset to a UTF-16 code-unit offset, returning a
    /// typed error instead of panicking on a bad offset.
    ///
    /// Returns [`EditError::OutOfBounds`] if `byte > len()`, or
    /// [`EditError::NotCharBoundary`] if `byte` splits a multibyte scalar value;
    /// otherwise the UTF-16 offset. Never panics for any input.
    pub fn try_byte_to_utf16(&self, byte: usize) -> Result<usize, EditError> {
        let len = self.rope.len_bytes();
        if byte > len {
            return Err(EditError::OutOfBounds { offset: byte, len });
        }
        if !self.is_char_boundary(byte) {
            return Err(EditError::NotCharBoundary { offset: byte });
        }
        let char_idx = self.rope.byte_to_char(byte);
        Ok(self.rope.char_to_utf16_cu(char_idx))
    }

    /// Convert a UTF-16 code-unit offset to a UTF-8 byte offset, returning a
    /// typed error instead of panicking on a bad offset.
    ///
    /// Returns [`EditError::OutOfBounds`] if `utf16 > utf16_len()`, or
    /// [`EditError::NotCodeUnitBoundary`] if `utf16` falls inside a surrogate
    /// pair (i.e. is not a scalar-value boundary); otherwise the byte offset.
    /// Never panics for any input — `ropey`'s `utf16_cu_to_char` would round a
    /// mid-pair index, so the round-trip is asserted via a comparison, not a
    /// panicking assert.
    pub fn try_utf16_to_byte(&self, utf16: usize) -> Result<usize, EditError> {
        let utf16_len = self.rope.len_utf16_cu();
        if utf16 > utf16_len {
            return Err(EditError::OutOfBounds {
                offset: utf16,
                len: utf16_len,
            });
        }
        let char_idx = self.rope.utf16_cu_to_char(utf16);
        if self.rope.char_to_utf16_cu(char_idx) != utf16 {
            return Err(EditError::NotCodeUnitBoundary { offset: utf16 });
        }
        Ok(self.rope.char_to_byte(char_idx))
    }

    /// The byte offset of the grapheme-cluster boundary at or before `b`.
    ///
    /// Steps left by one user-perceived grapheme cluster (ADR-0001), so a ZWJ
    /// emoji family or a base+combining-mark pair moves as a unit, not by byte
    /// or `char`. Clamps to `0` at the start of the buffer (see CONTRACT).
    ///
    /// # Panics
    ///
    /// Panics if `b` is not on a char boundary or is out of range: the backing
    /// [`GraphemeCursor`] rejects such an offset. Callers pass a valid boundary
    /// (the cursor only ever feeds in offsets it produced).
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
    ///
    /// # Panics
    ///
    /// Panics if `b` is not on a char boundary or is out of range (the backing
    /// [`GraphemeCursor`] rejects it); callers pass valid boundaries.
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

    /// The byte offset of the nearest word boundary strictly **before** `b`.
    ///
    /// CONVENTION: word boundaries are the segment edges reported by
    /// `unicode-segmentation`'s `split_word_bound_indices` (UAX #29 word
    /// boundaries). For `"foo bar"` the boundaries are `{0, 3, 4, 7}` — i.e. the
    /// edges of the runs `"foo"`, `" "`, `"bar"`. This method returns the
    /// largest boundary `< b.get()`. Stepping left from the start clamps to `0`
    /// (see CONTRACT). Note this is the *every-edge* convention (spaces are their
    /// own segment), so word-stepping visits run boundaries including whitespace
    /// edges, matching `split_word_bound_indices`.
    ///
    /// Delegates segmentation to `unicode-segmentation`; the cursor only owns the
    /// boundary-selection logic, never re-implements word splitting.
    ///
    /// Unlike the grapheme-boundary methods, this tolerates an interior (non-
    /// boundary) or out-of-range `b` without panicking: it always returns a valid
    /// segment boundary (clamped to `[0, len]`).
    pub fn prev_word_boundary(&self, b: ByteOffset) -> ByteOffset {
        let text = self.text();
        let target = b.get();
        // The last segment edge strictly before `target`, else 0.
        let prev = text
            .split_word_bound_indices()
            .map(|(i, _)| i)
            .take_while(|&i| i < target)
            .last()
            .unwrap_or(0);
        ByteOffset::new(prev)
    }

    /// The byte offset of the nearest word boundary strictly **after** `b`.
    ///
    /// The forward counterpart of
    /// [`prev_word_boundary`](Self::prev_word_boundary), using the same
    /// `split_word_bound_indices` convention. For `"foo bar"` from offset `0`
    /// this returns `3` (the end of `"foo"`); from `3` it returns `4` (the end of
    /// the space run); from `4` it returns `7` (the end of `"bar"`). Stepping
    /// right past the end clamps to the buffer length (see CONTRACT).
    ///
    /// Like [`prev_word_boundary`](Self::prev_word_boundary), it tolerates an
    /// interior or out-of-range `b` and always returns a valid boundary.
    pub fn next_word_boundary(&self, b: ByteOffset) -> ByteOffset {
        let text = self.text();
        let len = text.len();
        let target = b.get();
        // The first segment END strictly after `target`. `split_word_bound_indices`
        // yields segment START offsets, so a segment ending at `e` is the START of
        // the following segment, or `len` for the final segment.
        let next = text
            .split_word_bound_indices()
            .map(|(i, seg)| i + seg.len())
            .find(|&e| e > target)
            .unwrap_or(len);
        ByteOffset::new(next.min(len))
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
    fn slice_returns_subrange_text() {
        // The rope-locality read path (P1): slice a byte range without
        // materializing the whole document. ASCII, multibyte, and empty range.
        let buf = TextBuffer::from_str("Hello world");
        assert_eq!(buf.slice(5..11).as_ref(), " world");
        let emoji = TextBuffer::from_str("x😀y"); // 😀 occupies bytes 1..5
        assert_eq!(emoji.slice(1..5).as_ref(), "😀");
        assert_eq!(emoji.slice(0..0).as_ref(), ""); // empty range
        assert_eq!(emoji.slice(0..emoji.len()).as_ref(), "x😀y"); // whole doc
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

    // --- Non-panicking boundary validation (Task H1) ----------------------

    #[test]
    fn is_char_boundary_accepts_bounds_and_real_boundaries() {
        // "café": boundaries at bytes 0,1,2,3,5 ('é' spans 3..5). Byte 4 is
        // mid-'é'; byte 5 is the end (== len).
        let buf = TextBuffer::from_str("café");
        assert!(buf.is_char_boundary(0));
        assert!(buf.is_char_boundary(3));
        assert!(buf.is_char_boundary(5)); // == len
        assert!(!buf.is_char_boundary(4)); // inside 'é'
    }

    #[test]
    fn is_char_boundary_past_end_is_false_not_panic() {
        // Must NOT panic for byte > len — just `false`.
        let buf = TextBuffer::from_str("hi");
        assert!(!buf.is_char_boundary(3));
        assert!(!buf.is_char_boundary(usize::MAX));
    }

    #[test]
    fn floor_char_boundary_snaps_mid_codepoint_down() {
        // "ab😀": a=0..1, b=1..2, 😀=2..6 (4 UTF-8 bytes). Interior bytes 3,4,5
        // all floor to 2 (the START of the emoji); the boundaries floor to
        // themselves.
        let buf = TextBuffer::from_str("ab😀");
        assert_eq!(buf.floor_char_boundary(2), 2); // on the boundary before 😀
        assert_eq!(buf.floor_char_boundary(3), 2); // interior → start of 😀
        assert_eq!(buf.floor_char_boundary(4), 2); // interior → start of 😀
        assert_eq!(buf.floor_char_boundary(5), 2); // interior → start of 😀
        assert_eq!(buf.floor_char_boundary(6), 6); // == len, a boundary (after 😀)
    }

    #[test]
    fn floor_char_boundary_past_end_returns_len() {
        // Anything past the end snaps to len() (always a boundary), never panics.
        let buf = TextBuffer::from_str("ab😀"); // len 6
        assert_eq!(buf.floor_char_boundary(6), 6);
        assert_eq!(buf.floor_char_boundary(7), 6);
        assert_eq!(buf.floor_char_boundary(usize::MAX), 6);
    }

    #[test]
    fn floor_char_boundary_on_boundary_is_identity() {
        // "café": boundaries at 0,1,2,3,5 ('é' spans 3..5). Each floors to itself;
        // byte 4 (mid-'é') floors down to 3.
        let buf = TextBuffer::from_str("café");
        for b in [0usize, 1, 2, 3, 5] {
            assert_eq!(buf.floor_char_boundary(b), b, "boundary {b}");
        }
        assert_eq!(buf.floor_char_boundary(4), 3); // interior of 'é' → 3
    }

    #[test]
    fn floor_char_boundary_ascii_is_identity_everywhere() {
        // Every byte of an ASCII string is a boundary.
        let buf = TextBuffer::from_str("hello"); // len 5
        for b in 0..=5 {
            assert_eq!(buf.floor_char_boundary(b), b, "ascii byte {b}");
        }
    }

    #[test]
    fn floor_char_boundary_on_empty_buffer_is_zero() {
        // Empty buffer: len 0; any input floors to 0, never panics.
        let buf = TextBuffer::new();
        assert_eq!(buf.floor_char_boundary(0), 0);
        assert_eq!(buf.floor_char_boundary(99), 0);
    }

    #[test]
    fn validate_insert_ok_at_valid_boundary() {
        let buf = TextBuffer::from_str("café");
        assert_eq!(buf.validate_insert(0), Ok(()));
        assert_eq!(buf.validate_insert(3), Ok(()));
        assert_eq!(buf.validate_insert(5), Ok(())); // at end (== len)
    }

    #[test]
    fn validate_insert_out_of_bounds_returns_out_of_bounds() {
        let buf = TextBuffer::from_str("hi"); // len 2
        assert_eq!(
            buf.validate_insert(99),
            Err(EditError::OutOfBounds { offset: 99, len: 2 })
        );
    }

    #[test]
    fn validate_insert_inside_multibyte_returns_not_char_boundary() {
        // "café": 'é' occupies bytes 3..5; byte 4 is its interior.
        let buf = TextBuffer::from_str("café");
        assert_eq!(
            buf.validate_insert(4),
            Err(EditError::NotCharBoundary { offset: 4 })
        );
    }

    #[test]
    fn validate_delete_ok_for_valid_range() {
        let buf = TextBuffer::from_str("café"); // len 5
        assert_eq!(buf.validate_delete(0, 5), Ok(()));
        assert_eq!(buf.validate_delete(3, 5), Ok(()));
        assert_eq!(buf.validate_delete(2, 2), Ok(())); // empty range
    }

    #[test]
    fn validate_delete_inverted_returns_inverted_range() {
        let buf = TextBuffer::from_str("hello");
        assert_eq!(
            buf.validate_delete(4, 1),
            Err(EditError::InvertedRange { start: 4, end: 1 })
        );
    }

    #[test]
    fn validate_delete_out_of_bounds_returns_out_of_bounds() {
        let buf = TextBuffer::from_str("hi"); // len 2
        assert_eq!(
            buf.validate_delete(0, 99),
            Err(EditError::OutOfBounds { offset: 99, len: 2 })
        );
    }

    #[test]
    fn validate_delete_non_boundary_bound_returns_not_char_boundary() {
        // "café": deleting up to byte 4 (mid-'é') must be rejected.
        let buf = TextBuffer::from_str("café");
        assert_eq!(
            buf.validate_delete(0, 4),
            Err(EditError::NotCharBoundary { offset: 4 })
        );
        // A non-boundary start is rejected too.
        assert_eq!(
            buf.validate_delete(4, 5),
            Err(EditError::NotCharBoundary { offset: 4 })
        );
    }

    #[test]
    fn validate_delete_len_ok_and_overflow() {
        let buf = TextBuffer::from_str("hello"); // len 5
        assert_eq!(buf.validate_delete_len(1, 3), Ok(())); // 1..4
                                                           // start + len overflows usize → Overflow, not a panic.
        assert_eq!(
            buf.validate_delete_len(usize::MAX, 1),
            Err(EditError::Overflow)
        );
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
    #[should_panic(expected = "is out of bounds")]
    fn char_to_byte_out_of_bounds_panics() {
        // "hi" is 2 chars (len_chars == 2); char offset 99 is well past the end,
        // tripping the OOB assert in char_to_byte (see CONTRACT).
        let buf = TextBuffer::from_str("hi");
        buf.char_to_byte(CharOffset::new(99));
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
    #[should_panic(expected = "falls inside a surrogate pair")]
    fn utf16_to_byte_inside_surrogate_pair_panics() {
        // "😀" is one astral char = 2 UTF-16 code units (a surrogate pair). Offset
        // 1 lands BETWEEN the high and low surrogate, which is not a scalar-value
        // boundary, so the round-trip assert must reject it (see CONTRACT).
        let buf = TextBuffer::from_str("😀");
        buf.utf16_to_byte(Utf16Offset(1));
    }

    #[test]
    #[should_panic(expected = "is out of bounds")]
    fn utf16_to_byte_out_of_bounds_panics() {
        // "😀" has len_utf16 == 2; offset 3 is past the end, exercising the OOB
        // guard distinct from the surrogate-pair assert (see CONTRACT).
        let buf = TextBuffer::from_str("😀");
        buf.utf16_to_byte(Utf16Offset(3));
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

    // --- Non-panicking UTF-16 ↔ byte conversions (Task H2) ----------------

    #[test]
    fn try_byte_to_utf16_round_trips_on_astral_text() {
        // "a😀": a=0..1, 😀=1..5 (4 bytes / 1 char / 2 utf-16 code units).
        let buf = TextBuffer::from_str("a😀");
        assert_eq!(buf.try_byte_to_utf16(0), Ok(0));
        assert_eq!(buf.try_byte_to_utf16(1), Ok(1)); // after 'a'
        assert_eq!(buf.try_byte_to_utf16(5), Ok(3)); // after 😀 (1 + 2 surrogates)
                                                     // Inverse round-trips.
        assert_eq!(buf.try_utf16_to_byte(0), Ok(0));
        assert_eq!(buf.try_utf16_to_byte(1), Ok(1));
        assert_eq!(buf.try_utf16_to_byte(3), Ok(5));
    }

    #[test]
    fn try_byte_to_utf16_rejects_out_of_bounds_and_mid_codepoint() {
        // "café": len 5; 'é' spans 3..5, byte 4 is interior.
        let buf = TextBuffer::from_str("café");
        assert_eq!(
            buf.try_byte_to_utf16(99),
            Err(EditError::OutOfBounds { offset: 99, len: 5 })
        );
        assert_eq!(
            buf.try_byte_to_utf16(4),
            Err(EditError::NotCharBoundary { offset: 4 })
        );
    }

    #[test]
    fn try_utf16_to_byte_rejects_mid_surrogate_and_out_of_bounds() {
        // "😀": utf16_len 2; offset 1 is between the high/low surrogate.
        let buf = TextBuffer::from_str("😀");
        assert_eq!(
            buf.try_utf16_to_byte(1),
            Err(EditError::NotCodeUnitBoundary { offset: 1 })
        );
        assert_eq!(
            buf.try_utf16_to_byte(3),
            Err(EditError::OutOfBounds { offset: 3, len: 2 })
        );
    }

    // --- Word boundaries (Task F6 support) --------------------------------

    #[test]
    fn next_word_boundary_walks_segment_ends_of_foo_bar() {
        // "foo bar": split_word_bound_indices runs are "foo"(0..3) " "(3..4)
        // "bar"(4..7), so segment ENDS are {3,4,7}. next_word_boundary returns
        // the first end strictly after the offset.
        let buf = TextBuffer::from_str("foo bar");
        assert_eq!(buf.next_word_boundary(ByteOffset(0)), ByteOffset(3));
        assert_eq!(buf.next_word_boundary(ByteOffset(3)), ByteOffset(4));
        assert_eq!(buf.next_word_boundary(ByteOffset(4)), ByteOffset(7));
    }

    #[test]
    fn prev_word_boundary_walks_segment_starts_of_foo_bar() {
        // Segment START boundaries are {0,3,4,7}; prev returns the largest
        // boundary strictly before the offset.
        let buf = TextBuffer::from_str("foo bar");
        assert_eq!(buf.prev_word_boundary(ByteOffset(7)), ByteOffset(4));
        assert_eq!(buf.prev_word_boundary(ByteOffset(4)), ByteOffset(3));
        assert_eq!(buf.prev_word_boundary(ByteOffset(3)), ByteOffset(0));
    }

    #[test]
    fn word_boundaries_clamp_at_buffer_ends() {
        let buf = TextBuffer::from_str("foo bar");
        // prev at start clamps to 0; next at end clamps to len.
        assert_eq!(buf.prev_word_boundary(ByteOffset(0)), ByteOffset(0));
        assert_eq!(buf.next_word_boundary(ByteOffset(7)), ByteOffset(7));
    }

    #[test]
    fn word_boundaries_handle_multibyte_runs() {
        // "café本" — 'é' is 2 bytes, '本' is 3 bytes. UAX#29 keeps the word run
        // together; word boundaries land on real byte boundaries, never inside a
        // multibyte char.
        let buf = TextBuffer::from_str("café 本");
        // From 0, next boundary is end of "café" (5 bytes: c,a,f,é=2).
        assert_eq!(buf.next_word_boundary(ByteOffset(0)), ByteOffset(5));
        // prev from len lands at the start of the final word "本" (after space).
        let len = buf.len();
        assert_eq!(buf.prev_word_boundary(ByteOffset(len)), ByteOffset(6));
    }
}
