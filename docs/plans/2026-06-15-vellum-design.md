# Vellum — Design Document

> *The surface you write your prompts on.*
> A lightweight, Rust-cored, web-native code/prompt editor engine for the Nexum platform — built to escape the curse that 9 of 10 editors are just another Monaco/VSCode fork.

- **Status:** Design validated (brainstorm) — ready for implementation planning
- **Date:** 2026-06-15
- **Author:** Kael (Marcos) + Claude
- **License (intended):** Apache-2.0 (open source for Akaisys visibility)
- **Lineage:** Applies the [`kineticrs`](/home/nexus/workspace/rust-projects/kineticrs) house signature (enforcement over description, minimal deps, typed everything, event sourcing first-class, agent-ready) to the client.

---

## 1. Motivation

The legacy prompt "IDE" lives embedded in the Nexum admin `AgentModal`
(`nexum-rag-client/tmp/to-migrate/frontend`, core file `PromptTemplateEditor.tsx`,
1,436 lines). It is feature-rich but heavy and entangled:

- **Monaco** (~30MB self-hosted) + **MUI v7** + framer-motion + full lodash in one bundle.
- Monolithic components, no code splitting for modals, eager Jinja2 plugin (~1.4K lines).
- Tightly coupled to `agentId` + `admin-store` + `lib/api/admin.ts` — **not reusable standalone**.

Pulling it into the new Nexum app as-is would drag that weight into the bundle. Instead we
build a **separate, domain-agnostic, publishable engine** we can iterate on in isolation.

### Why build, not adopt (Monaco / CodeMirror)?

Monaco and CodeMirror are general-purpose editors carrying years of polyfills and hacks
(a `<span>` per token, `contenteditable` fighting the IME, hidden textareas) **because the
native platform APIs did not exist when they were built.** In 2026 they do. We only need to
be excellent at a small subset (prompts + a few languages), so a focused, pure, minimal-dep
engine wins on bundle size, security, reliability, and maintainability — and frees us from
the Monaco/VSCode-fork monoculture. The goal is **not** to compete; it is to **consume it
ourselves, built for tomorrow**, coherent with what `kineticrs` is saying to the world.

---

## 2. The 2026 web platform (verified)

| Tech | Status (mid-2026) | Problem it eliminates |
|------|-------------------|------------------------|
| **CSS Custom Highlight API** | ✅ Baseline since Jun 2025 (Firefox 140 closed the gap), cross-browser | Syntax highlighting by styling `Range` objects via `::highlight()` — **no `<span>` per token** (the thing that bloats Monaco/CodeMirror). |
| **CSS Anchor Positioning + Popover API** | ✅ Baseline 2026 (Chrome 125+, FF 147+, Safari 26) | Autocomplete, hover, diagnostics popups with **zero JS positioning library** (no Floating UI). |
| **Pretext.js** (Cheng Lou, 15KB) | ✅ Stable | DOM-free text measurement/layout via Canvas + arithmetic, ~500× faster, full i18n/bidi. We **learn from it**, do not depend on it. |
| **EditContext API** | ⚠️ Chromium-only (Chrome/Edge 121+); **not** Baseline (no Safari/Firefox yet) | Native IME/composition for custom editors — the one non-portable piece → progressive enhancement. |

Sources: MDN (EditContext, CSS Custom Highlight API), Chrome for Developers (EditContext),
OddBird (Anchor Positioning Baseline 2026), bram.us (Highlight API for syntax highlighting),
pretextjs.dev.

---

## 3. Core architectural concept

**An edit is an event.** The `kineticrs` worldview maps perfectly onto an editor:

- The document is an **aggregate** (rope buffer + version).
- Each keystroke is a **domain event** (`CharInserted`, `RangeDeleted`, `SelectionMoved`).
- **Undo/redo = replay/reverse of events** — not an ad-hoc stack; event sourcing.
- **Time-travel and future collaboration (CRDT/OT)** are natural extensions of the same model,
  not a rewrite.

The editor *breathes* the same architecture as the backend.

### Repository / crate layout

```
crates/
  core/        # PURE Rust. Rope buffer, edits (event-sourced), undo/redo,
               # cursor/selection, Language trait, layout arithmetic, diff.
               # #![forbid(unsafe_code)]. Minimal deps. 100% testable, NO browser, NO WASM here.
  lang-jinja/  # First language plugin: Jinja2 tokenizer + parser + lint + completion + hover.
  wasm/        # wasm-bindgen bindings: expose core to JS. Emits token ranges + diagnostics.
ts/
  view/        # Thin view: Highlight API + InputSource (EditContext/textarea) + Anchor/Popover + layout.
  react/       # React/Next wrapper for Nexum (the consumer).
```

- `core` knows nothing about the browser or about prompts.
- `lang-jinja` is the first language; `lang-sql`, `lang-md`, … come later.
- `ts/view` is the only layer touching the DOM.
- Nexum consumes `ts/react`.

---

## 4. View layer & progressive input

The view stays thin and native. The one real risk — **EditContext is Chromium-only in 2026** —
is solved with progressive enhancement behind a single `InputSource` port:

- **Chromium** → `EditContextInput` adapter: native IME/composition (best experience).
- **Safari/Firefox** → `HiddenTextareaInput` adapter: synced hidden textarea (the proven
  CodeMirror pattern). Works everywhere, correct IME, accessible.
- **Tests** → `FakeInput`: inject keystrokes with no DOM (agent-ready, fast).

The view does not know which adapter is active; it requests characters/composition from the port.

Everything else is native, **no positioning libraries**:

- **Render:** real-text DOM (virtualized lines) → accessibility & native selection intact
  (we deliberately avoid canvas, which would break screen readers).
