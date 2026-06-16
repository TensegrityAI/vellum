# Vellum — Increment 1 (Real Generic Editor) Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (fresh
> subagent per task + code review between tasks), exactly as Increment 0 was executed.
> Each task is TDD: write the failing test → see it fail → minimal code → see it pass → commit.

**Goal:** Turn the Increment 0 walking skeleton into a real, generic, editable editor: a
rope-backed **event-sourced** buffer with **undo/redo**, grapheme-correct cursor/selection,
**diff-based input** (no more full-buffer resync) via an `InputSource` port with an
**EditContext** adapter (+ textarea fallback), a **MeasurePort + arithmetic layout** with
viewport virtualization, and a real **`Language` trait** with `lang-jinja` extracted into its
own crate. Resolves the three tracked blockers from Increment 0.

**Repo root:** `/home/nexus/workspace/vellum` (origin = GitLab `nexum/vellum`, mirrors to
GitHub `TensegrityAI/vellum` on the default branch). **Tag at the end:** `v0.0.2-inc1`.

**Read first:**
- Design: `docs/plans/2026-06-15-vellum-design.md` (§3 events, §4 view/input, §5 measurement).
- Increment 0 plan + its "Tracked follow-ups" section: `docs/plans/2026-06-15-vellum-increment-0.md`.
- ADRs 0001–0005 in `docs/adr/`. This increment writes ADR-0006 (rope choice) and likely
  ADR-0007 (offset model).
- `AGENTS.md` for the non-negotiables.

**House signature (unchanged):** `core` stays `#![forbid(unsafe_code)]`, minimal justified
deps, typed errors (`thiserror`), behavior-named TDD tests, ADR-before-architecture, every
phase ends green on `cargo fmt/clippy -D warnings/test/deny`, `wasm-pack test --node`, and
`bun run check/test`. CI runs on GitLab (the `test` job already installs the full toolchain).

---

## Carry-over blockers from Increment 0 (MUST be resolved here)

1. **UTF-16 ↔ UTF-8 offset conversion.** DOM/textarea/EditContext speak UTF-16 code units;
   the core speaks UTF-8 bytes. Diff-based mutation **must** convert, or the core traps. See
   the warning in `ts/view/src/view.ts` (`syncCoreToValue`). → Phase F (core offset utils) + Phase I.
2. **Per-instance highlight names.** `CSS.highlights` is global; Inc 0 used fixed names →
   single-surface only. → Phase I.
3. **WASM-boundary panic → Result.** `Editor::insert/delete` panic on bad offsets and poison
   the instance. Inc 1 makes them validate and return `Result<_, JsError>`. → Phase H.

---

## Phase F — Core: rope, events, undo/redo, offsets (Rust, TDD)

> Decision to record as **ADR-0006**: back the buffer with the `ropey` crate (vetted, minimal,
> used by Helix) rather than hand-rolling a rope now. Rationale: correctness + speed today;
> the `TextBuffer` API already abstracts storage so a hand-rolled rope can replace it later
> without touching callers. `ropey` is MIT — add to `deny.toml` allow-list. Grapheme/word
> boundaries come from `unicode-segmentation` (ADR-0001), layered on top of `ropey`.

### Task F1: ADR-0006 (rope choice) + add `ropey` dep
- Write `docs/adr/0006-rope-buffer.md` (status accepted, rationale above, replaceability note).
- Add `ropey` to `crates/core/Cargo.toml`; add `MIT` already allowed in `deny.toml` (verify).
- Commit: `docs: add ADR-0006 (ropey-backed buffer)`.

### Task F2: Re-implement `TextBuffer` on `ropey` (keep the public API green)
- The existing `TextBuffer` tests (construct/read/insert/delete) MUST still pass — this is a
  refactor behind the same API. Add char/grapheme length accessors.
- TDD: add tests for large-text insert/delete and `char_len` vs `byte_len` before swapping the
  backing store; keep all Increment 0 tests green.
- Commit: `refactor(core): back TextBuffer with ropey (API unchanged)`.

### Task F3: Offset model + conversions (the blocker-1 foundation)
- New `crates/core/src/offset.rs`. Model the three coordinate spaces explicitly with newtypes:
  `ByteOffset(usize)`, `CharOffset(usize)`, `Utf16Offset(usize)` (house newtype style).
- Functions on `TextBuffer`: `byte_to_utf16`, `utf16_to_byte`, `byte_to_char`, `char_to_byte`,
  and grapheme-aware `prev_grapheme_boundary`/`next_grapheme_boundary` (via `unicode-segmentation`).
- TDD with multibyte + astral-plane cases: `"café"`, `"a👨‍👩‍👧b"` (ZWJ emoji), CJK. Assert
  round-trips and that astral chars are 2 UTF-16 units but 4 UTF-8 bytes.
