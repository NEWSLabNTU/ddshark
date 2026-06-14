# Issue 010 — RTPS DataFrag buffers (frag_messages) unbounded

- **Severity:** 🟠 high
- **Area:** memory
- **Status:** fixed (Phase 004)
- **Audit:** [001 / A2](../audits/001-traffic-and-decode-audit.md)
- **Phase:** [004](../phases/004-bound-memory-and-backpressure.md)

## Location
`src/updater.rs:541-555`; `WriterState.frag_messages` in `src/state.rs`

## Problem
`frag_messages` entries are removed only on `defrag_buf.is_full()` completion. DataFrag
sequences that never complete accumulate per writer indefinitely (RTPS-layer twin of
Issue 009).

## Impact
Per-writer unbounded growth on lossy/partial traffic.

## Proposed direction
Bound per-writer in-flight fragment sets; evict oldest/incomplete by age or count when a
threshold is exceeded. Tie into the same cleanup sweep as Issue 009.
