# Issue 007 — Source-prefix fallback fabricates reader/writer attribution

- **Severity:** 🟡 medium
- **Area:** statistics / relationship inference
- **Status:** fixed (Phase 003)
- **Audit:** [001 / B3](../audits/001-traffic-and-decode-audit.md)
- **Phase:** [003](../phases/003-fix-traffic-accounting.md)

## Location
`src/rtps_watcher.rs:414-419` (Gap), `442-448` (NackFrag), `525-531` (AckNack)

## Problem
When `InfoDestination` is absent, the writer GUID is rebuilt from the **source** prefix.
AckNack/NackFrag are reader→writer; the source is the reader, so the inferred writer GUID
is wrong. Inconsistent with line 242 which guards UNKNOWN.

## Impact
Phantom reader/writer relationships for peers that omit InfoDestination or for truncated
captures — directly undermines the tool's core relationship inference.

## Proposed direction
When destination prefix is unavailable, do not invent a writer GUID — skip relationship
attribution for that submessage (still count raw traffic), or mark it unattributed.
