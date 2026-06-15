/// A simple string-backed text buffer. Replaced by a rope in Increment 1 (ADR-0002).
#[derive(Debug, Default, Clone)]
pub struct TextBuffer {
    text: String,
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
            text: s.to_string(),
        }
    }

    /// Borrow the buffer contents as a string slice.
    pub fn text(&self) -> &str {
        &self.text
    }

    /// Length of the buffer in bytes.
    pub fn len(&self) -> usize {
        self.text.len()
    }

    /// Whether the buffer is empty.
    pub fn is_empty(&self) -> bool {
        self.text.is_empty()
    }

    /// Insert `s` at byte offset `at`. Panics if `at` is not a char boundary.
    pub fn insert(&mut self, at: usize, s: &str) {
        self.text.insert_str(at, s);
    }

    /// Delete the byte `range`. Panics if either bound is not a char boundary.
    pub fn delete(&mut self, range: std::ops::Range<usize>) {
        self.text.replace_range(range, "");
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
}
