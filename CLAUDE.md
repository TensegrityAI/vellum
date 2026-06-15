# CLAUDE.md — Vellum

The single source of truth for working in this repository is **[`AGENTS.md`](AGENTS.md)**.

Read it first. It carries the mission, the architecture (an edit is an event), the
crate/package layout and dependency direction
(`core ← lang-* ← wasm ← ts/view ← ts/react`), the quality non-negotiables
(forbid `unsafe` in `core`, minimal deps, typed errors with `thiserror`,
ADR-before-architecture, CI gates, Apache-2.0), and the increment order.

Supporting material:

- Architecture decisions: [`docs/adr/`](docs/adr/)
- Agent Operating Layer: [`.agents/README.md`](.agents/README.md)
- Instruction packs: [`.agents/instructions/`](.agents/instructions/)

Do not duplicate guidance here — update `AGENTS.md` instead.
