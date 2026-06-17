//! Vellum core — pure, framework-free editor engine. No browser, no WASM here.
#![forbid(unsafe_code)]

mod buffer;
mod cursor;
mod document;
mod event;
mod lang_jinja;
mod offset;
mod token;

pub use buffer::TextBuffer;
pub use cursor::Selection;
pub use document::Document;
pub use event::EditEvent;
pub use lang_jinja::tokenize;
pub use offset::{ByteOffset, CharOffset, Utf16Offset};
pub use token::{Token, TokenKind};
