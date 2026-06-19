//! Vellum WASM bindings. The only place `unsafe` may appear in the workspace:
//! wasm-bindgen generates glue code that requires it. The pure `vellum-core`
//! crate stays `#![forbid(unsafe_code)]`; this opt-out is scoped to this crate
//! via its `Cargo.toml` (`[lints.rust] unsafe_code = "allow"`) — a crate-level
//! `#![allow]` cannot relax the workspace `forbid` (E0453), so the lint lives in
//! the manifest, not here.
//!
//! ## Increment-1 (Task H2, DONE): the `Editor` wraps the [`Document`] aggregate
//!
//! H1 wrapped a bare [`TextBuffer`] and validated offsets so `insert`/`delete`
//! returned `Result` instead of trapping. H2 swaps the field to the
//! [`Document`] aggregate, so every mutation now flows through the event-sourced
//! write side: `insert`/`delete` record history, and `undo()`/`redo()` work at
//! the JS boundary (the win H1's `TODO(H2)` pointed to). The fallible H1 contract
//! is preserved — untrusted offsets are still VALIDATED before mutating, so a
//! rejected op returns `Err` WITHOUT mutating and never poisons the instance.
//!
//! ## No traps across the boundary (Phase H acceptance)
//!
//! Every JS-callable method that takes an untrusted offset is NON-trapping:
//!
//! - `insert`/`delete`/`set_caret`/`set_selection` VALIDATE via the buffer's
//!   non-panicking guards (`validate_insert`/`validate_delete`/`is_char_boundary`)
//!   and return `Result`, mapping [`EditError`] → `JsError`.
//! - `utf16_to_byte`/`byte_to_utf16` call the buffer's non-panicking `try_*`
//!   conversions, returning `Result` (a mid-surrogate or OOB offset → `Err`).
//! - The cursor movers (`move_*`/`extend_*`/`collapse_selection`) and the
//!   selection-aware edits (`insert_at_cursor`/`backspace`/`delete_forward`/
//!   `delete_selection`) cannot trap: the aggregate clamps the internal caret to
//!   a char boundary after every edit, so the grapheme primitives never see a
//!   mid-codepoint offset.

use vellum_core::{ByteOffset, ByteRange, Document, EditError, Language, Selection};
use vellum_lang_jinja::Jinja;
use wasm_bindgen::prelude::*;

/// JS-facing editor handle wrapping the pure-core [`Document`] aggregate.
///
/// The `Document` owns the rope buffer, the undo/redo history, and the single
/// text selection. The `Editor` is a thin, untrusted-boundary shim: it converts
/// bare `usize` offsets from JS into the typed `core` offsets, validating each
/// one so a bad value crosses back as a rejected `Result`, never a trap.
#[wasm_bindgen]
pub struct Editor {
    doc: Document,
}

#[wasm_bindgen]
impl Editor {
    /// Construct an editor seeded with `initial` text (empty history, caret at 0).
    #[wasm_bindgen(constructor)]
    pub fn new(initial: &str) -> Editor {
        Editor {
            doc: Document::from_str(initial),
        }
    }

    /// Current document text.
    pub fn text(&self) -> String {
        self.doc.text()
    }

    /// Insert `s` at byte offset `at`, recording history.
    ///
    /// Validates `at` (in bounds + UTF-8 char boundary) via the buffer's
    /// non-panicking guard BEFORE mutating, so a bad offset returns `Err(JsError)`
    /// without touching the document — the instance stays usable. The mutation
    /// goes through [`Document::insert`], so it is undoable.
    pub fn insert(&mut self, at: usize, s: &str) -> Result<(), JsError> {
        self.doc
            .buffer()
            .validate_insert(at)
            .map_err(edit_error_to_js)?;
        self.doc.insert(ByteOffset::new(at), s);
        Ok(())
    }

