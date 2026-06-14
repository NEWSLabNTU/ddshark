# Issue 005 — Fragmented-message completion mis-accounts writer & topic stats

- **Severity:** 🔴 critical (data correctness)
- **Area:** statistics
- **Status:** open
- **Audit:** [001 / B1](../audits/001-traffic-and-decode-audit.md)
- **Phase:** [003](../phases/003-fix-traffic-accounting.md)

## Location
`src/updater.rs:559-560` and `src/updater.rs:572-573` (DataFrag completion block)

## Problem
On DataFrag completion:
- `writer.total_msg_count` is incremented twice (line 559 and again 572),
- the topic block (569-579) increments `writer.total_msg_count` / `writer.msg_rate_stat`
  instead of `topic.total_msg_count` / `topic.msg_rate_stat`.

Correct non-fragmented path for contrast: `updater.rs:403-404`.

## Impact
Writer message count over-counts by 1 per fragmented sample; topic message count and
msg-rate **never** count fragmented samples (only topic bytes/bit-rate are updated).

## Proposed direction
In the topic block, replace the two `writer.*` lines with `topic.total_msg_count += 1`
and `topic.msg_rate_stat.push(...)`. Remove the duplicate writer increment so each
completed message counts once on writer and once on topic.
