---
description: 'Testing discipline for Vellum: TDD, AAA, behavior-named tests, TDD-on-bugs.'
applyTo: 'crates/**/*.rs, ts/**/*.ts, ts/**/*.tsx'
---

# Tests

Vellum is built test-first. The pure `core` is where coverage matters most,
because it carries the correctness-critical logic (buffer, segmentation, layout
arithmetic, tokenizing) and runs with no browser.

## TDD: red → green → commit

1. **Red.** Write the failing test first, expressing the behavior you want. Run it
   and confirm it fails for the right reason (the symbol/behavior is missing, not a
   typo).
2. **Green.** Write the minimal implementation that makes it pass.
3. **Commit.** Commit the working slice (Conventional Commits). Keep slices small.

Do not write implementation before its test exists.

## Structure: AAA

Each test follows **Arrange / Act / Assert**:

- **Arrange** the inputs and state.
- **Act** by calling the one behavior under test.
- **Assert** the observable outcome.

One behavior per test. Prefer clear literal expectations over computed ones.

## Naming: behavior-driven

Test names describe the behavior, not the implementation:

- Good: `new_buffer_is_empty`, `variable_block_is_tokenized`,
  `insert_on_non_char_boundary_panics`, `unterminated_block_runs_to_end`.
- Avoid: `test1`, `test_insert`, names tied to internal mechanics.

## TDD on bugs

When fixing a bug: **first write a failing test that reproduces it**, watch it
fail, then fix. The test stays as a regression guard. A bug fix without a test
that would have caught it is incomplete.

## Coverage and purity

- **High coverage on the pure `core`.** It has no DOM excuse — every branch is
  reachable in a plain `cargo test`.
- Use the `FakeInput` adapter (ADR-0003) and fakeable ports to test view behavior
  without a real browser where possible.
- WASM-boundary behavior is exercised with `wasm-bindgen-test`
  (`wasm-pack test --node crates/wasm`); the pure logic underneath it is already
  covered by `core` unit tests, so the WASM tests assert the wire contract, not
  the algorithm.
- TS pure logic (e.g. token grouping for the Highlight API) is unit-tested with
  Vitest, DOM-free.
