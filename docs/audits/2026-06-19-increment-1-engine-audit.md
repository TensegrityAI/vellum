# Vellum — Increment 1 Engine (Phases F/G/H) Deep Audit & Remediation

- **Date:** 2026-06-19
- **Scope:** the whole Inc-1 Rust + WASM engine (commits `5320e24..d0c7e77`, the
  state at handoff `d78487c`): `vellum-core`, `vellum-lang-jinja`, `vellum-wasm`.
- **Method:** 6 parallel adversarial lenses (correctness/Unicode-safety,
  architecture/DDD, test-rigor, API-coherence, performance, OSS-readiness), then
  synthesis, verification, and TDD remediation (red→green→commit) with a
  `code-reviewer` checkpoint on the behavior-changing commits.
- **Baseline:** all gates green at start (core 115, lang-jinja 12, wasm 11).
- **After remediation:** all gates green (core 127, lang-jinja 13, wasm 12; fmt,
  clippy `-D warnings`, `cargo deny`, `wasm-pack test`, `bun check`/`test`).

## Headline verdict

The engine is **solid**. The correctness/Unicode lens attacked it with a
~6,000,000-operation randomized fuzz against the WASM validation gates and found
**zero** reachable panics or data-corruption paths. Hexagonal purity is real (no
`ropey`/DOM/Jinja leak into `core`, `unsafe` correctly isolated to `wasm`,
dependency direction strict). Two handoff worries were **disproven**: CI *does*
run `wasm-pack test` (the JS-boundary suite is gated in both GitLab and GitHub
pipelines), and the `Unicode-3.0` license allowance is *required* (via
`unicode-ident`), not stale.

The real debt was in **honesty, encapsulation, test coverage, and a handful of
cheap perf wins** — plus one genuine architectural finding (token vocabulary).

## Findings & disposition

| ID | Lens | Severity | Finding | Disposition |
|----|------|----------|---------|-------------|
| C1/A1 | Arch | Critical | `TokenKind` carried Jinja-specific grammar (`Variable/Statement/Comment`) in `core`, undercutting the language-agnostic guarantee | **Fixed** — generic `HighlightKind` palette, languages map onto it (ADR-0009) |
| I1/A2 | Arch | Important | "event-sourced buffer" overclaimed in prose vs the honest ADR-0002 (it's inverse-event stacks) | **Fixed** — prose in `document.rs`/`event.rs`/AGENTS softened |
| A3 | Arch | Important | ADR-0001 & ADR-0005 still `proposed` though built upon | **Fixed** — flipped to `accepted` |
| A4 | Arch/API | Important | `Selection.anchor/head` `pub` → aggregate clamp invariant not encapsulated | **Fixed** — `pub(crate)` + `anchor()/head()` getters |
| API1 | API | Important | `ByteOffset(pub usize)` etc. — public tuple field bypasses newtype safety | **Fixed** — `pub(crate)` (external API can't wrap raw `usize`) |
| API2 | API | Important | `ByteRange::len()==0` vs `is_empty()` disagree on inverted ranges | **Fixed** — documented + pinned by test |
| API6 | API | Important | `TokenKind` not `#[non_exhaustive]` while siblings were | **Fixed** — folded into ADR-0009 (`HighlightKind` is `#[non_exhaustive]`) |
| P1 | Perf | Important | `Document::delete` materialized the whole doc (`text()[range]`) to read one run | **Fixed** — `TextBuffer::slice` via rope `byte_slice` |
| P3 | Perf | Important | Unbounded undo/redo `Vec` (memory vector in agent/scripted loops) | **Fixed** — `VecDeque` capped at `MAX_HISTORY=1000` |
| P4 | Perf | Minor | `tokens()` did an identity re-offset alloc on the whole-doc path | **Fixed** — skip when `range.start==0`; slice not `text()` |
| T1–T7 | Tests | Important | `backspace`/`delete_forward`/word-movers untested at `Document` level; `try_*` round-trip not covered on combining/CJK-mixed; ZWJ only in 2 of 8 movers; block-split & OOB-tolerance unpinned; type-over-two-undos unpinned | **Fixed** — 11 new tests (core 127, jinja 13, wasm 12) |
| O1 | OSS | Important | wrong `repository` URL; CHANGELOG "zero deps" now false; README arch missing `lang-jinja` | **Fixed** — URL→`TensegrityAI/vellum`, `[Unreleased]` honesty, README arch; `deny.toml` documents the required `Unicode-3.0` |
| API1-doc | API | Minor | panic docs inconsistent (`# Panics` vs prose) | **Deferred** — cosmetic; tracked for a docs sweep |
| API5 | API | Important | view will need `tokens_in_range`/`slice`/length getters on `Editor` | **Deferred to Phase I** — added where consumed (I2/I4), not speculatively |

### Disproven (not findings)

- **CI wasm gate** — `wasm-pack test --node crates/wasm` runs in both `.gitlab-ci.yml`
  and `.github/workflows/ci.yml`. The JS-boundary trap-safety suite is gated.
- **`Unicode-3.0` allowance** — required by `unicode-ident` (transitive via
  `proc-macro2`). Documented in `deny.toml`; do not remove.

### Consciously deferred (correct for Inc-1, confirmed by the perf lens)

- Chunk-streaming grapheme/word nav (still `text()`-per-step) — fine at prompt
  sizes; revisit with Inc-2 incremental re-lexing.
- Incremental/damaged-range re-tokenize (Inc-2), insert-coalescing, history
  snapshot/compaction (ADR-0002).
- README release presentation (status line, screenshot, version bump, tag) —
  a Phase-J task.
- Public identity reconciliation (Akaisys author vs `TensegrityAI` org) — the
  OSS-flip decision (ADR-0005 / Phase J).

## Remediation commits (on `master`, after `d78487c`)

1. `docs: honest event-sourcing scope + accept ADR-0001/0005 + neutral CompletionKind`
2. `refactor(core)!: generic HighlightKind vocabulary, languages map onto it [ADR-0009]`
3. `refactor(view): rename highlight 'statement' -> 'keyword' (ADR-0009 vocab)`
4. `refactor(core): encapsulate offset + selection fields as pub(crate) [audit API1/A4]`
5. `perf(core): read deleted run via rope byte_slice, not whole-doc text() [audit P1]`
6. `perf: cap undo history + slice-based tokenize without identity re-offset [audit P3/P4]`
7. `test: close coverage gaps from the test-rigor audit lens + API-2 doc`
8. `docs: OSS hygiene — correct repo URL, license provenance, changelog honesty [audit O1]`

New ADR: **ADR-0009** (generic highlight vocabulary).

## Next

Audit + remediation complete; engine is clean. Proceed to **Phase I — the
TypeScript view layer** (Task I1: `InputSource` port + adapters), per
`docs/plans/2026-06-15-vellum-increment-1.md`. The deferred view-API additions
(`tokens_in_range`, `Editor::slice`, length getters) are added there, at the
point they are consumed.
