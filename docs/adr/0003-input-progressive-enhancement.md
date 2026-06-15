---
status: accepted
date: 2026-06-15
tags: [view, input, ime, progressive-enhancement, ports]
related: [docs/plans/2026-06-15-vellum-design.md]
---

# ADR-0003: Input via an `InputSource` port with progressive enhancement

## Context

The one genuinely non-portable piece of the 2026 web platform is text input. The
**EditContext API** gives custom editors native IME/composition, but in mid-2026
it is **Chromium-only** (Chrome/Edge 121+) and not Baseline — no Safari, no
Firefox (design §2, §4). The view must still capture characters and composition
correctly on every browser, and tests must be able to inject keystrokes with no
DOM at all (agent-ready, fast).

Hard-coding any single input mechanism into the view would either break
non-Chromium browsers or make the view untestable without a browser.

## Decision

Define a single outbound **`InputSource` port**. The view requests
characters/composition from the port and never knows which mechanism is active.
Three adapters implement it:

- **`EditContextInput`** (Chromium) — native IME/composition, the best experience.
- **`HiddenTextareaInput`** (Safari/Firefox fallback) — a synced hidden textarea,
  the proven CodeMirror pattern: works everywhere, correct IME, accessible.
- **`FakeInput`** (tests) — injects keystrokes/composition with no DOM, enabling
  fast, deterministic, agent-driven tests of input behavior.

Selection between `EditContextInput` and `HiddenTextareaInput` is feature
detection at the view boundary; the editor core and view logic are written once
against the port.

**Increment scope:** Increment 0 ships only `HiddenTextareaInput` (simplest
first). `EditContextInput` and IME handling arrive in **Increment 1**. The port is
introduced from the start so the Increment 0 textarea is one adapter behind the
contract, not a special case to refactor later.

## Consequences

- Best-in-class IME on Chromium without sacrificing correctness or accessibility
  on Safari/Firefox — progressive enhancement, not a lowest-common-denominator.
- Input behavior is testable with `FakeInput` and no browser, supporting the
  agent-ready and high-coverage goals.
- The view depends on a port, not a browser API, keeping the dependency direction
  outward (core ← view) intact.
- Two real adapters must be kept behavior-equivalent; the `FakeInput` and shared
  port tests are the guard against them drifting.
