//! Vellum core — pure, framework-free editor engine. No browser, no WASM here.
#![forbid(unsafe_code)]

mod buffer;
mod cursor;
mod document;
mod edit_error;
mod event;
mod language;
mod offset;
mod token;

pub use buffer::TextBuffer;
pub use cursor::Selection;
pub use document::Document;
pub use edit_error::EditError;
pub use event::EditEvent;
pub use language::{Completion, CompletionKind, Diagnostic, Hover, Language, Severity};
pub use offset::{ByteOffset, ByteRange, CharOffset, Utf16Offset};
pub use token::{Token, TokenKind};
