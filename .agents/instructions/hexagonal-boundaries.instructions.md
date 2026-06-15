---
description: 'The Vellum dependency direction and port/adapter boundaries across crates and TS packages.'
applyTo: 'crates/**/*.rs, ts/**/*.ts, ts/**/*.tsx'
---

# Hexagonal boundaries

Vellum is hexagonal across the Rust ↔ WASM ↔ TS divide. The single most important
rule is the **dependency direction**.

## Dependency direction (strictly inward → outward)

```
core  ←  lang-*  ←  wasm  ←  ts/view  ←  ts/react
```

- **`core`** is pure. It knows **nothing** about the browser, WASM, or prompts. No
  DOM, no `wasm-bindgen`, no Jinja/prompt-specific logic. It is 100% testable
  without a browser.
- **`lang-*`** plugins (e.g. `lang-jinja`) depend on `core` only. They implement
  the `Language` trait; they do not reach outward to WASM or the DOM.
- **`wasm`** depends on `core` (and `lang-*`). It is the bindings layer and the
  only place `unsafe` (generated glue) is tolerated. It exposes core to JS.
- **`ts/view`** is the only layer that touches the DOM. It consumes the WASM
  bindings and implements adapters.
- **`ts/react`** wraps `ts/view` for the Nexum consumer.

**Never** let an inner layer import an outer one. `core` must not depend on
anything browser- or WASM-shaped; a DOM read inside `core` (or a `core` reference
to `wasm`) is a boundary violation.

## Ports are defined in `core`, adapters live outward

The ports are owned by `core`; their concrete adapters live in the outer layers:

- **`InputSource`** (port in core) — adapters: `EditContextInput` (Chromium),
  `HiddenTextareaInput` (Safari/Firefox), `FakeInput` (tests). See ADR-0003.
- **`MeasurePort`** (port in core) — adapter: Canvas `measureText()` in
  `ts/view`. **No DOM size reads (`getBoundingClientRect`/`offsetHeight`) in hot
  paths** — measurement happens once through this port, then layout is pure
  arithmetic in `core`. See ADR-0004.
- **`Language`** (trait in core) — implemented by `lang-*` plugins. See design §4.

## Consequences for everyday work

- New impure capability the engine needs → define the **port in `core` first**,
  then write the adapter in the outermost appropriate layer.
- If a feature seems to need the DOM inside `core`, that is the signal a port is
  missing — do not patch the symptom inward.
- Keep mapping/serialization at the `wasm` boundary (e.g. the flat
  `Uint32Array` token wire), not inside `core` domain types.
