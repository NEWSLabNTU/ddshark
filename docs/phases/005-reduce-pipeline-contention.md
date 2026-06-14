# Phase 005 — Reduce pipeline contention & raise throughput

- **Status:** in progress (012/013/014 done; 015 deferred)
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
- [x] Batch-drain the channel (`try_recv` up to `MAX_UPDATE_BATCH = 256`) and apply the batch under one `State` lock; record `batch_processed`
- [ ] Measure events/sec before/after — pending load harness
### TUI contention (013)
- [x] Build each tab's table under a brief `with_state` lock, release, then draw — the ratatui draw no longer blocks the updater
### Metrics (014)
- [x] Sample latency 1-in-`LATENCY_SAMPLE_RATE` (64) so the percentile write lock is off the per-event hot path (vectors already bounded)
- [ ] Optional: switch to a lock-free atomic histogram — deferred
### Allocations (015)
- [ ] Cut the double `missing_sn` materialization + locator clones — **deferred**: requires an
      event move-semantics refactor through ~8 handlers (compiler-verifiable but wide) for a
      marginal, unmeasurable gain. `gap_list` clone feeds the deferred gap feature (008), keep it.
      Best done together with a load harness.

## Acceptance criteria
- [ ] Measured sustained throughput improves and induced-congestion drop rate falls vs Phase 004 baseline
- [ ] Updater no longer stalls for full TUI frames (writer-wait metric drops)
- [ ] Latency tracking memory is bounded
- [ ] `just check` + `just test` green

## Rollback
Each optimization is an independent commit; revert any without affecting the others.