- **Highlight:** core emits ranges → `CSS.highlights` + `::highlight()`. Zero spans.
- **Popups** (autocomplete / hover / diagnostics): Popover API + Anchor Positioning.

### The `Language` trait (extension spine of "Option B")

```rust
trait Language {
    fn tokenize(&self, doc: &Rope, range: Range) -> Vec<Token>;   // → Highlight API
    fn diagnostics(&self, doc: &Rope) -> Vec<Diagnostic>;          // → lint / popups
    fn complete(&self, doc: &Rope, pos: Pos) -> Vec<Completion>;   // → autocomplete
    fn hover(&self, doc: &Rope, pos: Pos) -> Option<Hover>;
}
```

`lang-jinja` is the first implementation; the engine is language-agnostic, the language is a typed plugin.

---

## 5. Performance discipline (lessons from Pretext)

Pretext's edge is a **discipline**, not magic: touch the impure/slow thing **once**, cache it,
then do everything as pure arithmetic. It maps directly onto our hexagonal split.

1. **The enemy is forced synchronous reflow.** Reading `getBoundingClientRect`/`offsetHeight`
   forces a full layout recalculation (~94ms / 1000 items). **Golden rule of `core`: never
   measure via the DOM in hot paths.**
2. **Two phases = a measurement port.** Pretext measures with Canvas `measureText()` (no reflow)
   once per `(segment, font)`, caches, then sums. This *is* a hexagonal port:
   - **`MeasurePort` (outbound, impure)** → Canvas adapter in TS: `advance(grapheme, font) → width`.
     The only thing touching the browser.
   - **Layout = pure arithmetic in Rust** over rope + cached widths → line breaks, wrap, caret
     pixel position, visible viewport range. **Zero reflow, 100% testable without a DOM.**
3. **Our advantage over Pretext: monospace.** A code editor is monospaced; while Pretext handles
   the general proportional case, our cache collapses to *one advance per grapheme class* and
   layout becomes `column × advance` — faster and simpler than Pretext on its own turf.
4. **Graphemes, not bytes.** The classic editor bug (emoji, combining marks, CJK breaking the
   caret) comes from moving by byte. The core operates on **grapheme clusters** + Unicode
   line-break opportunities.

**Decision:** segmentation via the `unicode-segmentation` crate in `core` (tiny, vetted, keeps
the core autonomous and testable without a browser), leaving a clean seam to swap to the
browser's `Intl.Segmenter` at the view boundary if ever desired. → ADR.

---

## 6. Scope & increments (YAGNI)

Legacy had a lot (AI Assist, version history, diff viewer, snippets, multi-template, variables
sidebar, preview). We slice it so each increment is shippable and de-risks the next.

- **Increment 0 — Walking skeleton (the bet).** Standalone demo, no Nexum.
  - `core` Rust: rope + basic edits → WASM.
  - `view`: `HiddenTextareaInput` (simplest first), DOM line render.
  - Highlight API with a trivial Jinja2 tokenizer.
  - **Result:** type → see highlighting. If this shines, everything else is built on confidence.
- **Increment 1 — Real (generic) editor.** Event-sourced edits + **undo/redo**, cursor/selection,
  virtualized layout (arithmetic), `EditContextInput` adapter + IME, real `Language` trait.
- **Increment 2 — Full `lang-jinja`.** Tokenizer/parser/lint/autocomplete/hover for Jinja2.
  Variables as completions (injected by host → domain-agnostic).
- **Increment 3 — Prompt layer + Nexum.** `ts/react` wrapper, host ports (load/save/variables),
  live Jinja2 preview, and **replace the legacy editor** in Nexum.

**Deferred (postponed, not deleted — until the foundation is solid):**

- 🕒 AI Assist (prompt quality analysis)
- 🕒 Version history + diff viewer
- 🕒 Snippets library
- 🕒 Additional languages (`lang-sql`, `lang-md`)

Principle: an **excellent generic editor** first; the prompt domain on top; the premium features
(AI, versions) once the foundations hold.

---

## 7. Quality signature, governance & open source

Inherited wholesale from `kineticrs` — it is what makes the engine trustworthy and maintainable:

- **`#![forbid(unsafe_code)]`** in `core`. An unsafe-free editor is a strong selling point.
- **Minimal, justified dependencies** — each crate enters with a documented reason (ideally
  `unicode-segmentation` and little else in `core`).
- **Typed errors** (`thiserror`), no string errors.
- **CI as guardrails:** `clippy -D warnings`, `fmt --check`, `cargo deny`, `cargo audit`,
  high coverage on the pure `core`.
- **ADRs with frontmatter** for every decision.
- **Agent-ready:** `.agents/` with instructions + skills (mirroring `kineticrs`) so iteration
  happens without architectural drift.
- **Apache-2.0**, clean repo to open to the world when ready → Akaisys visibility.

---

## 8. Open decisions → ADRs to write

1. **ADR-0001** — Grapheme segmentation: `unicode-segmentation` (core) vs `Intl.Segmenter` (view). *(leaning: core crate)*
2. **ADR-0002** — Event-sourced buffer model (event shapes, snapshotting, undo/redo as reverse events).
3. **ADR-0003** — Input progressive enhancement (`InputSource` port: EditContext + textarea fallback).
4. **ADR-0004** — Measurement port & pure-arithmetic layout (no DOM reads in hot paths).
5. **ADR-0005** — Repo & packaging (separate OSS repo; `@nexum/*` or neutral `@vellum/*` scopes; GitLab vs GitHub).

---

## 9. Naming

**Vellum** — fine parchment, the writing surface. Editorial and elegant; coherent with the
house "paper-and-ink" aesthetic (TO-K Atelier). Conveys lightness and craft.
*Vellum: the surface you write your prompts on.*
