# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

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
