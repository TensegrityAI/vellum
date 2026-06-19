//! `vellum-lang-jinja` — the Jinja2 [`Language`] plugin for Vellum.
//!
//! This crate was extracted from `vellum-core` in Task G2 so that `core` no
//! longer knows about any concrete language — it owns only the [`Language`] port
//! and the [`Token`] value types. Jinja2 (and, later, SQL/Markdown) live in their
//! own crates that depend on `core` and plug into the port (design §4 / ADR-0007).
//!
//! The public surface is the unit struct [`Jinja`], which `impl`s [`Language`].
//! The underlying byte scanner lives in [`tokenizer::tokenize`] as a free
//! `&str -> Vec<Token>` function (the exact scanner that used to live inline in
//! `core`), so it can be unit-tested in isolation.
//!
//! ## Range-scoped tokenize (Increment 1)
//!
//! [`Language::tokenize`] takes a [`ByteRange`] and, per the Increment 1 plan,
//! re-tokenizes **only that range** rather than the whole document. The approach
//! is pragmatic and correct for Inc 1:
//!
//! 1. Slice the `range` out of the buffer with [`TextBuffer::slice`] (rope
//!    `byte_slice`, not a whole-document materialization).
//! 2. Run the whole-string [`tokenizer::tokenize`] scanner on the slice.
//! 3. **Offset** every returned token's `start`/`end` by `range.start` so the
//!    tokens are returned in **whole-document** byte coordinates, not relative to
//!    the slice (skipped when `range.start == 0`, where it is the identity).
//!
//! ### Contract / limitations
//!
//! - **Char boundaries.** `range`'s bounds must lie on UTF-8 char boundaries.
//!   Slicing a `&str` at a non-boundary **panics** — this matches the
//!   [`TextBuffer`] offset contract (a non-char-boundary offset is a programmer
//!   error), so it is surfaced as a panic here too rather than silently fixed up.
//! - **Block-split at range edges (Inc 1 known limitation).** A naive slice can
//!   cut a `{{ … }}` block in half at a range edge, which changes the
//!   tokenization of that edge versus scanning the whole document (the partial
//!   block tokenizes as text, or an unterminated block runs to the slice end).
//!   For Increment 1 this is **acceptable**: the view passes sensible,
//!   block-aligned ranges, and a whole-document tokenize (`range == 0..len`,
//!   which is what the WASM binding sends) is byte-for-byte identical to the old
//!   in-core behavior. True incremental re-lexing with damaged-range / block
//!   boundary expansion is an Increment 2 concern and is deliberately **not**
//!   implemented here.

#![forbid(unsafe_code)]

pub mod tokenizer;

pub use tokenizer::tokenize;

use vellum_core::{ByteRange, Language, TextBuffer, Token};

/// The Jinja2 language plugin: a stateless unit struct implementing [`Language`].
///
/// Construct it as `Jinja` and drive it through the [`Language`] port:
///
/// ```
/// use vellum_core::{ByteOffset, ByteRange, Language, TextBuffer};
/// use vellum_lang_jinja::Jinja;
///
/// let doc = TextBuffer::from_str("a {{ x }}");
/// let whole = ByteRange::new(ByteOffset::new(0), ByteOffset::new(doc.len()));
/// let tokens = Jinja.tokenize(&doc, whole);
/// assert_eq!(tokens.len(), 2); // "a " text, then the `{{ x }}` variable block
/// ```
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Jinja;

