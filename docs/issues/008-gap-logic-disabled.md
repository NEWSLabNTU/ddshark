# Issue 008 — GAP counted but missing-sequence logic disabled

- **Severity:** 🟡 medium (incomplete feature)
- **Area:** statistics
- **Status:** fixed (Phase 003)
- **Audit:** [001 / B4](../audits/001-traffic-and-decode-audit.md)
- **Phase:** [003](../phases/003-fix-traffic-accounting.md)

## Location
`src/updater.rs:585-612` (`handle_gap_event`; gap body commented out)

## Problem
GAP submessages bump counters but the gap/missing-sequence handling is commented out, so
writer-declared irrelevant/missing samples are never reflected in writer/reader state.

## Impact
Gap-based missing-message visibility is absent; counts are fine, semantics incomplete.

## Proposed direction
Implement gap application: mark the gap_start..gap_list range as not-expected on the
writer's sequence tracking so it doesn't read as loss. Depends on sequence-tracking model.
