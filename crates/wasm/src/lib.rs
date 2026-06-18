//! Vellum WASM bindings. The only place `unsafe` may appear in the workspace:
//! wasm-bindgen generates glue code that requires it. The pure `vellum-core`
//! crate stays `#![forbid(unsafe_code)]`; this opt-out is scoped to this crate
//! via its `Cargo.toml` (`[lints.rust] unsafe_code = "allow"`) — a crate-level
//! `#![allow]` cannot relax the workspace `forbid` (E0453), so the lint lives in
//! the manifest, not here.
//!
//! Increment-1 follow-up (tracked): `insert`/`delete` forward to core methods
//! that PANIC on out-of-bounds / non-char-boundary offsets. At the WASM boundary
//! a panic traps and poisons this `Editor` instance (later calls also trap).
//! Increment 1 should validate offsets and return `Result<_, JsError>` instead.

use vellum_core::{ByteOffset, ByteRange, Language, TextBuffer};
use vellum_lang_jinja::Jinja;
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
        self.buf.text()
    }

    /// Insert `s` at byte offset `at`. Panics on a non-char-boundary `at`.
    pub fn insert(&mut self, at: usize, s: &str) {
        self.buf.insert(at, s);
    }

    /// Delete the `[start, end)` byte range. Panics on non-char-boundaries.
    pub fn delete(&mut self, start: usize, end: usize) {
        // TODO(H2): route mutations through the Document aggregate
        // (Document::delete(ByteRange)) so undo/redo + the typed front door apply
        // to the JS boundary; consider a ByteRange::from_raw(usize,usize)
        // convenience ctor then.
        self.buf.delete(start..end);
    }

    /// Tokens flattened as `[start, end, kind, start, end, kind, ...]`.
    /// Crosses to JS as a `Uint32Array`; no serde involved.
    ///
    /// Drives the [`Jinja`] language plugin (extracted to `vellum-lang-jinja` in
    /// Task G2) over the **whole document** (`0..len`); `core` no longer knows
    /// about Jinja2. The whole-doc range produces byte-for-byte the same tokens
    /// the old in-core `tokenize(&buf.text())` did, so the wire is unchanged.
    pub fn tokens(&self) -> Vec<u32> {
        let whole = ByteRange::new(ByteOffset::new(0), ByteOffset::new(self.buf.len()));
        Jinja
            .tokenize(&self.buf, whole)
            .iter()
            .flat_map(|t| [t.start as u32, t.end as u32, t.kind as u32])
            .collect()
    }
}
