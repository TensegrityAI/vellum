# Contributing to Vellum

Thank you for your interest in Vellum. This document describes how we work so that the
codebase stays small, pure, and trustworthy.

> Vellum is **pre-alpha** and currently private. External contributions open once the repo
> flips public (see [ADR-0005](docs/adr/0005-repo-packaging.md)).

## Branch naming

Branch off the default branch. Use a short, kebab-case, prefixed name:

- `feat/<slug>` — a new capability
- `fix/<slug>` — a bug fix
- `docs/<slug>` — documentation only
- `chore/<slug>` — tooling, config, housekeeping
- `refactor/<slug>` — behavior-preserving restructure

Example: `feat/grapheme-cursor`, `fix/tokenizer-unterminated-block`.

## Commit messages — Conventional Commits

Every commit follows [Conventional Commits](https://www.conventionalcommits.org/):

```
<type>(<scope>): <summary>
```

- **types:** `feat`, `fix`, `docs`, `chore`, `refactor`, `test`, `ci`, `perf`.
- **scopes** map to the layout: `core`, `wasm`, `view`, `react`, `demo`.
- Keep commits small and verifiable. One logical change per commit.

Examples:

```
feat(core): add char-boundary-safe insert/delete
feat(wasm): expose Editor with flat Uint32Array token wire
docs: add ADRs 0001-0005
```

## CI gates (all must be green)

A change is not done until the strongest relevant checks pass:

- **Rust:** `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`,
  `cargo test`, `cargo deny check`.
- **WASM:** `wasm-pack test --node crates/wasm` and a successful `scripts/build-wasm.sh`.
- **TypeScript:** `bun run check` (tsc `--noEmit`) and `bun run test` (Vitest).

## Engineering rules

- **No `unsafe` in `core`.** The `core` crate is `#![forbid(unsafe_code)]` and must stay
  that way. The only place `unsafe` may appear is the `wasm` crate, and only as
  wasm-bindgen-generated glue. An unsafe-free editor core is a feature, not an accident.
- **No `todo!()` / `unimplemented!()` in merged code.** Do not leave fake completeness.
  If something is not wired, it does not merge.
- **ADR before architecture.** Any decision that changes boundaries, dependency direction,
  the wire format, the buffer model, or public API requires an ADR under `docs/adr/`
  (use [`0000-template.md`](docs/adr/0000-template.md)) accepted **before** the code lands.
- **Minimal, justified dependencies.** Each new dependency enters with a documented reason.
  The bar for `core` is especially high.
- **Typed errors.** Use `thiserror`; no stringly-typed errors in library code.
- **TDD.** Prefer red -> green -> commit. New behavior arrives with the test that proves it;
  bugs arrive with a failing regression test first.

## Definition of Done

- Tests added/updated and passing; all CI gates green.
- Public API changes carry doc comments.
- Non-obvious decisions recorded (ADR or `CHANGELOG.md` `## [Unreleased]` entry).
