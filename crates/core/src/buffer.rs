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
}
