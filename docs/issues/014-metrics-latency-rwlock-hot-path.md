# Issue 014 — Latency tracker is an RwLock<Vec> written per event

- **Severity:** 🟡 medium
- **Area:** contention / memory
- **Status:** fixed (Phase 005)
- **Audit:** [001 / A6](../audits/001-traffic-and-decode-audit.md)
- **Phase:** [005](../phases/005-reduce-pipeline-contention.md)

## Location
`src/metrics.rs` (`record_processing_latency`, `lock_acquired` → `latencies.write()`;
TUI reads via `.read()`)

## Problem
Every processed event takes a write lock on an `RwLock`-guarded `Vec` (also growing per
sample), read by the TUI. A second contended lock on the hot path; other pipeline counters
are lock-free atomics by contrast.

## Impact
Writer/TUI lock contention; unbounded latency-sample vectors.

## Proposed direction
Use a bounded/streaming latency estimator (reservoir or fixed-size ring, or an HDR/atomic
histogram) instead of an unbounded `Vec` under a lock.
