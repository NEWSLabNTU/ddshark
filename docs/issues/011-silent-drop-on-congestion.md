# Issue 011 — Events silently dropped on channel congestion

- **Severity:** 🟠 high
- **Area:** IO / observability correctness
- **Status:** fixed (drop visibility, Phase 004); backpressure still open
- **Audit:** [001 / A4](../audits/001-traffic-and-decode-audit.md)
- **Phase:** [004](../phases/004-bound-memory-and-backpressure.md)

## Location
`src/rtps_watcher.rs:67,109-119` (`SEND_TIMEOUT = 100ms`)

## Problem
When the bounded channel (default 1024) fills, each event waits up to 100 ms then is
dropped (`warn!("congestion occurs")`, `message_dropped()`). No backpressure to capture,
no retry. Drops happen precisely at peak traffic.

## Impact
Statistics become silently wrong under load — the worst time for a monitor to lose data.
Drop count is in metrics but not surfaced in the TUI stats view.

## Proposed direction
At minimum surface dropped-event count + a congestion indicator in the TUI. Consider a
larger/configurable buffer, a drop-rate gauge, and documenting the sampling guarantee
("counts are exact unless the drop gauge is non-zero").
