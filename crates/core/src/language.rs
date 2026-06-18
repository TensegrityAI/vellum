//! The `Language` port: the typed plugin seam for a language (Jinja2 today,
//! SQL/Markdown later).
//!
//! A language implementation provides syntax tokens (for highlighting),
//! diagnostics (lint), completions (autocomplete), and hover info. The engine is
//! language-agnostic; the language is a typed plugin that plugs into this port
//! (design §4, "Option B" extension spine). `lang-jinja` is the first
//! implementation (extracted into its own crate in Task G2).
//!
//! ## Deviation from the design doc (Phase F audit, finding M3)
//!
//! The design §4 sketch types every method's document argument as `&Rope`:
//! `fn tokenize(&self, doc: &Rope, range: Range) -> Vec<Token>`. Taking `&Rope`
//! here would **leak `ropey` through the language port**, defeating ADR-0006 —
//! whose entire point is that [`TextBuffer`](crate::TextBuffer) hides the backing
//! rope so it can be swapped without caller churn. A language plugin must not be
//! coupled to the storage engine. **This trait therefore takes `&TextBuffer`,
//! never `&Rope`.** `TextBuffer` today does not re-export or leak `ropey`, so the
//! port stays storage-agnostic. (Recorded in the commit message as well.)
//!
//! ## Range-scoped tokenize (forward compatibility with G2)
//!
//! [`tokenize`](Language::tokenize) takes a [`ByteRange`] now, even though the
//! Increment 1 Jinja2 tokenizer ignores it and re-scans the whole document. This
//! is deliberate: Task G2 adds a real range-scoped tokenizer (re-tokenize only a
//! damaged range) **without changing this trait signature**. An Inc-1 trivial
//! impl MAY ignore `range` and tokenize the whole doc — it must document that.

use crate::buffer::TextBuffer;
use crate::offset::{ByteOffset, ByteRange};
use crate::token::Token;

/// The severity of a [`Diagnostic`].
///
/// `#[non_exhaustive]`: this is `pub` in an OSS-from-commit-one crate (ADR-0005)
/// and is expected to grow as diagnostics mature in Increment 2, so downstream
/// matchers must carry a wildcard arm (the [`EditEvent`](crate::EditEvent)
/// `#[non_exhaustive]` precedent).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum Severity {
    /// A hard error (e.g. a syntax error).
    Error,
    /// A warning (valid but suspect).
    Warning,
    /// Informational.
    Info,
    /// A hint (the lowest severity).
    Hint,
}

/// A diagnostic: a message attached to a byte range of the document.
///
/// In Increment 1 the default [`Language::diagnostics`] returns none; real
/// diagnostics arrive in Increment 2. The shape is wired now so language impls
/// and the view can build against it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
    /// The byte span the diagnostic applies to.
    pub range: ByteRange,
    /// The severity of the diagnostic.
    pub severity: Severity,
    /// The human-readable message.
    pub message: String,
}

/// The kind of a [`Completion`], used by the view to pick an icon/affordance.
///
/// `#[non_exhaustive]` for the same reason as [`Severity`]: the set of completion
/// kinds will grow (ADR-0005), so matchers must carry a wildcard arm.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum CompletionKind {
    /// A variable (e.g. a host-injected Jinja2 variable).
    Variable,
    /// A filter (e.g. a Jinja2 `| filter`).
    Filter,
    /// A language keyword.
    Keyword,
    /// A snippet expansion.
    Snippet,
}

/// A completion candidate offered at a byte offset.
///
/// `label` is what the popup shows; `insert_text` is what is committed on accept
/// (they differ when the label is decorated, e.g. `name (variable)`). Increment 1
/// language impls return none by default; completions are filled in Increment 2.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Completion {
    /// The text shown in the completion popup.
    pub label: String,
    /// The text inserted into the buffer when the completion is accepted.
    pub insert_text: String,
    /// The kind of completion (drives the view's icon/affordance).
    pub kind: CompletionKind,
}

/// Hover information for a byte offset: contents plus the span they describe.
///
/// Increment 1 language impls return `None` by default; hover is filled in
/// Increment 2.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hover {
    /// The byte span the hover describes (e.g. the token under the cursor).
    pub range: ByteRange,
    /// The hover contents (Markdown-ish text; rendering is the view's job).
    pub contents: String,
}

