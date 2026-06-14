# Phase 005 — Reduce pipeline contention & raise throughput

- **Status:** planned
- **Goal:** raise sustained event throughput and cut lock contention so the pipeline
  keeps up under load (reduces the drops Phase 004 makes visible).
- **Issues:** [012](../issues/012-serial-updater-bottleneck.md),
  [013](../issues/013-tui-holds-state-lock-full-frame.md),
  [014](../issues/014-metrics-latency-rwlock-hot-path.md),
  [015](../issues/015-per-event-allocations.md)
- **Depends on:** Phase 004 (drop visibility) for before/after measurement.

## Background
The updater is a serial single-consumer with one lock per event (012); the TUI holds the
State lock for whole frames (013); the latency tracker adds an RwLock<Vec> write per event
(014); hot paths allocate per event (015).

## Work items
### Throughput (012)
- [ ] Batch-drain the channel and apply N events under one `State` lock (or adopt the lock-free state path)
- [ ] Measure events/sec before/after via existing metrics
### TUI contention (013)
- [ ] Snapshot minimal row data under the lock, release, then render from the snapshot
### Metrics (014)
- [ ] Replace unbounded `RwLock<Vec>` latency samples with a bounded estimator (ring/atomic histogram)
### Allocations (015)
- [ ] Cut the double `missing_sn` materialization and locator clones; reuse scratch buffers

## Acceptance criteria
- [ ] Measured sustained throughput improves and induced-congestion drop rate falls vs Phase 004 baseline
- [ ] Updater no longer stalls for full TUI frames (writer-wait metric drops)
- [ ] Latency tracking memory is bounded
- [ ] `just check` + `just test` green

## Rollback
Each optimization is an independent commit; revert any without affecting the others.