    /// Delete the `[start, end)` byte range, recording history.
    ///
    /// Validates the range (ordered, in bounds, both bounds on char boundaries)
    /// via the buffer's non-panicking guard BEFORE mutating, returning
    /// `Err(JsError)` on a bad range without touching the document. The mutation
    /// goes through [`Document::delete`], so it is undoable.
    pub fn delete(&mut self, start: usize, end: usize) -> Result<(), JsError> {
        self.doc
            .buffer()
            .validate_delete(start, end)
            .map_err(edit_error_to_js)?;
        self.doc.delete(ByteRange::from_raw(start, end));
        Ok(())
    }

    /// Tokens flattened as `[start, end, kind, start, end, kind, ...]`.
    /// Crosses to JS as a `Uint32Array`; no serde involved.
    ///
    /// Drives the [`Jinja`] language plugin over the **whole document**
    /// (`0..len`); the flat-`u32` wire is unchanged from H1.
    pub fn tokens(&self) -> Vec<u32> {
        let whole = ByteRange::from_raw(0, self.doc.len());
        Jinja
            .tokenize(self.doc.buffer(), whole)
            .iter()
            .flat_map(|t| [t.start as u32, t.end as u32, t.kind as u32])
            .collect()
    }

    // --- Undo / redo ------------------------------------------------------

    /// Undo the most recent edit. Returns `true` if an edit was undone.
    pub fn undo(&mut self) -> bool {
        self.doc.undo()
    }

    /// Redo the most recently undone edit. Returns `true` if an edit was redone.
    pub fn redo(&mut self) -> bool {
        self.doc.redo()
    }

    /// Whether there is an edit available to [`undo`](Self::undo).
    pub fn can_undo(&self) -> bool {
        self.doc.can_undo()
    }

    /// Whether there is an edit available to [`redo`](Self::redo).
    pub fn can_redo(&self) -> bool {
        self.doc.can_redo()
    }

    // --- Cursor: read -----------------------------------------------------

    /// The selection's `anchor` (fixed end) as a byte offset.
    ///
    /// wasm-bindgen cannot return the `Selection` struct directly without more
    /// glue, so the two ends are exposed as bare offsets; the view reconstructs
    /// the selection from `cursor_anchor`/`cursor_head`.
    pub fn cursor_anchor(&self) -> usize {
        self.doc.selection().anchor().get()
    }

    /// The selection's `head` (moving caret) as a byte offset.
    pub fn cursor_head(&self) -> usize {
        self.doc.selection().head().get()
    }

    // --- Cursor: set (validated) ------------------------------------------

    /// Collapse the selection to a bare caret at byte offset `at`.
    ///
    /// Validates `at` is in bounds AND on a char boundary via the buffer's
    /// non-panicking guard, returning `Err(JsError)` otherwise — JS can never
    /// push a mid-codepoint caret into the aggregate.
    pub fn set_caret(&mut self, at: usize) -> Result<(), JsError> {
        if !self.doc.buffer().is_char_boundary(at) {
            return Err(boundary_error_to_js(at, self.doc.len()));
        }
        self.doc.set_caret(ByteOffset::new(at));
        Ok(())
    }

    /// Replace the selection with `[anchor, head]` (both byte offsets).
    ///
    /// Validates BOTH ends are in bounds AND on char boundaries before applying,
    /// returning `Err(JsError)` on the first bad end. The order of `anchor`/`head`
    /// is preserved (a reversed selection is allowed).
    pub fn set_selection(&mut self, anchor: usize, head: usize) -> Result<(), JsError> {
        let buf = self.doc.buffer();
        if !buf.is_char_boundary(anchor) {
            return Err(boundary_error_to_js(anchor, self.doc.len()));
        }
        if !buf.is_char_boundary(head) {
            return Err(boundary_error_to_js(head, self.doc.len()));
        }
        self.doc.set_selection(Selection::new(
            ByteOffset::new(anchor),
            ByteOffset::new(head),
        ));
        Ok(())
    }

    // --- Cursor: movers (never trap — core clamps to boundaries) ----------

    /// Move the caret one grapheme left (collapse-or-move).
    pub fn move_left(&mut self) {
        self.doc.move_left();
    }

    /// Move the caret one grapheme right (collapse-or-move).
    pub fn move_right(&mut self) {
        self.doc.move_right();
    }

