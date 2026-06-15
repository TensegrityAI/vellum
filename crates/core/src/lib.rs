//! Vellum core — pure, framework-free editor engine. No browser, no WASM here.
#![forbid(unsafe_code)]

mod buffer;
mod token;

pub use buffer::TextBuffer;
pub use token::{Token, TokenKind};
