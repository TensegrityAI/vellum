# Vellum — Foundations + Increment 0 (Walking Skeleton) Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Stand up the `vellum` OSS-native repo (Cargo workspace + TS + governance + CI) and ship Increment 0 — a standalone demo where typing Jinja2 shows live syntax highlighting, with every text operation flowing through a Rust core compiled to WASM.

**Architecture:** Pure Rust `core` (text buffer + trivial Jinja2 tokenizer, `#![forbid(unsafe_code)]`) → `wasm` bindings (wasm-bindgen, tokens crossing as a flat `Uint32Array`) → thin TS `view` that renders real-text DOM, captures input via a hidden textarea, and paints syntax with the **CSS Custom Highlight API** (zero `<span>`s). No editor framework, no positioning library.

**Tech Stack:** Rust (edition 2021, MSRV 1.89), wasm-bindgen + wasm-pack, TypeScript + Bun + Vite (demo), Vitest, GitHub Actions, Apache-2.0.

**Repo root (all paths relative to it):** `/home/nexus/workspace/vellum`

**Reference design:** `/home/nexus/workspace/rag-apptlas/docs/plans/2026-06-15-vellum-design.md`
**House-signature references:** `/home/nexus/workspace/rust-projects/kineticrs`, `/home/nexus/learning/boilerplates/rust-backend-HEX-ES-starter`

**Conventions for every task:** DRY, YAGNI, TDD (red → green → commit), Conventional Commits, small commits.

---

## Phase A — Repo foundation & governance

### Task A1: Initialize repo & workspace skeleton

**Files:**
- Create: `Cargo.toml` (workspace root)
- Create: `.gitignore`, `rust-toolchain.toml`
- Create: `crates/core/Cargo.toml`, `crates/core/src/lib.rs`

**Step 1:** Create the directory and init git.
```bash
mkdir -p /home/nexus/workspace/vellum/crates/core/src
cd /home/nexus/workspace/vellum && git init
```

**Step 2:** Write workspace `Cargo.toml`:
```toml
[workspace]
resolver = "2"
members = ["crates/core"]

[workspace.package]
edition = "2021"
rust-version = "1.89"
license = "Apache-2.0"
repository = "https://github.com/akaisys/vellum"
authors = ["Akaisys"]

[workspace.lints.rust]
unsafe_code = "forbid"

[workspace.lints.clippy]
all = { level = "deny", priority = -1 }
```

**Step 3:** Write `rust-toolchain.toml`:
```toml
[toolchain]
channel = "1.89"
components = ["rustfmt", "clippy", "rust-src"]
```

**Step 4:** Write `crates/core/Cargo.toml`:
```toml
[package]
name = "vellum-core"
version = "0.0.0"
edition.workspace = true
rust-version.workspace = true
license.workspace = true

[lints]
workspace = true
```

**Step 5:** Write `crates/core/src/lib.rs`:
```rust
//! Vellum core — pure, framework-free editor engine. No browser, no WASM here.
#![forbid(unsafe_code)]
```

**Step 6:** Standard Rust `.gitignore` (`/target`, `node_modules`, `dist`, `pkg`, `.env`).

**Step 7:** Verify and commit.
```bash
cargo build
git add -A && git commit -m "chore: initialize vellum cargo workspace"
```
Expected: clean build.

---

### Task A2: Tooling config (fmt, clippy, deny)

**Files:** Create `rustfmt.toml`, `clippy.toml`, `deny.toml`.

**Step 1:** `rustfmt.toml`:
```toml
edition = "2021"
max_width = 100
```

**Step 2:** `clippy.toml`:
```toml
msrv = "1.89.0"
```

**Step 3:** `deny.toml` — allow only permissive licenses, deny known advisories:
```toml
[advisories]
yanked = "deny"

[licenses]
allow = ["Apache-2.0", "MIT", "BSD-3-Clause", "BSD-2-Clause", "ISC", "Unicode-3.0"]
confidence-threshold = 0.9
```