impl Language for Jinja {
    /// Tokenize `doc`, restricted to `range`.
    ///
    /// Re-tokenizes only the requested `range`: it slices the range out of the
    /// buffer (via [`TextBuffer::slice`], rope `byte_slice` — not a whole-document
    /// materialization, P4), runs the [`tokenize`] scanner on that slice, then
    /// re-offsets the resulting tokens by `range.start` so they are in
    /// **whole-document** byte coordinates. See the crate-level docs for the
    /// char-boundary requirement and the Inc-1 block-split-at-edges limitation.
    ///
    /// When `range.start == 0` (the whole-document path the WASM binding sends)
    /// the re-offset is the identity, so the scanner's tokens are returned
    /// directly without allocating a second `Vec` (P4).
    ///
    /// # Panics
    ///
    /// Panics if `range`'s bounds are not on UTF-8 char boundaries (the slice
    /// panics), matching the [`TextBuffer`] offset contract.
    fn tokenize(&self, doc: &TextBuffer, range: ByteRange) -> Vec<Token> {
        let start = range.start.get();
        let slice = doc.slice(range.get());
        let tokens = tokenize(&slice);
        if start == 0 {
            return tokens;
        }
        tokens
            .into_iter()
            .map(|t| Token {
                start: t.start + start,
                end: t.end + start,
                kind: t.kind,
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use vellum_core::{ByteOffset, ByteRange, HighlightKind, Language, TextBuffer};

    /// Whole-document convenience: the range every whole-doc caller (e.g. WASM)
    /// passes — `0..doc.len()`.
    fn whole_doc(doc: &TextBuffer) -> ByteRange {
        ByteRange::new(ByteOffset::new(0), ByteOffset::new(doc.len()))
    }

    #[test]
    fn whole_doc_range_matches_legacy_scan() {
        // The Inc-0 in-core behavior was `tokenize(&buf.text())`. A whole-document
        // `Language::tokenize` must be byte-for-byte identical to that.
        let text = "café {{ x }} {% if y %} tail {# c #}";
        let doc = TextBuffer::from_str(text);

        let via_language = Jinja.tokenize(&doc, whole_doc(&doc));
        let via_scanner = tokenize(text);

        assert_eq!(via_language, via_scanner);
        // And it genuinely covers the whole document, gap-free.
        assert_eq!(via_language.first().unwrap().start, 0);
        assert_eq!(via_language.last().unwrap().end, text.len());
    }

    #[test]
    fn sub_range_tokens_are_in_document_coordinates() {
        // Document: "prefix {{ x }}". Tokenize only the trailing block, which
        // starts at byte 7, NOT from byte 0. The returned offsets must be
        // absolute (offset by range.start), not relative to the sliced substring.
        let doc = TextBuffer::from_str("prefix {{ x }}");
        let start = "prefix ".len(); // 7
        let range = ByteRange::new(ByteOffset::new(start), ByteOffset::new(doc.len()));

        let tokens = Jinja.tokenize(&doc, range);

        // If offsets were slice-relative the block would start at 0; absolute it
        // starts at 7.
        assert_eq!(
            tokens,
            vec![Token {
                start: 7,
                end: 14,
                kind: HighlightKind::Variable,
            }]
        );
    }

    #[test]
    fn clean_subrange_containing_a_block_tokenizes_with_absolute_offsets() {
        // A range that cleanly contains a `{{ x }}` block (no edge split): the
        // block is a Variable token with correct ABSOLUTE offsets.
        let doc = TextBuffer::from_str("aa {{ x }} bb");
        // Slice exactly the block: bytes 3..10 == "{{ x }}".
        let block_start = 3;
        let block_end = 10;
        assert_eq!(&doc.text()[block_start..block_end], "{{ x }}");
        let range = ByteRange::new(ByteOffset::new(block_start), ByteOffset::new(block_end));

        let tokens = Jinja.tokenize(&doc, range);

        assert_eq!(
            tokens,
            vec![Token {
                start: 3,
                end: 10,
                kind: HighlightKind::Variable,
            }]
        );
    }

    #[test]
    fn default_diagnostics_complete_hover_are_empty() {
        // Jinja relies on the Inc-1 default stubs for the other three methods.
        let doc = TextBuffer::from_str("{{ x }}");
        let at = ByteOffset::new(0);
        assert!(Jinja.diagnostics(&doc).is_empty());
        assert!(Jinja.complete(&doc, at).is_empty());
        assert_eq!(Jinja.hover(&doc, at), None);
    }

    #[test]
    fn empty_document_yields_no_tokens() {
        let doc = TextBuffer::new();
        assert_eq!(Jinja.tokenize(&doc, whole_doc(&doc)), vec![]);
    }
}