- Commit: `feat(core): add byte/char/utf16/grapheme offset conversions`.

### Task F4: Edit events + apply
- `crates/core/src/event.rs`: `enum EditEvent { Inserted { at: ByteOffset, text: String }, Deleted { at: ByteOffset, removed: String } }` (store removed text so the inverse is exact).
- `TextBuffer::apply(&EditEvent)` and `EditEvent::inverse(&self) -> EditEvent`.
- TDD: applying then applying the inverse returns the original text; inverse of inverse is identity.
- Commit: `feat(core): model edits as reversible events`.

### Task F5: Document aggregate with undo/redo
- `crates/core/src/document.rs`: `Document { buffer, undo: Vec<EditEvent>, redo: Vec<EditEvent> }`.
  - `edit(&mut self, EditEvent)` applies + pushes inverse to `undo`, clears `redo`.
  - `undo()`/`redo()` move events across stacks, applying inverses.
  - (Coalescing of consecutive single-char inserts is OPTIONAL; if added, TDD it; else defer.)
- TDD: type → undo → redo restores; undo past empty is a no-op; new edit clears redo.
- Commit: `feat(core): add Document aggregate with undo/redo`.

### Task F6: Cursor & selection (grapheme-aware)
- `crates/core/src/cursor.rs`: `Selection { anchor: ByteOffset, head: ByteOffset }` with
  grapheme-step `move_left/right`, `extend_*`, `collapse`, word-step (via unicode-segmentation).
- TDD: moving right across an emoji moves one grapheme (not one byte/char); selection over CJK.
- Commit: `feat(core): add grapheme-aware cursor and selection`.

**Phase F acceptance:** `cargo test` green; core still zero `unsafe`; offset round-trips proven
on astral/CJK; undo/redo correct. ADR-0006 committed.

---

## Phase G — Language trait + `lang-jinja` crate (Rust, TDD)

### Task G1: Define the `Language` trait in core
- `crates/core/src/language.rs` (the design §4 trait, adapted to current types):
  `tokenize(&self, doc, range) -> Vec<Token>`, `diagnostics`, `complete`, `hover` (the latter
  three may return empty/None stubs in Inc 1 — wire the shape, fill in Inc 2). Add `Diagnostic`,
  `Completion`, `Hover` value types.
- TDD: a trivial test `Language` impl returns expected tokens for a range.
- Commit: `feat(core): define Language trait + diagnostic/completion/hover types`.

### Task G2: Extract `lang-jinja` into its own crate
- New `crates/lang-jinja` (workspace member, `#![forbid(unsafe_code)]`, dep `vellum-core`).
- Move the trivial tokenizer from `core/src/lang_jinja.rs` into `lang-jinja` implementing
  `Language`; add **range-scoped** tokenize (re-tokenize only a damaged range, not whole doc).
- Keep ALL existing tokenizer tests (move them); add range-tokenize tests.
- Update `core` to drop the inline `lang_jinja` module; update `wasm` to depend on `lang-jinja`.
- Commit: `refactor(lang-jinja): extract Jinja2 language into its own crate`.

**Phase G acceptance:** workspace builds with 3 crates; tokenizer tests green in new crate;
`core` no longer knows about Jinja2 (only the `Language` trait).

---

## Phase H — WASM: fallible API, events, undo/redo, cursor (TDD)

### Task H1: Make mutation fallible (blocker-3)
- `Editor::insert/delete` validate offsets (char-boundary + bounds) and return
  `Result<(), JsError>` instead of panicking. Add `wasm-bindgen-test`s for the error path
  (bad offset → `Err`, instance still usable afterward).
- Commit: `feat(wasm): validate offsets and surface errors instead of trapping`.

### Task H2: Expose Document (events/undo/redo) + cursor + offset conversions
- Swap the wasm `Editor` to wrap `Document`. Expose `undo()`, `redo()`, cursor getters/movers,
  and offset-conversion helpers (`utf16_to_byte`, `byte_to_utf16`) the view needs for diffing.
- Keep `tokens()` (now via `lang-jinja`’s `Language`), returning the same flat `Uint32Array` wire.
- TDD: insert → tokens; undo/redo roundtrip across the boundary; utf16↔byte on multibyte.
- Commit: `feat(wasm): expose document, undo/redo, cursor, offset conversions`.

**Phase H acceptance:** `wasm-pack test --node` green; no panics across the boundary.

---

## Phase I — View: InputSource port, diff input, MeasurePort, virtualization (TS, TDD where pure)

### Task I1: `InputSource` port + adapters
- `ts/view/src/input/` : an `InputSource` interface; `HiddenTextareaInput` (refactor the Inc 0
  textarea behind it) and `EditContextInput` (Chromium; feature-detect `window.EditContext`,
  handle `textupdate`/composition). A `FakeInput` for tests. `mountVellum` picks the adapter via
  feature detection (EditContext if present, else textarea).