**Step 4:** Verify and commit.
```bash
cargo fmt --check && cargo clippy --all-targets -- -D warnings
git add -A && git commit -m "chore: add rustfmt, clippy, cargo-deny config"
```

---

### Task A3: OSS governance documents

**Files:** Create `LICENSE` (Apache-2.0 full text), `README.md`, `CONTRIBUTING.md`, `CODE_OF_CONDUCT.md`, `CHANGELOG.md`, `SECURITY.md`.

**Step 1:** Drop the full Apache-2.0 text into `LICENSE` (fetch canonical text).

**Step 2:** `README.md` — tagline *"Vellum: the surface you write your prompts on."*, the "why not Monaco/CodeMirror" pitch, the 2026-platform table, build/run instructions (filled as later tasks land), and a "Status: pre-alpha, private until Increment 0" note.

**Step 3:** `CONTRIBUTING.md` — mirror kineticrs: branch naming, Conventional Commits, the CI gates, ADR requirement for architectural changes, "no `unsafe` in core", "no `todo!()` in merged code".

**Step 4:** `CODE_OF_CONDUCT.md` (Contributor Covenant 2.1), `SECURITY.md` (private disclosure to security@akaisys.com), empty `CHANGELOG.md` with `## [Unreleased]`.

**Step 5:** Commit.
```bash
git add -A && git commit -m "docs: add OSS governance (license, contributing, coc, security)"
```

---

### Task A4: ADR scaffolding

**Files:** Create `docs/adr/0000-template.md` + the five decision records from the design doc (status `accepted` for 0002–0004, `proposed` for 0001/0005).

**Step 1:** Template with frontmatter (`status`, `date`, `tags`, `related`), mirroring kineticrs ADR format.

**Step 2:** Write:
- `0001-grapheme-segmentation.md` — `unicode-segmentation` in core vs `Intl.Segmenter` in view (decision: core crate; seam left at view).
- `0002-event-sourced-buffer.md` — edits as events; undo/redo = reverse events; snapshotting deferred.
- `0003-input-progressive-enhancement.md` — `InputSource` port: EditContext (Chromium) + hidden-textarea fallback.
- `0004-measurement-port-and-arithmetic-layout.md` — no DOM reads in hot paths; Canvas measure once; layout is pure arithmetic; monospace fast path.
- `0005-repo-packaging.md` — separate OSS repo; `@vellum/*` npm scope; private→public flip after Inc 0. (status: proposed)

**Step 3:** Commit.
```bash
git add -A && git commit -m "docs: add ADRs 0001-0005"
```

---

### Task A5: Agent Operating Layer (`.agents/`)

**Files:** Create `.agents/README.md`, `.agents/instructions/rust-style.instructions.md`, `.agents/instructions/hexagonal-boundaries.instructions.md`, `.agents/instructions/tests.instructions.md`, `AGENTS.md`, `CLAUDE.md` (symlink or pointer to AGENTS.md).

**Step 1:** `AGENTS.md` — project soul (the design doc's §1, §3, §7 condensed), the non-negotiables (forbid unsafe in core, minimal deps, typed errors, ADR-before-architecture, increments order), and where things live.

**Step 2:** Instruction files mirroring kineticrs `.github/instructions/` content, scoped to Vellum (Rust style; the core↔wasm↔ts dependency direction; behavior-named tests, AAA, TDD-on-bugs).

**Step 3:** `CLAUDE.md` pointing to `AGENTS.md` (single source of truth).

**Step 4:** Commit.
```bash
git add -A && git commit -m "docs: add Agent Operating Layer (.agents, AGENTS.md)"
```

---

## Phase B — Increment 0 core (Rust, TDD)

> Scope discipline: **no rope, no event sourcing, no grapheme cursor yet** (those are Increment 1). A `String`-backed buffer with char-boundary-safe ops and a trivial tokenizer is enough to prove the pipeline.

### Task B1: `TextBuffer` — construction & read

**Files:**
- Modify: `crates/core/src/lib.rs` (add `mod buffer;`)
- Create: `crates/core/src/buffer.rs`

**Step 1: Write the failing test** (in `buffer.rs` under `#[cfg(test)]`):
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_buffer_is_empty() {
        let buf = TextBuffer::new();
        assert_eq!(buf.text(), "");
        assert_eq!(buf.len(), 0);
    }

    #[test]
    fn from_str_round_trips() {
        let buf = TextBuffer::from_str("Hello {{ name }}");
        assert_eq!(buf.text(), "Hello {{ name }}");
    }
}
```

**Step 2: Run → fail.** `cargo test -p vellum-core` → FAIL (`TextBuffer` undefined).

**Step 3: Minimal impl** (top of `buffer.rs`):
```rust
/// A simple string-backed text buffer. Replaced by a rope in Increment 1 (ADR-0002).
#[derive(Debug, Default, Clone)]
pub struct TextBuffer {
    text: String,
}

