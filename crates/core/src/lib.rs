//! Vellum core — pure, framework-free editor engine. No browser, no WASM here.
#![forbid(unsafe_code)]

mod buffer;
mod lang_jinja;
mod token;

pub use buffer::TextBuffer;
pub use lang_jinja::tokenize;
pub use token::{Token, TokenKind};