    /// Extend the selection one grapheme left (move only `head`).
    pub fn extend_left(&mut self) {
        self.doc.extend_left();
    }

    /// Extend the selection one grapheme right (move only `head`).
    pub fn extend_right(&mut self) {
        self.doc.extend_right();
    }

    /// Move the caret to the previous word boundary, collapsing the selection.
    pub fn move_word_left(&mut self) {
        self.doc.move_word_left();
    }

    /// Move the caret to the next word boundary, collapsing the selection.
    pub fn move_word_right(&mut self) {
        self.doc.move_word_right();
    }

    /// Extend the selection to the previous word boundary (move only `head`).
    pub fn extend_word_left(&mut self) {
        self.doc.extend_word_left();
    }

    /// Extend the selection to the next word boundary (move only `head`).
    pub fn extend_word_right(&mut self) {
        self.doc.extend_word_right();
    }

    /// Drop the selection, keeping the caret at `head`.
    pub fn collapse_selection(&mut self) {
        self.doc.collapse_selection();
    }

    // --- Selection-aware editing (infallible — core clamps the caret) -----

    /// Type `text` at the cursor (deletes a non-empty selection first), recording
    /// history. Infallible: the caret is core-clamped, so this cannot trap.
    pub fn insert_at_cursor(&mut self, s: &str) {
        self.doc.insert_at_cursor(s);
    }

    /// Delete the current selection. Returns `true` if a non-empty selection was
    /// removed, `false` if the selection was empty.
    pub fn delete_selection(&mut self) -> bool {
        self.doc.delete_selection()
    }

    /// Backspace: delete the selection if non-empty, else one grapheme left.
    /// Returns `true` if anything was removed.
    pub fn backspace(&mut self) -> bool {
        self.doc.backspace()
    }

    /// Forward-delete: delete the selection if non-empty, else one grapheme
    /// right. Returns `true` if anything was removed.
    pub fn delete_forward(&mut self) -> bool {
        self.doc.delete_forward()
    }

    // --- Offset conversions (untrusted UTF-16 ↔ byte; never trap) ---------

    /// Convert a UTF-16 code-unit offset (DOM space) to a byte offset (core
    /// space).
    ///
    /// The view passes UTF-16 offsets from the DOM, which are untrusted; this
    /// routes through the buffer's non-panicking `try_utf16_to_byte`, so an
    /// out-of-range or mid-surrogate offset returns `Err(JsError)` instead of
    /// trapping. This is what I2's diff-based mutation uses to convert DOM
    /// offsets before touching the buffer.
    pub fn utf16_to_byte(&self, u: usize) -> Result<usize, JsError> {
        self.doc
            .buffer()
            .try_utf16_to_byte(u)
            .map_err(edit_error_to_js)
    }

    /// Convert a byte offset (core space) to a UTF-16 code-unit offset (DOM
    /// space).
    ///
    /// The inverse of [`utf16_to_byte`](Self::utf16_to_byte), via the buffer's
    /// non-panicking `try_byte_to_utf16`: an out-of-range or mid-codepoint byte
    /// offset returns `Err(JsError)` instead of trapping.
    pub fn byte_to_utf16(&self, b: usize) -> Result<usize, JsError> {
        self.doc
            .buffer()
            .try_byte_to_utf16(b)
            .map_err(edit_error_to_js)
    }
}

/// Map a core [`EditError`] to a wasm-bindgen [`JsError`] (a JS `Error`).
///
/// The typed core error becomes a JS `Error` carrying its `Display` message, so
/// the JS caller gets a thrown `Error` (rejected `Result`) instead of a trap.
fn edit_error_to_js(err: EditError) -> JsError {
    JsError::new(&err.to_string())
}

/// Build the `JsError` for a caret/selection offset that is out of bounds or not
/// on a char boundary, reusing the typed [`EditError`] messages so the JS error
/// text is consistent with the validation path.
fn boundary_error_to_js(at: usize, len: usize) -> JsError {
    let err = if at > len {
        EditError::OutOfBounds { offset: at, len }
    } else {
        EditError::NotCharBoundary { offset: at }
    };
    edit_error_to_js(err)
}
