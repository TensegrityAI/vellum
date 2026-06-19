---
status: accepted
date: 2026-06-19
tags: [core, language, highlight, token, port, boundary]
related: [docs/adr/0007-lang-jinja-crate.md, docs/plans/2026-06-15-vellum-design.md]
---

# ADR-0009: `core` owns a generic highlight vocabulary, languages map onto it

## Context

ADR-0007 extracted the Jinja2 scanner into `vellum-lang-jinja` so "core no longer
knows about Jinja2, only the `Language` trait." The crate split moved the
*scanner* out, but the F/G/H audit (2026-06-19) found it left the *vocabulary*
in: `core::TokenKind` had the variants `Text / Variable / Statement / Comment`,
and `Variable` (`{{ }}`), `Statement` (`{% %}`), `Comment` (`{# #}`) are exactly
Jinja2's three block kinds — with doc comments in `core` that literally said "A
`{{ ... }}` expression block." So `core`'s token vocabulary was coupled to one
concrete language's grammar.

This undercuts the swappability guarantee that is the whole reason Vellum is
built (AGENTS.md §0–1) and is inconsistent with the care taken elsewhere: the
`Language` trait deliberately takes `&TextBuffer` not `&Rope` (ADR-0006/0007) to
avoid storage coupling, yet the token enum hardcoded language coupling. A second
language (`lang-sql`, `lang-md` on the roadmap) could not reuse the enum without
abusing `Variable`/`Statement` to mean something else or editing `core` to add
variants — exactly the "new languages are new crates, not edits to the engine"
property ADR-0007 claims to deliver.

## Decision

`core` owns a **generic, language-agnostic highlight vocabulary** that every
language plugin targets — the *palette* of the `Language` port — and no language
maps 1:1 onto it. `TokenKind` is renamed to **`HighlightKind`** and its variants
become standard syntax-highlighting scopes, modelled on the LSP
`SemanticTokenTypes` / TextMate scope conventions that every syntax highlighter
already speaks:

```rust
#[repr(u32)]
#[non_exhaustive]
pub enum HighlightKind {
    Text = 0,        // literal text outside any highlighted construct
    Variable = 1,    // variable / identifier / interpolated value
    Keyword = 2,     // keyword / control construct / tag
    Comment = 3,     // comment
    String = 4,
    Number = 5,
    Operator = 6,
    Function = 7,
    Type = 8,
    Punctuation = 9,
}
```

A language plugin is the **adapter** that maps its grammar onto this palette.
`lang-jinja` (Increment 1, block-granularity) maps:

- plain text → `Text`
- `{{ … }}` (interpolation) → `Variable`
- `{% … %}` (statement / tag) → `Keyword`
- `{# … #}` → `Comment`

This reframes the boundary correctly: the palette is `core`'s *port vocabulary*
(like a protocol's enum); languages are adapters that target it. Defining a
standard scope set in the engine is **not** "core knows Jinja" — it is "core
defines the highlight scopes, Jinja targets them," the same relationship LSP has
with its servers.

### Stable wire (`#[repr(u32)]`)

`HighlightKind` keeps `#[repr(u32)]`: the discriminant is the WASM token wire
(the flat `Uint32Array` of `[start, end, kind, …]`). The discriminants `0..=3`
are **chosen to be byte-identical to the old `TokenKind`** (`Text=0`,
`Variable=1`, the former `Statement` is now `Keyword=2`, `Comment=3`), so the
existing wire output for Jinja is unchanged and the TS view keeps working
numerically. New variants append at `4..` and the enum is `#[non_exhaustive]`
(consistent with `EditEvent`/`Severity`/`CompletionKind`, ADR-0005), so adding
scopes later — or a language emitting `String`/`Number`/etc. once it has a real
grammar — is non-breaking on both the Rust and wire sides.

### Why a curated set, not an open `HighlightId(u32)`

An open id space (languages mint arbitrary ids) was considered and rejected for
Increment 1: the view needs a *known* finite set of CSS `::highlight()` names to
style, and an open space pushes a name registry across the WASM boundary with no
payoff yet. A curated, growable enum gives the view a stable, themeable palette
today and can still evolve toward richer scopes later.

## Consequences

- `core` is genuinely language-agnostic in its token vocabulary, not just its
  scanner: `HighlightKind` names a generic palette, and `core`'s doc comments no
  longer mention Jinja. ADR-0007's "core no longer knows about Jinja2" is now
  true of the *vocabulary* too, not only the code.
- The wire contract is preserved for the same input (discriminants `0..=3`
  stable); the TS view's numeric `kind → highlight-name` map is unchanged, and
  the CSS/highlight *names* are renamed `statement → keyword` for clarity (a
  cosmetic follow-up, numbers unchanged).
- `lang-jinja` gains an explicit grammar→palette mapping (its adapter
  responsibility); when its real grammar lands (Increment 2) it can emit finer
  scopes (`String`, `Operator`, `Punctuation` for delimiters) without touching
  `core`.
- Some variants (`String`..`Punctuation`) are unused by the Increment-1 Jinja
  block tokenizer. They are vocabulary, not unwired features — the palette a
  highlighter targets, like an enum of status codes. `#[non_exhaustive]` means
  the set can still grow non-breakingly, so the curated list is a starting
  palette, not a frozen one.
- `core` stays `#![forbid(unsafe_code)]`; this is a rename + variant addition,
  no unsafe and no new dependency.
