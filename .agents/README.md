# Agent Operating Layer

`.agents/` is the canonical home for Vellum's agent-operating assets: the
operating contract, instruction packs, and any future policies, workflows, or
evaluations. It exists so AI agents and human contributors iterate on the engine
**without architectural drift**.

## What the AOL is

A small, version-controlled set of rules that encode how work is done here — the
non-negotiables (forbid `unsafe` in `core`, minimal deps, typed errors,
ADR-before-architecture), the dependency direction, and the testing discipline —
so they are enforceable and reproducible, not folklore.

## How to use it

1. **Read `AGENTS.md` first** (repo root) — the project soul and operating
   contract. It is the single source of truth; `CLAUDE.md` points to it.
2. **Consult `docs/adr/`** before any change that touches architecture, ports, or
   boundaries. If your change makes such a decision, write the ADR first
   (template: `docs/adr/0000-template.md`).
3. **Apply the relevant instruction pack** for the layer you are editing:
   - `.agents/instructions/rust-style.instructions.md` — Rust conventions.
   - `.agents/instructions/hexagonal-boundaries.instructions.md` — the
     `core ← lang-* ← wasm ← ts/view ← ts/react` dependency direction and ports.
   - `.agents/instructions/tests.instructions.md` — TDD, AAA, behavior-named tests.
4. **Respect the increment order** (`AGENTS.md` §3). Do not pull later-increment
   work forward.

## Layout

- `.agents/README.md` — this file.
- `.agents/instructions/` — instruction packs (canonical AOL pack root).

New AOL instructions, policies, or workflows should be authored here first.
