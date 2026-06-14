# Issue 006 — `packet_count` counts submessages, displayed as packets

- **Severity:** 🟠 high (misleading metric)
- **Area:** statistics
- **Status:** open
- **Audit:** [001 / B2](../audits/001-traffic-and-decode-audit.md)
- **Phase:** [003](../phases/003-fix-traffic-accounting.md)

## Location
`src/updater.rs:354,421,587,621,689,754,771`; `UpdateEvent::RtpsMsg` handler `todo!()` at
`src/updater.rs:166`; surfaced as "packets" in `src/ui/tab_stat.rs`.

## Problem
`stat.packet_count += 1` runs in every submessage handler, so one RTPS packet with N
submessages adds N. True packet count is never tracked — the `RtpsMsg` variant that would
carry it is never constructed and its handler is `todo!()`.

## Impact
The TUI "packets" figure is actually a submessage count; real packet rate is unknown.

## Proposed direction
Either (a) rename the field/label to `submsg_count` to match reality, or (b) emit and
handle a per-packet `RtpsMsg` event and count packets there. Prefer (b) if packet-level
rate matters; (a) is the cheap honest fix.
