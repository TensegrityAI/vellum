# Vellum — Increment 1 Phase I (View Layer) Deep Audit & Remediation

- **Date:** 2026-06-22
- **Scope:** the whole Inc-1 **Phase I** surface — the TS view layer plus the
  Rust/wasm support it required. Commits `fd16cd8..0f14dae` on `master`
  (engine F/G/H, audited separately on 2026-06-19, sits underneath):
  - **I2** `fd16cd8` — diff-based core mutation (`ts/view/src/diff.ts`, `applyDiff`).
  - **I3** `43c4216` — per-instance highlight namespacing + byte→UTF-16 multibyte paint.
  - **I4** `727b36d` + `eafe938` — arithmetic layout in `core` (ADR-0004), `MeasurePort`,
    per-line windowed virtualization, `tokens_in_line`.
  - **I5** `9d3db0b` — caret + selection render, keyboard movement, composition.
  - **Audit remediation** `0f14dae` — undo/redo wiring, inert nav keys, pure `keyboard.ts`.
- **Method:** 3 parallel adversarial lenses — architecture/ADR-conformance,
  correctness/robustness/trap-freedom, DoD/test-rigor/honesty — then synthesis and
  TDD remediation of the cheap, high-value findings (red→green→commit), with per-slice
  `code-reviewer` checkpoints already applied during I2–I5.
- **Gates at audit close (all green):** `cargo test --workspace` core **146** /
  lang-jinja **13**; `wasm-pack test --node` **27**; `bun --cwd ts/view test` **52**;
  `cargo fmt --check`, `cargo clippy -D warnings`, `cargo deny`, `tsc --noEmit` clean.

## Headline verdict

Phase I is **solid and ships the Increment-1 view scope**. The correctness lens
attacked the new wasm surface (`tokens_in_line`/`selection_in_line`/`caret_xy`/
`line_text`/`visible_lines`/line primitives) for panics, underflow, and desync across
the JS boundary and found **zero** reachable traps — the Phase-H "no panics across the
boundary" bar holds for every Phase-I method, proven by guards (`line >= line_count()`),
monotonic `byte_to_utf16` (no line-local underflow), `clamp_selection` char-boundary
snapping, and saturating `f32→usize` casts. The hexagonal spine is intact: `core` stays
pure (deps `ropey`/`thiserror`/`unicode-segmentation`, zero browser/wasm), **all** layout
arithmetic genuinely lives in `core` per ADR-0004 (the view does no text-model math), the
line model is single-sourced from `core` (`line_text`/`line_byte_range`) so the view never
re-derives it, and ADR-0009's generic `HighlightKind` vocabulary is Jinja-free end to end.

The real debt was in **assembled/DOM-bound behavior asserted rather than verified**, one
**undo/redo wiring gap** (the most serious), one **ADR-0004 hot-path reflow**, and a few
honesty nits. The gating gap and the cheap honesty/correctness items were remediated this
session; the rest are scoped to Phase J or Increment 2 below.

## Findings & disposition

