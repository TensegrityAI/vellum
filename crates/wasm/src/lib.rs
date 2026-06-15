//! Vellum WASM bindings. The only place `unsafe` may appear in the workspace:
//! wasm-bindgen generates glue code that requires it. The pure `vellum-core`
//! crate stays `#![forbid(unsafe_code)]`; this opt-out is scoped to this crate
//! and applies to wasm-bindgen generated glue ONLY.
#![allow(unsafe_code)]

use vellum_core::{tokenize, TextBuffer};
use wasm_bindgen::prelude::*;

/// JS-facing editor handle wrapping the pure-core [`TextBuffer`].
#[wasm_bindgen]
pub struct Editor {
    buf: TextBuffer,
}

#[wasm_bindgen]
impl Editor {
    /// Construct an editor seeded with `initial` text.
    #[wasm_bindgen(constructor)]
    pub fn new(initial: &str) -> Editor {
        Editor {
            buf: TextBuffer::from_str(initial),
        }
    }

    /// Current buffer text.
    pub fn text(&self) -> String {
        self.buf.text().to_string()
    }

    /// Insert `s` at byte offset `at`. Panics on a non-char-boundary `at`.
    pub fn insert(&mut self, at: usize, s: &str) {
        self.buf.insert(at, s);
    }

    /// Delete the `[start, end)` byte range. Panics on non-char-boundaries.
    pub fn delete(&mut self, start: usize, end: usize) {
        self.buf.delete(start..end);
    }

    /// Tokens flattened as `[start, end, kind, start, end, kind, ...]`.
    /// Crosses to JS as a `Uint32Array`; no serde involved.
    pub fn tokens(&self) -> Vec<u32> {
        tokenize(self.buf.text())
            .iter()
            .flat_map(|t| [t.start as u32, t.end as u32, t.kind as u32])
            .collect()
    }
}