impl TextBuffer {
    pub fn new() -> Self { Self::default() }
    pub fn from_str(s: &str) -> Self { Self { text: s.to_string() } }
    pub fn text(&self) -> &str { &self.text }
    pub fn len(&self) -> usize { self.text.len() }
    pub fn is_empty(&self) -> bool { self.text.is_empty() }
}
```
Add `mod buffer;` and `pub use buffer::TextBuffer;` to `lib.rs`.

**Step 4: Run → pass.** `cargo test -p vellum-core`.

**Step 5: Commit.** `git commit -am "feat(core): add TextBuffer construction and read"`

---

### Task B2: `TextBuffer` — insert & delete (char-boundary safe)

**Files:** Modify `crates/core/src/buffer.rs`.

**Step 1: Failing tests:**
```rust
#[test]
fn insert_at_start_middle_end() {
    let mut buf = TextBuffer::from_str("Helo");
    buf.insert(3, "l");                 // byte offset
    assert_eq!(buf.text(), "Hello");
}

#[test]
fn delete_range_removes_text() {
    let mut buf = TextBuffer::from_str("Hello world");
    buf.delete(5..11);
    assert_eq!(buf.text(), "Hello");
}

#[test]
#[should_panic]
fn insert_on_non_char_boundary_panics() {
    let mut buf = TextBuffer::from_str("áé"); // multibyte
    buf.insert(1, "x");                 // splits 'á'
}
```

**Step 2: Run → fail.**

**Step 3: Minimal impl** (use `String::insert_str` / `String::replace_range`, which already panic on non-char-boundaries — document that contract):
```rust
impl TextBuffer {
    /// Insert `s` at byte offset `at`. Panics if `at` is not a char boundary.
    pub fn insert(&mut self, at: usize, s: &str) {
        self.text.insert_str(at, s);
    }

    /// Delete the byte range. Panics if bounds are not char boundaries.
    pub fn delete(&mut self, range: std::ops::Range<usize>) {
        self.text.replace_range(range, "");
    }
}
```

**Step 4: Run → pass.**

**Step 5: Commit.** `git commit -am "feat(core): add char-boundary-safe insert/delete"`

---

### Task B3: Token model

**Files:** Modify `crates/core/src/lib.rs` (add `mod token;`); Create `crates/core/src/token.rs`.

**Step 1: Failing test:**
```rust
#[test]
fn token_kind_maps_to_stable_u32() {
    assert_eq!(TokenKind::Text as u32, 0);
    assert_eq!(TokenKind::Variable as u32, 1);
    assert_eq!(TokenKind::Statement as u32, 2);
    assert_eq!(TokenKind::Comment as u32, 3);
}
```
(The stable u32 mapping is the WASM wire contract — see Phase C.)

**Step 2: Run → fail.**

**Step 3: Impl:**
```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum TokenKind { Text = 0, Variable = 1, Statement = 2, Comment = 3 }

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Token { pub start: usize, pub end: usize, pub kind: TokenKind }
```

**Step 4: Run → pass.**

**Step 5: Commit.** `git commit -am "feat(core): add Token / TokenKind model"`

---

### Task B4: Trivial Jinja2 tokenizer

**Files:** Modify `crates/core/src/lib.rs` (add `mod lang_jinja;`); Create `crates/core/src/lang_jinja.rs`.

> Increment 0 keeps the tokenizer in-core as a function. It graduates to the `lang-jinja` crate + `Language` trait in Increment 2.

**Step 1: Failing tests:**
```rust
#[test]
fn plain_text_is_one_text_token() {
    let toks = tokenize("hello");
    assert_eq!(toks, vec![Token { start: 0, end: 5, kind: TokenKind::Text }]);
}

