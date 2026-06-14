# Issue 009 — IP-defrag buffers never evicted (unbounded memory)

- **Severity:** 🔴 critical
- **Area:** memory / IO
- **Status:** open
- **Audit:** [001 / A1](../audits/001-traffic-and-decode-audit.md)
- **Phase:** [004](../phases/004-bound-memory-and-backpressure.md)

## Location
`src/rtps/packet_decoder.rs:16-20, 81, 103-110`

## Problem
`fragments` / `assemblers` maps delete an entry only when reassembly completes. Any
incomplete IP datagram (lost/dropped/hostile fragment, crashed peer) leaks forever, keyed
by (src, dst, ip-id). No timeout, no cap. The intended cleanup in `state_cleanup.rs` is
not compiled (not a module in `main.rs`).

## Impact
Unbounded memory growth; a churning/spoofed ip-id space is an amplified leak / DoS.

## Proposed direction
Evict stale partial reassemblies by age (per-entry first-seen timestamp + periodic sweep)
and cap total buffered fragment bytes. Wire up a real `state_cleanup` path.
