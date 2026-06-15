---
status: accepted
date: 2026-06-15
tags: [core, buffer, event-sourcing, undo-redo]
related: [docs/plans/2026-06-15-vellum-design.md, docs/plans/2026-06-15-vellum-increment-0.md]
---

# ADR-0002: Event-sourced buffer model

## Context

Vellum inherits the `kineticrs` house worldview: **an edit is an event** (design
§3). The document is an aggregate (rope buffer + version); each keystroke is a
domain event (`CharInserted`, `RangeDeleted`, `SelectionMoved`). Undo/redo,
time-travel, and future collaboration (CRDT/OT) are all natural extensions of one
model rather than separate ad-hoc machinery.

The alternative — an imperative buffer with a bolted-on undo stack — is simpler to
start but diverges from the backend architecture, makes undo/redo a special case,
and forecloses collaboration without a rewrite.

## Decision

Model edits as **events**. The buffer is an aggregate whose state is the result of
applying an ordered sequence of edit events. **Undo/redo is reverse/replay of
events**, not a separate stack: undo applies the inverse of the last event, redo
re-applies it. Each event carries enough information to be inverted.

**Snapshotting is deferred.** Until a measured need appears (very long histories,
large documents), the buffer replays from the event log without periodic
snapshots. The event shape is designed so snapshots can be added later as a pure
optimization without changing event semantics.

**Increment scope:** Increment 0 deliberately ships a simple `String`-backed
buffer with char-boundary-safe insert/delete and **no** event log yet — just
enough to prove the core → WASM → view pipeline. The event-sourced **rope**
buffer (events, inversion, undo/redo) arrives in **Increment 1**. This ADR records
the target model so the Increment 0 buffer is understood as a temporary stand-in,
not the design.

## Consequences

- Undo/redo, time-travel, and eventual CRDT/OT collaboration share one model; each
  is an extension, not a rewrite.
- The editor "breathes" the same event-sourced architecture as the Nexum backend,
  keeping the house signature coherent.
- Inversion requires every event to retain the data needed to undo it (e.g. the
  deleted text on a `RangeDeleted`), a modest memory cost accepted for correctness.
- Deferring snapshots keeps Increment 1 small; the cost is unbounded replay until
  snapshotting lands, acceptable for the document sizes Vellum targets.
- The Increment 0 `String` buffer must be replaced, not extended — it is a
  scaffold for the pipeline, recorded as such here.
