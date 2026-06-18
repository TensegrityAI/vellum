//! Vellum WASM bindings. The only place `unsafe` may appear in the workspace:
//! wasm-bindgen generates glue code that requires it. The pure `vellum-core`
//! crate stays `#![forbid(unsafe_code)]`; this opt-out is scoped to this crate
//! via its `Cargo.toml` (`[lints.rust] unsafe_code = "allow"`) — a crate-level
//! `#![allow]` cannot relax the workspace `forbid` (E0453), so the lint lives in
//! the manifest, not here.
//!
//! Increment-1 (Task H1, blocker #3, DONE): `insert`/`delete` no longer forward
//! blindly to core methods that PANIC on out-of-bounds / non-char-boundary
//! offsets. They VALIDATE the (untrusted) offset first via the non-panicking
//! [`TextBuffer::validate_insert`]/[`TextBuffer::validate_delete`] guards and
//! return `Result<(), JsError>`. Validation precedes mutation, so a rejected op
//! returns `Err` WITHOUT mutating — the instance is never partially applied and
//! never poisoned; the next call works normally.

use vellum_core::{ByteOffset, ByteRange, EditError, Language, TextBuffer};
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

    /// Insert `s` at byte offset `at`.
    ///
    /// Validates `at` (in bounds + UTF-8 char boundary) BEFORE mutating, so a bad
    /// offset returns `Err(JsError)` without touching the buffer — the instance
    /// stays usable. The post-validation `insert` cannot panic.
    pub fn insert(&mut self, at: usize, s: &str) -> Result<(), JsError> {
        self.buf.validate_insert(at).map_err(edit_error_to_js)?;
        self.buf.insert(at, s);
        Ok(())
    }

    /// Delete the `[start, end)` byte range.
    ///
    /// Validates the range (ordered, in bounds, both bounds on char boundaries)
    /// BEFORE mutating, returning `Err(JsError)` on a bad range without touching
    /// the buffer. The post-validation `delete` cannot panic.
    pub fn delete(&mut self, start: usize, end: usize) -> Result<(), JsError> {
        // TODO(H2): route mutations through the Document aggregate
        // (Document::delete(ByteRange)) so undo/redo + the typed front door apply
        // to the JS boundary; consider a ByteRange::from_raw(usize,usize)
        // convenience ctor then.
        self.buf
            .validate_delete(start, end)
            .map_err(edit_error_to_js)?;
        self.buf.delete(start..end);
        Ok(())
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

/// Map a core [`EditError`] to a wasm-bindgen [`JsError`] (a JS `Error`).
///
/// The typed core error becomes a JS `Error` carrying its `Display` message, so
/// the JS caller gets a thrown `Error` (rejected `Result`) instead of a trap.
/// Validation runs before any mutation, so producing this error never leaves the
/// `Editor` in a partially-applied state.
fn edit_error_to_js(err: EditError) -> JsError {
    JsError::new(&err.to_string())
}
