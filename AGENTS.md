# AGENTS.md — Vellum

> Operating contract for AI coding agents working in this repository.
> Scope: applies to the whole `vellum` workspace unless a deeper `AGENTS.md` overrides it.
> Vellum is a **client-side editor engine**. There is no server, no database, no
> message broker here — do not introduce any.

---

## 0. Mission and philosophy

**Vellum is a lightweight, Rust-cored, web-native code/prompt editor engine.**
*The surface you write your prompts on.*

We build it to escape the curse that 9 of 10 editors are just another
Monaco/VSCode fork. The legacy prompt "IDE" lived embedded in the Nexum admin
`AgentModal` (`PromptTemplateEditor.tsx`, 1,436 lines): Monaco (~30MB) + MUI v7 +
framer-motion + full lodash, monolithic, eager Jinja2 plugin, and coupled to
`agentId` + admin-store — **not reusable standalone**. Instead of dragging that
weight into the new app, we build a separate, domain-agnostic, publishable engine.

### Why build, not adopt (Monaco / CodeMirror)?

Monaco and CodeMirror carry years of polyfills and hacks — a `<span>` per token,
`contenteditable` fighting the IME, hidden textareas — **because the native
platform APIs did not exist when they were built.** In 2026 they do (CSS Custom
Highlight API, Anchor Positioning + Popover, EditContext). We only need to be
excellent at a small subset (prompts + a few languages), so a focused, pure,
minimal-dep engine wins on bundle size, security, reliability, and
maintainability. The goal is not to compete; it is to **consume it ourselves,
built for tomorrow**, coherent with what `kineticrs` says to the world.

### Core posture

- **Foundations before velocity.** Protect repo contracts before adding code.
- **Small, verifiable, reversible changes.** Narrow slices with clear validation.
- **No fake completeness.** Do not leave `todo!()`, `unimplemented!()`, or docs
  that claim functionality that is not wired.
- **ADR before architecture.** A decision that changes boundaries, ports, or
  guarantees gets an ADR first (`docs/adr/`).

---

## 1. Architecture: an edit is an event

The `kineticrs` worldview maps onto an editor:

- The document is an **aggregate** (rope buffer + version).
- Each keystroke is a **domain event** (`CharInserted`, `RangeDeleted`,
  `SelectionMoved`).
- **Undo/redo = replay/reverse of events**, not an ad-hoc stack.
- Time-travel and future collaboration (CRDT/OT) are extensions of the same
  model, not a rewrite.

The editor breathes the same architecture as the backend. See ADR-0002.

### Crate / package layout and dependency direction

```
crates/
  core/        # PURE Rust. Buffer + edits (event-sourced), undo/redo,
               # cursor/selection, Language trait, layout arithmetic, diff.
               # #![forbid(unsafe_code)]. Minimal deps. NO browser, NO WASM, NO prompt knowledge.
  lang-jinja/  # First language plugin: Jinja2 tokenizer/parser/lint/completion/hover.
  wasm/        # wasm-bindgen bindings. The ONLY place unsafe may appear (generated glue).
ts/
  view/        # Thin view: Highlight API + InputSource + Anchor/Popover + layout. Only layer touching the DOM.
  react/       # React/Next wrapper for Nexum (the consumer).
```

**Dependency direction (strictly inward → outward):**

```
core  ←  lang-*  ←  wasm  ←  ts/view  ←  ts/react
```

- `core` knows nothing about the browser, WASM, or prompts.
- **Ports are defined in `core`** (`InputSource`, `MeasurePort`, `Language`);
  **adapters live outward** (Canvas measure, EditContext/textarea input, etc.).
- See ADR-0003 (input port), ADR-0004 (measurement port + arithmetic layout),
  ADR-0001 (grapheme segmentation in core). Boundary rules:
  `.agents/instructions/hexagonal-boundaries.instructions.md`.

---

## 2. Quality signature (non-negotiables)

Inherited wholesale from `kineticrs` — it is what makes the engine trustworthy:

- **`#![forbid(unsafe_code)]` in `core`.** An unsafe-free editor is a selling
  point. `wasm` is the only crate where unsafe (wasm-bindgen glue) is tolerated.
- **Minimal, justified dependencies.** Each crate enters with a documented reason
  (ideally `unicode-segmentation` and little else in `core`).
- **Typed errors** with `thiserror`. No string errors.
- **No `todo!()` / `unimplemented!()` in merged code.**
- **ADR before architecture.** Decisions that change boundaries get an ADR.
- **CI as guardrails:** `cargo fmt --check`, `cargo clippy --all-targets -D
  warnings`, `cargo test`, `cargo deny`, and `bun run check` / `bun run test` for
  TS. High coverage on the pure `core`.
- **TDD:** red → green → commit. Behavior-named tests, AAA structure.
  See `.agents/instructions/tests.instructions.md`.
- **Apache-2.0**, clean repo, OSS-ready from commit one (ADR-0005).

---

## 3. Increment order (YAGNI)

Each increment is shippable and de-risks the next. Do not pull later-increment
work forward.

- **Increment 0 — Walking skeleton (the bet).** Standalone demo, no Nexum.
  `core` (String buffer + trivial Jinja2 tokenizer) → WASM → thin view with
  `HiddenTextareaInput` + DOM line render + CSS Custom Highlight API. Result: type
  Jinja2, see live highlighting, every edit flows through WASM.
- **Increment 1 — Real generic editor.** Event-sourced rope + undo/redo,
  cursor/selection, virtualized arithmetic layout, `EditContextInput` + IME, real
  `Language` trait.
- **Increment 2 — Full `lang-jinja`.** Tokenizer/parser/lint/autocomplete/hover;
  variables as host-injected completions (domain-agnostic).
- **Increment 3 — Prompt layer + Nexum.** `ts/react` wrapper, host ports
  (load/save/variables), live Jinja2 preview, replace the legacy Nexum editor.

**Deferred (postponed, not deleted):** AI Assist (prompt quality analysis),
version history + diff viewer, snippets library, additional languages
(`lang-sql`, `lang-md`).

---

## 4. Where things live

| Resource | Path |
| --- | --- |
| Architecture decisions | `docs/adr/` (template: `docs/adr/0000-template.md`) |
| Design document | `docs/plans/2026-06-15-vellum-design.md` |
| Increment 0 plan | `docs/plans/2026-06-15-vellum-increment-0.md` |
| Agent Operating Layer | `.agents/` (start at `.agents/README.md`) |
| Rust conventions | `.agents/instructions/rust-style.instructions.md` |
| Boundary rules | `.agents/instructions/hexagonal-boundaries.instructions.md` |
| Test conventions | `.agents/instructions/tests.instructions.md` |
| Contributing / CI gates | `CONTRIBUTING.md` |

> Note: the design and plan docs currently live under `docs/plans/` in the
> `rag-apptlas` workspace during bootstrap; mirror or relink them into this repo as
> the OSS history is finalized (ADR-0005).
