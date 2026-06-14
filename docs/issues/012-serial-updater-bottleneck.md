# Issue 012 — Serial single-consumer updater is the throughput ceiling

- **Severity:** 🟠 high
- **Area:** CPU / throughput
- **Status:** fixed (Phase 005)
- **Audit:** [001 / A3](../audits/001-traffic-and-decode-audit.md)
- **Phase:** [005](../phases/005-reduce-pipeline-contention.md)

## Location
`src/updater.rs` run loop; `src/rtps_watcher.rs:175` (serial `flat_map` decode)

## Problem
Decode is a single serial stream and the updater processes one event per mutex
acquisition — no batching. The `batch_updater.rs` scaffolding that would amortize the lock
is not compiled. This serial stage backs up the channel (→ Issue 011).

## Impact
Caps sustained event rate; under load the channel fills and events drop.

## Proposed direction
Batch-drain the channel (e.g. `recv` + `drain`) and apply N events under one lock; or move
to the lock-free state path. Measure before/after with the existing metrics.
