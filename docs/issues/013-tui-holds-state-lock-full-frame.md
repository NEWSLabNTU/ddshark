# Issue 013 — TUI holds State mutex for the whole frame render

- **Severity:** 🟡 medium
- **Area:** contention
- **Status:** fixed (Phase 005)
- **Audit:** [001 / A5](../audits/001-traffic-and-decode-audit.md)
- **Phase:** [005](../phases/005-reduce-pipeline-contention.md)

## Location
`src/ui.rs:230-` (`render` takes `state.lock()`, then builds/renders all tabs)

## Problem
The render path locks `State` and iterates every participant/topic/writer/reader while
holding it. The sole writer (updater) blocks for the full render each tick; cost scales
with entity count.

## Impact
Couples UI rendering cost to ingest latency → periodic writer stalls.

## Proposed direction
Snapshot the minimal data needed under the lock (clone/collect rows), release, then render
from the snapshot. Or move reads to the lock-free state.
