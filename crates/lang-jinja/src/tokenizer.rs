//! The trivial Jinja2 byte scanner, extracted from `core` in Task G2.
//!
//! This is the underlying scanner the [`Jinja`](crate::Jinja) language plugin
//! drives. It is a free function over `&str` (the shape the `core` module had)
//! so it can be tested in isolation; [`Language::tokenize`](vellum_core::Language::tokenize)
//! slices a range out of the document, runs this scanner, and re-offsets the
//! result into whole-document coordinates (see [`crate`] docs).

use vellum_core::{Token, TokenKind};

/// Tokenize Jinja2-flavored `input` in a single O(n) left-to-right byte scan.
///
/// Recognizes `{{ }}` (variable), `{% %}` (statement), and `{# #}` (comment)
/// blocks, emitting [`TokenKind::Text`] spans for everything in between. A block
/// whose closing delimiter is missing runs to end-of-input. Spans are half-open
/// byte ranges; a block's `end` is the byte just past its closing delimiter.
///
/// This is a deliberately *trivial* tokenizer (Increment 0/1): delimiters inside
/// a block body are **not** string-literal- or escape-aware, so `{{ "}}" }}` ends
/// at the first inner `}}`. A real grammar arrives in Increment 2. The spans it
/// emits are guaranteed gap-free, non-overlapping, and aligned to UTF-8 char
/// boundaries — the contract the WASM token wire relies on.
pub fn tokenize(input: &str) -> Vec<Token> {
    let bytes = input.as_bytes();
    let len = bytes.len();
    let mut tokens = Vec::new();
    let mut i = 0;
    let mut text_start = 0;

    while i < len {
        if bytes[i] == b'{' && i + 1 < len {
            let (kind, close) = match bytes[i + 1] {
                b'{' => (TokenKind::Variable, b'}'),
                b'%' => (TokenKind::Statement, b'%'),
                b'#' => (TokenKind::Comment, b'#'),
                _ => {
                    i += 1;
                    continue;
                }
            };

            // Flush any pending text before this block.
            if text_start < i {
                tokens.push(Token {
                    start: text_start,
                    end: i,
                    kind: TokenKind::Text,
                });
            }

            let block_start = i;
            i += 2; // past the opening delimiter
            let mut end = len; // unterminated: run to end-of-input
            while i + 1 < len {
                if bytes[i] == close && bytes[i + 1] == b'}' {
                    end = i + 2; // just past the closing delimiter
                    break;
                }
                i += 1;
            }

            tokens.push(Token {
                start: block_start,
                end,
                kind,
            });
            i = end;
            text_start = end;
        } else {
            i += 1;
        }
    }

    if text_start < len {
        tokens.push(Token {
            start: text_start,
            end: len,
            kind: TokenKind::Text,
        });
    }

    tokens
}

#[cfg(test)]
mod tests {
    use super::*;
    use vellum_core::TokenKind;

    #[test]
    fn plain_text_is_one_text_token() {
        let toks = tokenize("hello");
        assert_eq!(
            toks,
            vec![Token {
                start: 0,
                end: 5,
                kind: TokenKind::Text
            }]
        );
    }

    #[test]
    fn variable_block_is_tokenized() {
        let toks = tokenize("a {{ x }} b");
        assert_eq!(
            toks,
            vec![
                Token {
                    start: 0,
                    end: 2,
                    kind: TokenKind::Text
                },
                Token {
                    start: 2,
                    end: 9,
                    kind: TokenKind::Variable
                },
                Token {
                    start: 9,
                    end: 11,
                    kind: TokenKind::Text
                },
            ]
        );
    }

    #[test]
    fn statement_and_comment_blocks() {
        assert_eq!(tokenize("{% if x %}")[0].kind, TokenKind::Statement);
        assert_eq!(tokenize("{# c #}")[0].kind, TokenKind::Comment);
    }

    #[test]
    fn empty_input_yields_no_tokens() {
        assert_eq!(tokenize(""), vec![]);
    }

    #[test]
    fn adjacent_blocks_have_no_gap() {
        let toks = tokenize("{{a}}{%b%}");
        assert_eq!(
            toks,
            vec![
                Token {
                    start: 0,
                    end: 5,
                    kind: TokenKind::Variable
                },
                Token {
                    start: 5,
                    end: 10,
                    kind: TokenKind::Statement
                },
            ]
        );
    }

    #[test]
    fn multibyte_text_around_block_stays_on_char_boundaries() {
        // "café " is 6 bytes (é = 2), block then trailing multibyte.
        let toks = tokenize("café {{x}} ☕");
        // Spans must be gap-free and cover the whole input.
        assert_eq!(toks.first().unwrap().start, 0);
        assert_eq!(toks.last().unwrap().end, "café {{x}} ☕".len());
        for pair in toks.windows(2) {
            assert_eq!(pair[0].end, pair[1].start, "no gaps between spans");
        }
    }

    #[test]
    fn unterminated_block_runs_to_end() {
        let toks = tokenize("{{ x");
        assert_eq!(
            toks,
            vec![Token {
                start: 0,
                end: 4,
                kind: TokenKind::Variable
            }]
        );
    }
}
