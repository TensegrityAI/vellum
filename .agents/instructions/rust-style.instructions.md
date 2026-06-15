---
description: 'Rust conventions for the Vellum core, lang-* plugins, and wasm bindings.'
applyTo: 'crates/**/*.rs'
---

# Rust style

Conventions for all Rust in the workspace. These are enforced by CI
(`cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`) and by review.

## Safety

- **`#![forbid(unsafe_code)]` in `core` and every `lang-*` crate.** No exceptions.
  An unsafe-free editor core is a deliberate selling point (ADR design §7).
- The **only** crate where `unsafe` may appear is `wasm`, and only because
  `wasm-bindgen` generates it. Do not write hand-rolled `unsafe`.

## Errors

- **Typed errors with `thiserror`.** Every fallible public operation returns a
  domain error enum, never a `String` or `Box<dyn Error>` at API boundaries.
- Do not `panic!` for recoverable conditions. Documented invariant violations
  (e.g. a non-char-boundary byte offset) may panic, but the contract must be
  stated in the doc comment.
- No `unwrap()` / `expect()` on fallible paths in non-test code unless an invariant
  is locally proven and commented.

## Completeness

- **No `todo!()` or `unimplemented!()` in merged code.** Do not leave stubs that
  claim functionality which is not wired. Land smaller, complete slices instead.
- Public API carries **doc comments** (`///`) explaining behavior, panics, and any
  invariants the caller must uphold.

## Naming and shape

- **Behavior-driven names.** Functions and tests say what happens, not how
  (`insert_at`, `delete_range`; tests like `insert_on_non_char_boundary_panics`).
- Prefer small, pure functions; keep impure/IO concerns at the adapter boundary
  (see `hexagonal-boundaries.instructions.md`).
- Keep scanners and hot-path code allocation-light and `O(n)` where the design
  calls for it (e.g. the tokenizer is a single left-to-right scan).

## Dependencies

- **Minimal and justified.** Each new dependency enters with a documented reason
  and, if it changes architecture, an ADR. The aim is `unicode-segmentation` and
  little else in `core`.
- License must be permissive and pass `cargo deny`.
