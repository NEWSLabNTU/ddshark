# Issue 015 — Per-event allocations on hot paths

- **Severity:** 🟡 medium
- **Area:** CPU / memory pressure
- **Status:** open (deferred — needs event move-semantics refactor + load measurement)
- **Audit:** [001 / A7](../audits/001-traffic-and-decode-audit.md)
- **Phase:** [005](../phases/005-reduce-pipeline-contention.md)

## Location
`src/rtps_watcher.rs` (AckNack `missing_sn` collect; Gap `gap_list.clone()`; locator-list
clones); `src/updater.rs:729` (`to_vec()`)

## Problem
Each AckNack/Gap/discovery event allocates one or more `Vec`s, cloned again between
watcher event and updater. Bounded per event but constant pressure under load.

## Impact
Sustained allocator pressure at high event rates; compounds Issue 012.

## Proposed direction
Reduce copies: pass slices/`Arc` where ownership allows, reuse scratch buffers, avoid the
double `missing_sn` materialization (watcher event + updater). Low priority until 012/011
are addressed.
