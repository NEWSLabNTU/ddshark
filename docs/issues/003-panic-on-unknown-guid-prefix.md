# Issue 003 — assert_ne! panics on GuidPrefix::UNKNOWN

- **Severity:** 🟡 medium
- **Area:** DDS decode / robustness
- **Status:** fixed (Phase 002)
- **Audit:** [001 / C3](../audits/001-traffic-and-decode-audit.md)
- **Phase:** [002](../phases/002-harden-decode-path.md)

## Location
`src/rtps_watcher.rs:146`, `src/rtps_watcher.rs:225`

## Problem
Both sites `assert_ne!(guid_prefix, GuidPrefix::UNKNOWN)`. A packet with an all-zero GUID
prefix trips the assert and crashes the process. RTPS permits UNKNOWN in places, and
line 242 already handles it gracefully — the asserts are inconsistent.

## Impact
DoS via crash on a crafted/malformed packet.

## Proposed direction
Replace the asserts with a graceful skip (drop the event, optionally count it) matching
the line-242 handling.