- Unit-test the **pure** selection logic; adapter DOM wiring is verified in the demo.
- Commit: `feat(view): InputSource port with EditContext + textarea adapters`.

### Task I2: Diff-based mutation with offset conversion (blocker-1)
- Replace `syncCoreToValue` (full clear+reinsert) with a minimal diff: compute the changed
  UTF-16 range from the input event, convert to byte offsets via the wasm helpers (Task H2),
  and call `editor.insert/delete` on just that range.
- TDD a pure `computeDiff(oldStr, newStr) -> {utf16Start, utf16RemovedLen, inserted}` helper with
  multibyte/astral cases. Wire it through the adapter.
- Commit: `feat(view): diff-based input via UTF-16→byte conversion`.

### Task I3: Per-instance highlight namespacing (blocker-2)
- Give each `mountVellum` instance a unique id; highlight names become `vellum-${id}-variable`
  etc.; `clearHighlights` scoped to the instance. CSS uses attribute-scoped `::highlight()` or
  generated style. TDD the name-generation helper; verify two surfaces don't clobber (demo).
- Commit: `fix(view): namespace CSS highlights per editor instance`.

### Task I4: MeasurePort + arithmetic layout + virtualization
- `ts/view/src/measure.ts`: a `MeasurePort` (Canvas `measureText`, cached per `(grapheme,font)`);
  monospace fast path (single advance). Pure layout math (line breaks/positions) — TDD it with a
  fake measurer. Virtualize: render only visible lines (content-visibility or manual windowing).
- Commit: `feat(view): MeasurePort + arithmetic layout + viewport virtualization`.

### Task I5: Caret + selection rendering
- Render the caret and selection (selection via a `vellum-selection` highlight or overlay).
  Wire cursor movement from keyboard through the core cursor (Task F6).
- Commit: `feat(view): render caret and selection`.

**Phase I acceptance (visual, in browser via chrome-devtools MCP):** type multibyte text
(emoji/CJK) — no traps, correct highlighting; undo/redo works; caret/selection move by grapheme;
two editors on one page don't clobber highlights.

---

## Phase J — Tests, demo, docs, release

### Task J1: View tests under jsdom/happy-dom (add the dev dep) for the diff + namespacing logic.
### Task J2: Update the Vite demo — add an undo/redo button, a second editor instance, multibyte seed text. Re-capture `docs/assets/` screenshot.
### Task J3: ADRs (0006 rope already; add 0007 offset model if not folded in), CHANGELOG `[0.0.2]`, README updates.
### Task J4: Verify full DoD locally + push; confirm GitLab pipeline green and GitHub mirror updated; tag `v0.0.2-inc1`.

---

## Definition of Done (Increment 1)

- [ ] Rope-backed, event-sourced buffer; undo/redo correct; core still zero `unsafe`, zero panics across WASM.
- [ ] Offset conversions proven on astral/CJK; diff-based input (no full resync).
- [ ] `Language` trait in core; `lang-jinja` is its own crate; core ignorant of Jinja2.
- [ ] EditContext adapter (Chromium) + textarea fallback behind `InputSource`; IME works.
- [ ] MeasurePort + arithmetic layout + virtualization; caret/selection render and move by grapheme.
- [ ] Per-instance highlights; two editors coexist.
- [ ] All gates green (rust/wasm/ts); GitLab pipeline + GitHub mirror green; tagged `v0.0.2-inc1`.

---

## Roadmap beyond Increment 1 (not yet bite-sized — plan in detail when reached)

### Increment 2 — `lang-jinja` becomes excellent
Real Jinja2 grammar (parser, not regex): proper expressions/filters/blocks, **diagnostics**
(unclosed blocks, unknown filters), **autocomplete** (variables injected by host + filters),
**hover** docs. Host-injected variable provider (domain-agnostic). Autocomplete/hover popups via
**Popover API + CSS Anchor Positioning** (design §4). Incremental re-parse on edit.

### Increment 3 — `@vellum/react` + Nexum integration
`ts/react` wrapper (controlled/uncontrolled, ref API). Host **ports**: load/save template,
variable provider, version list. Live **Jinja2 preview** pane (render with host-provided values).
Replace the legacy `PromptTemplateEditor` in `nexum-rag-client`. Wire to the v2 API.

### Deferred (post-Inc 3)
AI Assist (prompt quality analysis), version history + diff viewer, snippets library, additional
languages (`lang-sql`, `lang-md`), hand-rolled rope (if `ropey` ever constrains us), collaborative
editing (CRDT/OT) — the event-sourced model already makes this a natural extension.

### Standing optimizations
A prebuilt CI Docker image (rust 1.89 + node + bun + wasm-pack + cargo-deny) to kill the
cold-install time of the GitLab `test` job. Flip GitHub public per ADR-0005 once Inc 1 lands.
