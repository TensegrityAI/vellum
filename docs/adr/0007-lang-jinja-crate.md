---
status: accepted
date: 2026-06-18
tags: [core, language, jinja, crate-split, port]
related: [docs/adr/0005-repo-packaging.md, docs/plans/2026-06-15-vellum-increment-1.md]
---

# ADR-0007: Extract Jinja2 into a `vellum-lang-jinja` crate behind the `Language` port

## Context

Increment 0 shipped a trivial Jinja2 tokenizer as an inline module in
`vellum-core` (`core/src/lang_jinja.rs`), re-exported as a free `tokenize(&str)`
function. Task G1 then added the `Language` trait to `core`: the typed plugin
seam a syntax implementation plugs into (design §4 "Option B" extension spine),
with `tokenize(&self, doc, range) -> Vec<Token>` plus diagnostics/completion/
hover.

With the port in place, the inline Jinja module became the one thing keeping
`core` coupled to a concrete language. Phase G's acceptance is explicit: "core no
longer knows about Jinja2, only the `Language` trait." A language plugin is, by
design, swappable and additive (Jinja2 today; SQL/Markdown later), so it does not
belong inside the engine.

## Decision

Extract the Jinja2 tokenizer into a new workspace member, **`vellum-lang-jinja`**,
that depends on `vellum-core` and implements the `core` `Language` port:

- `pub struct Jinja;` — a stateless unit struct — `impl Language for Jinja`.
- The byte scanner stays a free `tokenize(&str) -> Vec<Token>` function (moved
  verbatim from `core`, with all its tests) so it remains unit-testable in
  isolation.
- `core` drops the `lang_jinja` module and its `pub use tokenize`. `core` now
  owns only the `Language` trait and the `Token`/`TokenKind` value types; it has
  zero Jinja knowledge.
- `wasm` gains a `vellum-lang-jinja` path dependency and drives `Jinja` through
  the `Language` port; the flat `Uint32Array` token wire is unchanged.

### Range-scoped tokenize (Increment 1 shape)

`Language::tokenize(doc, range)` re-tokenizes **only `range`**, per the plan:
slice `doc.text()` to the range, run the scanner on the slice, then offset the
returned tokens by `range.start` into whole-document byte coordinates. Two
documented Increment-1 limitations are accepted deliberately:

- Range bounds must be on UTF-8 char boundaries (a non-boundary `&str` slice
  panics — consistent with the `TextBuffer` offset contract).
- A naive slice can split a `{{ … }}` block at a range edge, changing the edge
  tokenization versus a whole-document scan. The view passes block-aligned
  ranges, and the whole-document case (`0..len`, what WASM sends) is byte-for-byte
  identical to the old in-core behavior. True incremental re-lexing with
  damaged-range / block-boundary expansion is an Increment 2 concern and is not
  implemented now.

## Consequences

- `core` is language-agnostic: it depends on nothing Jinja-specific and exposes
  only the `Language` port plus token types. New languages are new crates, not
  edits to the engine. (**Refined by ADR-0009:** this split moved the Jinja
  *scanner* out but left a Jinja-shaped token *vocabulary* — `TokenKind`'s
  `Variable/Statement/Comment` — in `core`; ADR-0009 replaces it with a generic
  `HighlightKind` palette so the vocabulary is language-agnostic too.)
- The workspace now has three lib crates (`core`, `lang-jinja`, `wasm`).
  `lang-jinja` has a single dependency, `vellum-core` (minimal-deps rule).
- `lang-jinja` is `#![forbid(unsafe_code)]`, matching `core`; only `wasm` relaxes
  that (for wasm-bindgen glue), unchanged by this split.
- The split is internal-path-dep only; `deny.toml`'s `allow-wildcard-paths`
  already covers it (as it did for `wasm` → `core`), so `cargo deny check` stays
  green with no policy change.
- The wire contract is preserved: the WASM `tokens()` output is unchanged for the
  same input (asserted by the existing wasm integration test), because a
  whole-document range reproduces the old scan exactly.
