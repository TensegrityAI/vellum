//! Edit events: the event-sourced foundation (ADR-0002).
//!
//! In Vellum, **an edit is an event**. An [`EditEvent`] is the reified domain
//! event for a single buffer mutation; the buffer's state is the result of
//! applying an ordered sequence of these. Each event carries enough information
//! to be **inverted exactly** — a [`EditEvent::Deleted`] retains the `removed`
//! text — so undo/redo is reverse/replay of events (Task F5), not a separate
//! ad-hoc stack.
//!
//! ## Apply contract
//!
//! [`TextBuffer::apply`](crate::TextBuffer::apply) delegates to the buffer's
//! `insert`/`delete`, which **panic** on out-of-bounds or non-char-boundary
//! offsets (the established `TextBuffer` contract). `apply` is therefore
//! **infallible by contract**: a malformed event (offset past the end, a
//! `removed` byte-length that disagrees with the buffer) is a *programmer error*
//! in `core`, not a recoverable condition, and fails loudly rather than
//! silently corrupting the document.
//!
//! Events fed to `apply` are produced by trusted in-process logic (the
//! `Document` aggregate, the cursor/editor), so they are well-formed by
//! construction. Untrusted input — UTF-16 offsets from the DOM/textarea/
//! `EditContext` — is validated at the **WASM boundary** (Task H1), which is the
//! correct place to convert a contract violation into a `Result`. No `Result`
//! is introduced here; the plan (Task F4) does not call for one.

use crate::buffer::TextBuffer;
use crate::offset::ByteOffset;

/// A single reversible edit to a [`TextBuffer`](crate::TextBuffer).
///
/// Offsets are **byte** offsets (UTF-8), matching the `TextBuffer` contract.
/// `Deleted` stores the exact `removed` text so its inverse re-inserts byte-for
/// byte — this is what makes undo lossless.
///
/// Marked `#[non_exhaustive]`: this enum is `pub` in an OSS-from-commit-one crate
/// (ADR-0005) and is expected to grow (e.g. `SelectionMoved`, future CRDT/OT
/// events per ADR-0002), so downstream matchers must carry a wildcard arm.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum EditEvent {
    /// `text` was inserted at byte offset `at`.
    Inserted {
        /// Byte offset at which `text` was inserted.
        at: ByteOffset,
        /// The inserted text.
        text: String,
    },
    /// `removed` was deleted starting at byte offset `at`.
    Deleted {
        /// Byte offset at which the deletion started.
        at: ByteOffset,
        /// The exact text that was removed (retained for lossless inversion).
        removed: String,
    },
}

impl EditEvent {
    /// The exact inverse of this event.
    ///
    /// Applying an event and then its inverse is a no-op on the buffer
    /// (event-sourcing invariant). The inverse of an insert is the deletion of
    /// the same text at the same offset, and vice versa; `inverse` of `inverse`
    /// is therefore the identity.
    #[must_use]
    pub fn inverse(&self) -> EditEvent {
        match self {
            EditEvent::Inserted { at, text } => EditEvent::Deleted {
                at: *at,
                removed: text.clone(),
            },
            EditEvent::Deleted { at, removed } => EditEvent::Inserted {
                at: *at,
                text: removed.clone(),
            },
        }
    }
}

