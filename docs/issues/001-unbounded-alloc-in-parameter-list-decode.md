# Issue 001 — Unbounded allocation in parameter-list decode (DoS)

- **Severity:** 🔴 critical
- **Area:** DDS decode / security (untrusted input)
- **Status:** fixed (Phase 002)
- **Audit:** [001 / C1](../audits/001-traffic-and-decode-audit.md)
- **Phase:** [002](../phases/002-harden-decode-path.md)

## Location
`RustDDS/src/messages/submessages/elements/parameter_list.rs` (`read_vec(length)`),
reached from ddshark via `src/rtps/packet_decoder.rs` `Message::read_from_buffer` and
`src/rtps_watcher.rs:559` PL-CDR discovery decode.

## Problem
Parameter length is attacker-controlled and used to size an allocation before validating
that many bytes remain in the buffer. A single crafted SPDP/SEDP/inline-QoS packet with
length `0xFFFFFFFF` drives a multi-GB allocation. ddshark feeds raw sniffed bytes
straight into the decoder, so this is reachable from the wire.

## Impact
OOM / DoS on one malformed packet. The monitor is exactly the component sitting on
hostile traffic.

## Proposed direction
Cherry-pick upstream RustDDS fix `4fc80689` (GH #404, adds `minimum_bytes_needed`
bound checks) onto the fork's `ddshark` patch branch. Fix is in 0.11.9-WIP, **absent**
from pinned 0.11.8. Follow the fork rebase workflow in CLAUDE.md.
