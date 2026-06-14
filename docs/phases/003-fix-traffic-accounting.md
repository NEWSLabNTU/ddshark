# Phase 003 — Fix traffic accounting & relationship inference

- **Status:** planned
- **Goal:** make the statistics the tool reports actually correct and the inferred
  reader/writer relationships sound.
- **Issues:** [005](../issues/005-fragmented-message-stat-miscount.md),
  [006](../issues/006-packet-count-is-submessage-count.md),
  [007](../issues/007-destination-fallback-misattribution.md),
  [008](../issues/008-gap-logic-disabled.md)

## Background
A copy-paste bug mis-counts fragmented messages (005), the headline "packets" figure is
really a submessage count (006), missing-destination submessages fabricate writer GUIDs
(007), and GAP application is disabled (008).

## Work items
### Issue 005 — fragmented stat miscount (highest value, smallest fix)
- [ ] In `updater.rs:572-573` topic block, use `topic.total_msg_count`/`topic.msg_rate_stat`
- [ ] Remove the duplicate writer increment so a completed frag msg counts once per writer, once per topic
- [ ] Add/extend a unit test asserting writer==topic==1 for one completed fragmented sample
### Issue 006 — packet vs submessage count
- [ ] Decide: relabel field to `submsg_count` (cheap) OR emit+handle per-packet `RtpsMsg`
- [ ] Update `tab_stat.rs` labels accordingly
### Issue 007 — attribution fallback
- [ ] On absent destination prefix, skip relationship attribution (count raw traffic only)
### Issue 008 — gap application
- [ ] Implement gap range handling against the writer sequence model

## Acceptance criteria
- [ ] Replaying a known capture yields message counts that reconcile writer⇄topic⇄participant
- [ ] No relationship is created from a submessage lacking a real destination
- [ ] Stat labels match what is actually counted
- [ ] `just check` + `just test` green (with a frag-accounting test)

## Rollback
Per-issue commits; 005 is a self-contained one-block change.