#[test]
fn variable_block_is_tokenized() {
    let toks = tokenize("a {{ x }} b");
    assert_eq!(toks, vec![
        Token { start: 0, end: 2, kind: TokenKind::Text },
        Token { start: 2, end: 9, kind: TokenKind::Variable },
        Token { start: 9, end: 11, kind: TokenKind::Text },
    ]);
}

#[test]
fn statement_and_comment_blocks() {
    assert_eq!(tokenize("{% if x %}")[0].kind, TokenKind::Statement);
    assert_eq!(tokenize("{# c #}")[0].kind, TokenKind::Comment);
}

#[test]
fn unterminated_block_runs_to_end() {
    let toks = tokenize("{{ x");
    assert_eq!(toks, vec![Token { start: 0, end: 4, kind: TokenKind::Variable }]);
}
```

**Step 2: Run → fail.**

**Step 3: Impl** — single left-to-right scan over bytes, matching `{{`/`}}`, `{%`/`%}`, `{#`/`#}`, emitting Text between blocks. Return `Vec<Token>`. (Write the scanner; keep it allocation-light and `O(n)`.)

**Step 4: Run → pass.** Add a `proptest` later (Increment 2); not in Inc 0.

**Step 5: Commit.** `git commit -am "feat(core): add trivial Jinja2 tokenizer"`

---

## Phase C — WASM bindings

### Task C1: `wasm` crate scaffold

**Files:**
- Modify root `Cargo.toml` (add `crates/wasm` to members)
- Create `crates/wasm/Cargo.toml`, `crates/wasm/src/lib.rs`

**Step 1:** `crates/wasm/Cargo.toml`:
```toml
[package]
name = "vellum-wasm"
version = "0.0.0"
edition.workspace = true
rust-version.workspace = true
license.workspace = true

[lib]
crate-type = ["cdylib", "rlib"]

[dependencies]
vellum-core = { path = "../core" }
wasm-bindgen = "0.2"

[dev-dependencies]
wasm-bindgen-test = "0.3"

[lints]
workspace = true
```
> Note: the `wasm` crate is the **only** place `unsafe` may appear (generated by wasm-bindgen). Core stays `forbid(unsafe_code)`.

**Step 2:** Verify it builds for host: `cargo build -p vellum-wasm`.

**Step 3:** Commit. `git commit -am "chore(wasm): scaffold wasm-bindgen crate"`

---

### Task C2: `Editor` binding with flat token wire format

**Files:** Modify `crates/wasm/src/lib.rs`.

**Wire contract:** `tokens()` returns a `Vec<u32>` flattened as `[start, end, kind, start, end, kind, ...]` → JS sees a `Uint32Array`. No serde dependency.

**Step 1: Failing test** (`wasm-bindgen-test`, runs in headless browser/node):
```rust
use wasm_bindgen_test::*;
use vellum_wasm::Editor;

#[wasm_bindgen_test]
fn insert_then_tokens_roundtrip() {
    let mut ed = Editor::new("a ");
    ed.insert(2, "{{ x }}");
    assert_eq!(ed.text(), "a {{ x }}");
    let t = ed.tokens(); // [0,2,0, 2,9,1]
    assert_eq!(t, vec![0, 2, 0, 2, 9, 1]);
}
```

**Step 2: Run → fail.** `wasm-pack test --node crates/wasm`.

**Step 3: Impl:**
```rust
#![allow(unsafe_code)] // wasm-bindgen generated glue only
use wasm_bindgen::prelude::*;
use vellum_core::{tokenize, TextBuffer};

#[wasm_bindgen]
pub struct Editor { buf: TextBuffer }

#[wasm_bindgen]
impl Editor {
    #[wasm_bindgen(constructor)]
    pub fn new(initial: &str) -> Editor { Editor { buf: TextBuffer::from_str(initial) } }
    pub fn text(&self) -> String { self.buf.text().to_string() }
    pub fn insert(&mut self, at: usize, s: &str) { self.buf.insert(at, s); }
    pub fn delete(&mut self, start: usize, end: usize) { self.buf.delete(start..end); }
    pub fn tokens(&self) -> Vec<u32> {
        tokenize(self.buf.text()).iter()
            .flat_map(|t| [t.start as u32, t.end as u32, t.kind as u32])
            .collect()
    }
}
```

**Step 4: Run → pass.**

**Step 5: Commit.** `git commit -am "feat(wasm): expose Editor with flat Uint32Array token wire"`

---

### Task C3: Build script for the web target

**Files:** Create `scripts/build-wasm.sh`.

**Step 1:** Script: `wasm-pack build crates/wasm --target web --out-dir ../../ts/view/wasm --out-name vellum` (+ `set -euo pipefail`). Document `wasm-pack` install in README.

**Step 2:** Run it; verify `ts/view/wasm/vellum.js` + `.wasm` are produced (and gitignored — built artifact).

**Step 3:** Commit. `git commit -am "chore(wasm): add web build script"`

---

## Phase D — TS view + demo

### Task D1: TS workspace + Vitest

**Files:** Create `ts/view/package.json`, `ts/view/tsconfig.json`, `ts/view/vitest.config.ts`, `package.json` (root, Bun workspaces).

**Step 1:** Root `package.json` with `"workspaces": ["ts/*"]`, scripts `check` (tsc --noEmit), `test` (vitest run). Use Bun.

**Step 2:** `ts/view` with `typescript`, `vitest` dev deps; strict `tsconfig`.

**Step 3:** Verify `bun install` + `bun run check` pass on an empty `src/index.ts`.

**Step 4:** Commit. `git commit -am "chore(ts): scaffold Bun workspace + Vitest for view package"`

---

### Task D2: Pure token→highlight mapping (unit-tested)

**Files:** Create `ts/view/src/highlights.ts`, `ts/view/src/highlights.test.ts`.

> The DOM-free, testable heart of the view: turn the flat `Uint32Array` into per-kind offset groups.

**Step 1: Failing test:**
```ts
import { describe, it, expect } from "vitest";
import { groupTokensByKind } from "./highlights";

describe("groupTokensByKind", () => {
  it("groups flat token triples by kind", () => {
    const flat = new Uint32Array([0, 2, 0, 2, 9, 1]);
    expect(groupTokensByKind(flat)).toEqual({
      0: [[0, 2]],
      1: [[2, 9]],
    });
  });
});
```

**Step 2: Run → fail.** `bun run test`.

**Step 3: Impl** `groupTokensByKind(flat: Uint32Array): Record<number, [number, number][]>` — iterate in steps of 3.

**Step 4: Run → pass.**

**Step 5: Commit.** `git commit -am "feat(view): add pure token grouping for Highlight API"`

---

### Task D3: The view component (DOM render + hidden-textarea input + Highlight API)

**Files:** Create `ts/view/src/view.ts`, `ts/view/src/highlight-styles.css`, modify `ts/view/src/index.ts`.

> Manual/visual task — keep logic thin; the testable parts already live in `highlights.ts`.

**Step 1:** `mountVellum(host: HTMLElement, editor: Editor)`:
- Render a `<div class="vellum-surface">` containing a single text node (or one `<div>` per line) with the buffer text.
- Overlay a transparent `<textarea class="vellum-input">` (the `HiddenTextareaInput` adapter) sized to the surface; route its `input`/`beforeinput` to `editor.insert/delete`, then re-read `editor.text()` and re-render.
- After each render, build `Range` objects from `groupTokensByKind(editor.tokens())` mapped onto the surface text node, and register them: `CSS.highlights.set("vellum-variable", new Highlight(...ranges))` per kind.

**Step 2:** `highlight-styles.css`:
```css
::highlight(vellum-variable) { color: #7c5cff; }
::highlight(vellum-statement){ color: #d08770; }
::highlight(vellum-comment)  { color: #8a8f98; font-style: italic; }
```

**Step 3:** Export `mountVellum` from `index.ts`. `bun run check` passes.

**Step 4:** Commit. `git commit -am "feat(view): DOM render + textarea input + Highlight API painting"`

---

### Task D4: Demo playground (Vite)

**Files:** Create `ts/demo/` (Vite app): `index.html`, `src/main.ts`, `package.json`, `vite.config.ts`.

**Step 1:** `main.ts`: import the built `vellum` wasm (`init()` then `new Editor("Hello {{ name }}, {# note #} {% if x %}...")`), call `mountVellum(document.getElementById("app"), editor)`, import the highlight CSS.

**Step 2:** Run `scripts/build-wasm.sh` then `bun run --cwd ts/demo dev`; open the page.

**Step 3: VERIFY (the Increment-0 acceptance):** Typing `{{ var }}`, `{% if %}`, `{# c #}` shows live, correctly-colored highlighting; all edits round-trip through WASM (confirm via a `console.log(editor.text())`). Capture a screenshot into `docs/assets/inc0-demo.png`.

**Step 4:** Commit. `git commit -am "feat(demo): standalone Vite playground for Increment 0"`

---

## Phase E — CI & closeout

### Task E1: GitHub Actions CI

**Files:** Create `.github/workflows/ci.yml`.

**Step 1:** Jobs:
- **rust:** checkout → toolchain (1.89 + clippy/rustfmt) → `cargo fmt --check` → `cargo clippy --all-targets -- -D warnings` → `cargo test` → install `cargo-deny` → `cargo deny check`.
- **wasm:** install `wasm-pack` → `wasm-pack test --node crates/wasm` → `scripts/build-wasm.sh`.
- **ts:** setup Bun → `bun install` → `bun run check` → `bun run test`.

**Step 2:** Push a branch, confirm all jobs green (or run `act`/manual locally if no remote yet).

**Step 3:** Commit. `git commit -am "ci: add rust + wasm + ts pipeline"`

---

### Task E2: README run instructions + CHANGELOG + tag

**Files:** Modify `README.md`, `CHANGELOG.md`.

**Step 1:** Fill README "Build & Run" (install rust 1.89, `wasm-pack`, `bun`; `scripts/build-wasm.sh`; `bun run --cwd ts/demo dev`), embed the demo screenshot.

**Step 2:** `CHANGELOG.md` → `## [0.0.1] - Increment 0` summary.

**Step 3:** Commit + tag. `git commit -am "docs: Increment 0 run instructions" && git tag v0.0.1-inc0`

---

## Definition of Done (Increment 0)

- [ ] `cargo test`, `cargo clippy -D warnings`, `cargo fmt --check`, `cargo deny check` all green.
- [ ] `wasm-pack test --node crates/wasm` green; `core` has zero `unsafe`.
- [ ] `bun run check` + `bun run test` green.
- [ ] Demo: typing Jinja2 shows live Highlight-API coloring; edits round-trip through WASM (no `<span>` per token).
- [ ] Governance present: LICENSE, README, CONTRIBUTING, CoC, SECURITY, 5 ADRs, `.agents/` + AGENTS.md.
- [ ] CI green on all three jobs.
- [ ] Repo is OSS-ready (clean history, no secrets) but remains **private** until the demo is judged to "shine" → then flip public (ADR-0005).

---

## What Increment 0 deliberately does NOT do (next increments)

- Rope buffer, event-sourced edits, undo/redo → **Increment 1** (ADR-0002).
- `EditContext` adapter + IME, grapheme cursor, virtualized layout, `MeasurePort` → **Increment 1** (ADR-0003/0004).
- `Language` trait + `lang-jinja` crate (parser/lint/autocomplete/hover) → **Increment 2**.
- `ts/react` wrapper, host ports, live preview, Nexum replacement → **Increment 3**.
- AI Assist, version history + diff, snippets → **Deferred**.