| ID | Lens | Severity | Finding | Disposition |
|----|------|----------|---------|-------------|
| D1 | DoD/honesty | **Important** | Undo/redo exposed in wasm but **unreachable from the assembled view**; the `lastValue` comment implied a `setValue`-on-undo path that did not exist (borderline overclaim). Inc-1 "undo/redo works" unmet on the EditContext adapter. | **Fixed** (`0f14dae`) — Ctrl/Cmd+Z → `undo`, Ctrl+Y / Ctrl/Cmd+Shift+Z → `redo`, programmatic push + cursor re-sync; comment corrected; visually verified. |
| A-M1 | Arch | Important→**Fixed** | Unhandled vertical/Home/End keys were left to the browser default, drifting the device caret out of sync with the core-owned caret on the next edit. | **Fixed** (`0f14dae`) — `isInertNavKey` swallows Up/Down/Home/End/PageUp/PageDown (preventDefault, inert); vertical movers deferred. |
| T-gap | Tests | Important→**Fixed** | The keyboard navigation/history policy (pure, but DOM-event-shaped) had **zero** automated coverage. | **Fixed** (`0f14dae`) — extracted to pure `keyboard.ts`; 13 DOM-free unit tests (movers × modifiers, undo/redo intents, inert-key set). |
| H1 | Honesty | Minor→**Fixed** | `TODO(I5)` for the resize/ResizeObserver re-window pointed at an already-shipped phase. | **Fixed** — relabelled in `view.ts` to a forward TODO (re-window on resize). |
| A-I1 | Arch | **Important** | ADR-0004 §Decision forbids DOM size reads in hot paths; `renderViewport` calls `surface.getBoundingClientRect()` + `caret.getBoundingClientRect()` on **every render incl. scroll** (per-frame reflow) to feed IME caret bounds. Self-documented `TODO(perf)`. | **Deferred (Phase J / Inc-2)** — replace with an arithmetic screen-rect (cached surface origin + `caret_xy` − scroll), and/or only push bounds on caret-affecting renders. Tracked; not a correctness bug, monospace Inc-1 impact is small. |
| A-I3 | Arch | Important | Keyboard navigation flows through a raw `keydown` listener on `host`, outside the `InputSource` port — so movement is not exercisable via `FakeInput`, a coherence gap vs ADR-0003's single-input-abstraction intent. | **Partially addressed** — the *policy* is now pure + unit-tested (`keyboard.ts`); routing key intents through the port (or an ADR-0003 amendment scoping the port to text+selection) is **deferred to Phase J**. |
| D2 | DoD | Important | "Two editors coexist" (blocker #2) is **argued** (disjoint-name unit test) but never **executed** — the demo mounts one editor. | **Deferred to Phase J** — J2 adds a second demo instance / J1 a jsdom two-editor test. Do not tick the DoD box until then. |
| A-M2 | Arch | Minor | Caret x is a **grapheme**-column × advance (ADR-0001) while highlight/selection ranges use **UTF-16** columns; they diverge on mixed-width/astral lines (the documented Inc-1 monospace simplification). | **Deferred (Inc-2)** — width-aware columns; note the caret↔selection drift in ADR-0004 when revisited. |
| C-M1 | Correctness | Minor | `line_byte_range` trims only `\n`/`\r`, but `line_count` counts ropey's full break set (U+2028/2029/NEL/VT/FF), so those separators render literally inside a line (offsets stay aligned; no trap). | **Deferred (Inc-2)** — trim the full break set via one shared constant. Documented in `buffer.rs`. |
| C-M2 | Correctness | Minor | The `Math.min(offset, maxLen)` clamp in highlight/selection paint silently truncates rather than surfacing a desync (opposite of `applyDiff`'s "let it throw"). | **Deferred (Inc-2)** — add a dev-build assert before the clamp; keep the clamp as the prod fallback. |
| H2 | Honesty | Minor | The demo's Inc-0 `input`-event console logger is dead on the EditContext adapter (no DOM `input` event fires). | **Deferred to Phase J** — J2 rewrites the demo; flagged so it isn't mistaken for Phase-I verification. |

### Verified safe (with evidence, not assertion)
- **Trap-freedom across the boundary** for every Phase-I method: empty doc, only-`\n`,
  past-the-end line, caret/selection at doc end, astral + combining at line boundaries,
  CRLF, multi-line/boundary-ending Jinja blocks, NaN/inf/negative/`usize::MAX` layout
  inputs — no panic, `start <= end` always, no line-local underflow.
- **`computeDiff`** surrogate-safe over 17 adversarial cases incl. lone-surrogate inputs;
  `utf16RemovedLen` provably ≥ 0; conversions run against the unmutated core before mutation.
- **ADR-0009** generic highlight vocabulary is clean from `core` through the wire to the CSS
  names; **ADR-0001** grapheme columns correct in `locate`; **MeasurePort** is measure-once.

### Docs governance
- **No new ADR required for "layout in Rust core"** — that decision is ADR-0004 (accepted
  2026-06-15); I4 *implemented* it, it did not make a new decision. The one ADR follow-up
  worth considering is an **ADR-0003 note** on the keyboard-navigation/port relationship
  (A-I3), to be decided in Phase J.
- CHANGELOG `[Unreleased]`, "nothing pushed", and `v0.0.2-inc1` as a Phase-J step are all
  honest. The Inc-1 DoD checklist is the one place docs lead reality: **do not tick the
  undo/redo, two-editor, or IME boxes until Phase J verifies them end to end** (undo/redo is
  now wired but its *verification* is demo-only).

## Phase J entry checklist (carried forward)

1. **A-I1** — kill the per-frame `getBoundingClientRect` reflow (arithmetic caret screen-rect).
2. **D2** — second demo editor + jsdom two-editor test; then tick the "two editors coexist" box.
3. **A-I3** — decide: route key intents through `InputSource`, or amend ADR-0003.
4. IME candidate-window placement: verify with a real IME (currently wired, unverified).
5. Inc-2 carry: full Unicode line-break trimming (C-M1), dev-assert on paint clamp (C-M2),
   width-aware caret/selection columns (A-M2), incremental re-lex (per-line tokenize perf).
