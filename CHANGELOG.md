# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased] - Increment 1 (Real Generic Editor) — in progress

The Increment-0 scaffold becomes a real editor. Full release notes, version bump,
and tag land in Phase J; this section keeps the changelog honest meanwhile.

### Added

- **Rope-backed buffer** (`ropey`, ADR-0006) with byte/char/UTF-16 offset
  conversions (panicking + non-panicking `try_*`), grapheme/word boundaries
  (`unicode-segmentation`), and `TextBuffer::slice` over rope chunks.
- **Reified-event undo/redo** (ADR-0002): `EditEvent`, the `Document` aggregate
  (cursor in the aggregate, ADR-0008), and a bounded undo history.
- **`vellum-lang-jinja`** crate (ADR-0007): the Jinja2 tokenizer extracted behind
  the `core` `Language` port; `core` no longer knows any concrete language.
- **Generic `HighlightKind` vocabulary** (ADR-0009): `core` owns a language-
  agnostic highlight palette; languages map their grammar onto it.
- **Fallible WASM boundary**: `Editor` validates untrusted offsets and returns
  `Result` (no traps); exposes undo/redo, cursor movement, and UTF-16↔byte
  conversions.

### Changed

- **`vellum-core` is no longer zero-dependency** (the Increment-0 claim below):
  it now depends on `ropey`, `unicode-segmentation`, and `thiserror`, each a
  justified, documented entry (minimal-deps rule, AGENTS §2).
- `Token`'s kind is now `HighlightKind` (was `TokenKind`); the WASM u32 token
  wire is unchanged (discriminants `0..=3` preserved).

## [0.0.1] - Increment 0 (Walking Skeleton)

The end-to-end pipeline proven: typing Jinja2 in a standalone demo shows live syntax
highlighting, with every text operation flowing through a Rust core compiled to WASM and
painted via the CSS Custom Highlight API (no `<span>` per token).

### Added

- **Foundations:** Cargo workspace, tooling (rustfmt, clippy, cargo-deny), OSS governance
  documents, ADRs 0001–0005, the Agent Operating Layer, and GitHub Actions CI (rust + wasm + ts).
- **`vellum-core`** (pure Rust, `#![forbid(unsafe_code)]`, zero deps): `TextBuffer`
  (char-boundary-safe insert/delete), `Token`/`TokenKind` model, and a trivial Jinja2
  `tokenize` emitting gap-free, UTF-8-boundary-aligned token ranges.
- **`vellum-wasm`** (wasm-bindgen): an `Editor` exposing `text`/`insert`/`delete`/`tokens`,
  with tokens crossing to JS as a flat `Uint32Array` wire (no serde).
- **`@vellum/view`** (TypeScript): pure `groupTokensByKind`, and a thin `mountVellum` view
  that renders real-text DOM, captures input via a hidden textarea, and paints syntax with
  the CSS Custom Highlight API.
- **Demo:** a standalone Vite playground.
