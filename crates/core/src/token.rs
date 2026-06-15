/// The kind of a tokenized span. The discriminants are the stable WASM wire
/// contract (see Phase C): they cross to JS as `u32` and must not be reordered.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum TokenKind {
    /// Literal text outside any Jinja2 block.
    Text = 0,
    /// A `{{ ... }}` expression block.
    Variable = 1,
    /// A `{% ... %}` statement block.
    Statement = 2,
    /// A `{# ... #}` comment block.
    Comment = 3,
}

/// A half-open byte span `[start, end)` tagged with its [`TokenKind`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Token {
    /// Inclusive start byte offset.
    pub start: usize,
    /// Exclusive end byte offset.
    pub end: usize,
    /// The kind of span.
    pub kind: TokenKind,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn token_kind_maps_to_stable_u32() {
        assert_eq!(TokenKind::Text as u32, 0);
        assert_eq!(TokenKind::Variable as u32, 1);
        assert_eq!(TokenKind::Statement as u32, 2);
        assert_eq!(TokenKind::Comment as u32, 3);
    }
}
