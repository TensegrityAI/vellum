/// A generic, language-agnostic syntax-highlight scope (ADR-0009).
///
/// This is the **palette** of the [`Language`](crate::Language) port: every
/// language plugin is an adapter that maps its grammar onto these scopes, modelled
/// on the LSP `SemanticTokenTypes` / TextMate scope conventions. `core` owns the
/// vocabulary; it knows nothing about any concrete language (the variant names are
/// generic scopes, not e.g. Jinja block kinds).
///
/// The discriminants are the stable WASM wire contract: they cross to JS as `u32`
/// (the flat `[start, end, kind, …]` token array) and **must not be reordered**.
/// New scopes append at the end; the enum is `#[non_exhaustive]` so adding them is
/// non-breaking on both the Rust and wire sides.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
#[non_exhaustive]
pub enum HighlightKind {
    /// Literal text outside any highlighted construct.
    Text = 0,
    /// A variable / identifier / interpolated value (e.g. a template `{{ x }}`).
    Variable = 1,
    /// A keyword / control construct / tag (e.g. a template `{% if %}`).
    Keyword = 2,
    /// A comment.
    Comment = 3,
    /// A string literal.
    String = 4,
    /// A numeric literal.
    Number = 5,
    /// An operator.
    Operator = 6,
    /// A function or callable name.
    Function = 7,
    /// A type name.
    Type = 8,
    /// Punctuation / delimiters.
    Punctuation = 9,
}

/// A half-open byte span `[start, end)` tagged with its [`HighlightKind`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Token {
    /// Inclusive start byte offset.
    pub start: usize,
    /// Exclusive end byte offset.
    pub end: usize,
    /// The highlight scope of the span.
    pub kind: HighlightKind,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn highlight_kind_maps_to_stable_u32_wire() {
        // Discriminants 0..=3 are byte-identical to the pre-ADR-0009 TokenKind
        // (the former `Statement` is now `Keyword=2`), so the wire is unchanged.
        assert_eq!(HighlightKind::Text as u32, 0);
        assert_eq!(HighlightKind::Variable as u32, 1);
        assert_eq!(HighlightKind::Keyword as u32, 2);
        assert_eq!(HighlightKind::Comment as u32, 3);
    }

    #[test]
    fn generic_scopes_have_distinct_appended_discriminants() {
        // The generic palette beyond the original four appends at 4.. so old
        // wire values are preserved and new scopes are non-breaking additions.
        assert_eq!(HighlightKind::String as u32, 4);
        assert_eq!(HighlightKind::Number as u32, 5);
        assert_eq!(HighlightKind::Operator as u32, 6);
        assert_eq!(HighlightKind::Function as u32, 7);
        assert_eq!(HighlightKind::Type as u32, 8);
        assert_eq!(HighlightKind::Punctuation as u32, 9);
    }
}
