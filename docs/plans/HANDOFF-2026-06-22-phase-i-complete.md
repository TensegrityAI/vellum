# Vellum — Session Handoff (2026-06-22, Mon · evening)

Pick up in a **fresh session**. **Phase I (the view layer) is COMPLETE, audited,
remediated, and green.** Tasks I2 → I5 all landed with TDD + per-task `code-reviewer`,
and a 3-lens Phase-I deep audit ran with its cheap/gating findings remediated. The next
session is **Phase J** (tests, demo, docs, release).

---

## TL;DR for the next session

1. **Read, in order:** this file → `AGENTS.md` (non-negotiables) →
   `docs/audits/2026-06-22-phase-i-view-audit.md` (esp. its **"Phase J entry checklist"**)
   → `docs/plans/2026-06-15-vellum-increment-1.md` (Phase J tasks J1–J4 + the DoD).
2. **Do Phase J** (below), same discipline: TDD (red→green→commit), per-task
   `superpowers:code-reviewer`, small Conventional Commits on `master`.
3. Work happens in **`/home/nexus/workspace/vellum`** (NOT inside rag-apptlas).

---

## Current state (verified green at handoff)

- Branch `master`, HEAD `ed8624a`, working tree clean. **Nothing pushed yet** — all
  local (push to GitLab `master` triggers the GitHub mirror; a Phase-J step, J4).
- **3 Rust crates** (`vellum-core`, `vellum-lang-jinja`, `vellum-wasm`) + **`ts/view`** + `ts/demo`.
- **Gates (all green):**
  - `cargo test --workspace` → **core 146**, **lang-jinja 13** (+doctests).
  - `wasm-pack test --node crates/wasm` → **27** (only under wasm-pack, NOT `cargo test`).
  - `cargo fmt --all -- --check`; `cargo clippy --all-targets -- -D warnings`; `cargo deny check` — clean.
  - `bun run --cwd ts/view check` (tsc) clean; `bun run --cwd ts/view test` → **52**
    (15 diff + 5 measure + 6 highlight-names + 4 highlights + 9 input + 13 keyboard).
- **PATH gotchas:** `export PATH="$HOME/.cargo/bin:$HOME/.bun/bin:$PATH"` — `wasm-pack`,
  `cargo`, `bun` are not on the default PATH. Build the wasm pkg the view imports with
  `bash scripts/build-wasm.sh` (output `ts/view/wasm/`, gitignored) — rebuild it before any
  demo check and after any Rust change, or the view's `.d.ts` is stale.
- **chrome-devtools MCP connected** — used throughout Phase I for in-browser visual acceptance.

---

## What Phase I delivered (commits `fd16cd8..ed8624a`)

- **I2** `fd16cd8` — diff-based core mutation: pure `computeDiff` (`ts/view/src/diff.ts`,
  surrogate-safe), `applyDiff` via wasm `utf16_to_byte`; `syncCoreToValue` removed (blocker #1).
- **I3** `43c4216` — per-instance highlight namespacing (`highlight-names.ts`, blocker #2) +
  byte→UTF-16 multibyte paint correctness.
- **I4** `727b36d` + `eafe938` — **ADR-0004 honored**: layout = pure arithmetic in `core`
  (`crates/core/src/layout.rs` + `TextBuffer` line primitives), wasm `line_count`/`visible_lines`/
  `tokens_in_line`/`line_text`, `MeasurePort` (`measure.ts`), per-line **windowed virtualization**.
  Decision made with Kael: layout lives in Rust core (not TS), honoring ADR-0004 over the
  plan's looser wording.
- **I5** `9d3db0b` — caret + selection render (wasm `caret_xy`/`selection_in_line`), keyboard
  grapheme/word movement, cursor↔device sync, composition (InputSource port widened with
  `updateCaretBounds`; EditContext pushes IME bounds).
- **Audit remediation** `0f14dae` — wired **undo/redo** (Ctrl/Cmd+Z, Ctrl+Y/Ctrl+Shift+Z) from
  the view (was unreachable); swallow inert nav keys (Up/Down/Home/End/PageUp/Down) to prevent
  device-caret drift; extracted pure `keyboard.ts` (13 unit tests).
- **Audit report** `ed8624a` — `docs/audits/2026-06-22-phase-i-view-audit.md`.

The editor is now usable in-browser: type multibyte text (no traps, correct highlighting,
virtualized scrolling), undo/redo, caret + selection move by grapheme/word — verified on **both**
the EditContext and hidden-textarea adapters.

---

## ➡️ NEXT: Phase J — tests, demo, docs, release

Per `docs/plans/2026-06-15-vellum-increment-1.md` §Phase J, and the audit's Phase-J checklist:

- **J0 (from the audit, do first):**
  1. **A-I1** — kill the per-frame `getBoundingClientRect` reflow in `renderViewport`
     (`view.ts`, the `updateCaretBounds` call): compute the caret screen-rect arithmetically
     (cache surface origin + `caret_xy` − scroll), or only push bounds on caret-affecting
     renders. This is the one self-admitted **ADR-0004 hot-path violation** still shipping.
  2. **A-I3** — decide the keyboard↔`InputSource` story: route key intents through the port
     (so `FakeInput` can drive movement), or write an ADR-0003 amendment scoping the port to
     text+selection. Today navigation bypasses the port.
- **J1** — view tests under **jsdom/happy-dom** (add the dev dep) for the DOM-bound paths that
  are currently demo-only: `mountVellum` assembly, virtualization windowing, caret/selection
  registration, and a **two-editor** test (D2 — proves blocker #2's no-clobber, not just argued).
- **J2** — update the Vite demo: a **second editor instance** (two-editor coexistence),
  an undo/redo button, keep the multibyte multi-line seed; remove the dead Inc-0 `input`-event
  logger (`ts/demo/src/main.ts` — no-op on EditContext); re-capture `docs/assets/` screenshot.
- **J3** — ADRs (consider the ADR-0003 keyboard note), CHANGELOG `[0.0.2]`, README updates.
- **J4** — verify full DoD locally + push; confirm GitLab pipeline green + GitHub mirror
  updated; tag `v0.0.2-inc1`. **Do not tick the undo/redo, two-editor, or IME DoD boxes until
  J1/J2 verify them end to end.**

**Deferred to Increment 2** (tracked in the audit, do NOT pull forward): full Unicode
line-break trimming in `line_byte_range` (C-M1); dev-assert on the paint clamp (C-M2);
width-aware caret/selection columns for CJK/astral (A-M2); incremental re-lex instead of
per-line whole-doc tokenization.

---

## How we work (house signature — non-negotiable)

- TDD (red→green→commit) + per-task `superpowers:code-reviewer`. Pure logic is unit-tested
  (Vitest DOM-free / `cargo test`); DOM/wasm-bound adapters are demo/jsdom/chrome-devtools-verified.
- `core` zero `unsafe`, minimal justified deps, typed errors, ADR-before-architecture.
- Every change ends green on the full gate set (Rust + wasm-pack + bun). Small Conventional
  Commits, linear history on `master`.