impl TextBuffer {
    /// Apply an [`EditEvent`] to this buffer.
    ///
    /// `Inserted { at, text }` inserts `text` at byte offset `at`;
    /// `Deleted { at, removed }` deletes the byte range
    /// `at .. at + removed.len()` (where `removed.len()` is the **byte** length,
    /// consistent with the byte-offset contract).
    ///
    /// **Panics** on a malformed event (bad offset, inconsistent `removed`
    /// length): see the apply contract in the [module docs](self). The event is
    /// borrowed because the buffer only needs to read it (the `Document`
    /// aggregate retains ownership for the undo log).
    pub fn apply(&mut self, event: &EditEvent) {
        match event {
            EditEvent::Inserted { at, text } => {
                self.insert(at.get(), text);
            }
            EditEvent::Deleted { at, removed } => {
                let start = at.get();
                self.delete(start..start + removed.len());
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::buffer::TextBuffer;
    use crate::offset::ByteOffset;

    #[test]
    fn apply_inserted_inserts_text() {
        let mut buf = TextBuffer::from_str("Hello");
        buf.apply(&EditEvent::Inserted {
            at: ByteOffset::new(5),
            text: " world".into(),
        });
        assert_eq!(buf.text(), "Hello world");
    }

    #[test]
    fn apply_deleted_removes_text() {
        let mut buf = TextBuffer::from_str("Hello world");
        buf.apply(&EditEvent::Deleted {
            at: ByteOffset::new(5),
            removed: " world".into(),
        });
        assert_eq!(buf.text(), "Hello");
    }

    #[test]
    fn apply_then_apply_inverse_returns_original() {
        // Insert event: apply, then apply inverse → original.
        let mut buf = TextBuffer::from_str("Hello");
        let original = buf.text();
        let insert = EditEvent::Inserted {
            at: ByteOffset::new(5),
            text: " world".into(),
        };
        buf.apply(&insert);
        assert_eq!(buf.text(), "Hello world");
        buf.apply(&insert.inverse());
        assert_eq!(buf.text(), original);

        // Delete event: apply, then apply inverse → original.
        let mut buf = TextBuffer::from_str("Hello world");
        let original = buf.text();
        let delete = EditEvent::Deleted {
            at: ByteOffset::new(5),
            removed: " world".into(),
        };
        buf.apply(&delete);
        assert_eq!(buf.text(), "Hello");
        buf.apply(&delete.inverse());
        assert_eq!(buf.text(), original);
    }

    #[test]
    fn inverse_of_inverse_is_identity() {
        let insert = EditEvent::Inserted {
            at: ByteOffset::new(2),
            text: "xyz".into(),
        };
        assert_eq!(insert.inverse().inverse(), insert);

        let delete = EditEvent::Deleted {
            at: ByteOffset::new(2),
            removed: "xyz".into(),
        };
        assert_eq!(delete.inverse().inverse(), delete);
    }

    #[test]
    fn inverse_inserted_is_deleted() {
        let insert = EditEvent::Inserted {
            at: ByteOffset::new(3),
            text: "abc".into(),
        };
        assert_eq!(
            insert.inverse(),
            EditEvent::Deleted {
                at: ByteOffset::new(3),
                removed: "abc".into(),
            }
        );
    }

    #[test]
    fn inverse_deleted_is_inserted() {
        let delete = EditEvent::Deleted {
            at: ByteOffset::new(3),
            removed: "abc".into(),
        };
        assert_eq!(
            delete.inverse(),
            EditEvent::Inserted {
                at: ByteOffset::new(3),
                text: "abc".into(),
            }
        );
    }

    #[test]
    fn multibyte_insert_apply_inverse_round_trips() {
        // "café" — 'é' is 2 UTF-8 bytes, so `removed.len()` (byte length) is the
        // figure the Deleted inverse must use, not char count.
        let mut buf = TextBuffer::from_str("a b");
        let original = buf.text();
        let insert = EditEvent::Inserted {
            at: ByteOffset::new(2),
            text: "café".into(),
        };
        buf.apply(&insert);
        assert_eq!(buf.text(), "a caféb");
        buf.apply(&insert.inverse());
        assert_eq!(buf.text(), original);
    }

    #[test]
    fn multibyte_delete_apply_inverse_round_trips() {
        // Delete a multibyte run, then re-insert it exactly via the inverse.
        let mut buf = TextBuffer::from_str("x😀y");
        let original = buf.text();
        // "😀" is 4 UTF-8 bytes, occupying byte range 1..5.
        let delete = EditEvent::Deleted {
            at: ByteOffset::new(1),
            removed: "😀".into(),
        };
        buf.apply(&delete);
        assert_eq!(buf.text(), "xy");
        buf.apply(&delete.inverse());
        assert_eq!(buf.text(), original);
    }
}