/// A language plugin: the typed port a syntax implementation plugs into.
///
/// The engine drives this port to highlight, lint, complete, and explain text.
/// `tokenize` is the only method an Increment 1 implementation must provide; the
/// other three have empty/`None` default impls (Increment 1 stubs, filled in
/// Increment 2).
///
/// All methods take [`&TextBuffer`](crate::TextBuffer), **never** `&Rope` — see
/// the module-level deviation note (Phase F audit M3): a language plugin must not
/// be coupled to the rope storage engine (ADR-0006).
pub trait Language {
    /// Tokenize the document, restricted to `range`.
    ///
    /// The returned [`Token`]s drive syntax highlighting (the CSS Custom Highlight
    /// API at the view layer). An Increment 1 trivial implementation **MAY ignore
    /// `range` and tokenize the whole document** — it must document that it does.
    /// Task G2 introduces a real range-scoped tokenizer (re-tokenize only a damaged
    /// range) *without* changing this signature, which is why `range` is present
    /// now.
    fn tokenize(&self, doc: &TextBuffer, range: ByteRange) -> Vec<Token>;

    /// Diagnostics (lint results) for the document.
    ///
    /// Default: empty (Increment 1 stub). Real diagnostics arrive in Increment 2.
    fn diagnostics(&self, doc: &TextBuffer) -> Vec<Diagnostic> {
        let _ = doc;
        Vec::new()
    }

    /// Completion candidates at byte offset `at`.
    ///
    /// Default: empty (Increment 1 stub). Real completions arrive in Increment 2.
    fn complete(&self, doc: &TextBuffer, at: ByteOffset) -> Vec<Completion> {
        let _ = (doc, at);
        Vec::new()
    }

    /// Hover information at byte offset `at`.
    ///
    /// Default: `None` (Increment 1 stub). Real hover arrives in Increment 2.
    fn hover(&self, doc: &TextBuffer, at: ByteOffset) -> Option<Hover> {
        let _ = (doc, at);
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::buffer::TextBuffer;
    use crate::offset::{ByteOffset, ByteRange};
    use crate::token::{Token, TokenKind};

    /// A trivial in-test [`Language`] proving the trait is usable: it emits a
    /// single `Text` token spanning the requested range and relies on the default
    /// impls for everything else.
    struct TestLang;

    impl Language for TestLang {
        fn tokenize(&self, _doc: &TextBuffer, range: ByteRange) -> Vec<Token> {
            vec![Token {
                start: range.start.get(),
                end: range.end.get(),
                kind: TokenKind::Text,
            }]
        }
    }

    #[test]
    fn trivial_language_tokenizes_a_range() {
        let lang = TestLang;
        let doc = TextBuffer::from_str("hello world");
        let range = ByteRange::new(ByteOffset::new(0), ByteOffset::new(5));

        let tokens = lang.tokenize(&doc, range);

        assert_eq!(
            tokens,
            vec![Token {
                start: 0,
                end: 5,
                kind: TokenKind::Text,
            }]
        );
    }

    #[test]
    fn default_diagnostics_complete_hover_are_empty() {
        let lang = TestLang;
        let doc = TextBuffer::from_str("hello");
        let at = ByteOffset::new(0);

        assert!(lang.diagnostics(&doc).is_empty());
        assert!(lang.complete(&doc, at).is_empty());
        assert_eq!(lang.hover(&doc, at), None);
    }

    #[test]
    fn value_types_construct() {
        let range = ByteRange::new(ByteOffset::new(2), ByteOffset::new(7));

        let diagnostic = Diagnostic {
            range,
            severity: Severity::Error,
            message: "unexpected token".to_owned(),
        };
        assert_eq!(diagnostic.range, range);
        assert_eq!(diagnostic.severity, Severity::Error);
        assert_eq!(diagnostic.message, "unexpected token");

        let completion = Completion {
            label: "name (variable)".to_owned(),
            insert_text: "name".to_owned(),
            kind: CompletionKind::Variable,
        };
        assert_eq!(completion.label, "name (variable)");
        assert_eq!(completion.insert_text, "name");
        assert_eq!(completion.kind, CompletionKind::Variable);

        let hover = Hover {
            range,
            contents: "the variable `name`".to_owned(),
        };
        assert_eq!(hover.range, range);
        assert_eq!(hover.contents, "the variable `name`");

        // Enum variants compare equal to themselves and differ across variants.
        assert_eq!(Severity::Warning, Severity::Warning);
        assert_ne!(Severity::Warning, Severity::Info);
        assert_eq!(CompletionKind::Filter, CompletionKind::Filter);
        assert_ne!(CompletionKind::Filter, CompletionKind::Keyword);
    }
}
