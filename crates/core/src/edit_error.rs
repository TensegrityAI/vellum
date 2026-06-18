//! Typed errors for fallible offset validation at the untrusted boundary.
//!
//! The `TextBuffer` mutators (`insert`/`delete`) and conversions PANIC on a bad
//! offset by design (the established `core` contract: a malformed offset from a
//! *trusted* in-process caller is a programmer error). Untrusted offsets —
//! UTF-16/byte offsets that cross the WASM boundary from the DOM/textarea/
//! `EditContext` — must instead be **validated** and rejected as a recoverable
//! [`Result`], never panic-trap and poison the instance (Increment 1 blocker #3,
//! Task H1).
//!
//! [`EditError`] is the typed validation failure (house signature: `thiserror`,
//! no string errors). The validation helpers that produce it
//! ([`TextBuffer::validate_insert`](crate::TextBuffer::validate_insert),
//! [`TextBuffer::validate_delete`](crate::TextBuffer::validate_delete)) do their
//! checks **without mutating and without panicking**, so the WASM layer (and the
//! H2 `Document` path) can validate *before* applying — guaranteeing no partial
//! apply and no poisoned instance after a rejected op.

/// A recoverable offset-validation failure at the untrusted edit boundary.
///
/// Returned by the non-panicking validation helpers on
/// [`TextBuffer`](crate::TextBuffer); mapped to `JsError` at the WASM boundary
/// (Task H1). Distinct variants so callers can branch on the precise reason an
/// offset was rejected rather than parse a string.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum EditError {
    /// A byte offset lies past the end of the buffer (`offset > len`).
    #[error("byte offset {offset} is out of bounds (len {len})")]
    OutOfBounds {
        /// The offending offset.
        offset: usize,
        /// The buffer length in bytes.
        len: usize,
    },
    /// A byte offset is in bounds but splits a multibyte UTF-8 scalar value.
    #[error("byte offset {offset} is not on a UTF-8 char boundary")]
    NotCharBoundary {
        /// The offending offset.
        offset: usize,
    },
    /// A UTF-16 code-unit offset is in bounds but falls inside a surrogate pair
    /// (i.e. it is not on a scalar-value boundary).
    ///
    /// Surfaced by the non-panicking UTF-16↔byte conversions
    /// ([`TextBuffer::try_utf16_to_byte`](crate::TextBuffer::try_utf16_to_byte)):
    /// the DOM/`EditContext` speak UTF-16, and an untrusted offset that splits an
    /// astral surrogate pair must be rejected as a `Result`, never panic-trapped.
    #[error("utf-16 offset {offset} falls inside a surrogate pair")]
    NotCodeUnitBoundary {
        /// The offending UTF-16 code-unit offset.
        offset: usize,
    },
    /// A delete range has `start > end`.
    #[error("range start {start} is greater than end {end}")]
    InvertedRange {
        /// The range start.
        start: usize,
        /// The range end.
        end: usize,
    },
    /// Computing an offset from `start + len` overflowed `usize` (F-1 defense).
    #[error("offset arithmetic overflowed")]
    Overflow,
}
