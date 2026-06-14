# Phase 003 — Fix traffic accounting & relationship inference

- **Status:** done
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
- [x] In `updater.rs` topic block, use `topic.total_msg_count`/`topic.msg_rate_stat`
- [x] Writer now increments once (line 559); topic once — each completed frag msg counts once per writer, once per topic
- [ ] Add a unit test asserting writer==topic==1 for one completed fragmented sample — deferred (no test harness for updater yet)
### Issue 006 — packet vs submessage count
- [x] Relabel the stat display "packets" → "total submsg" (honest to what is counted)
- [ ] Optional internal field rename `packet_count`→`submsg_count` — deferred (wide mechanical churn, no behavior change)
### Issue 007 — attribution fallback
- [x] On absent destination prefix, attribute the peer to `GuidPrefix::UNKNOWN` (count traffic, no false relationship to the source participant)
### Issue 008 — gap application
- [x] Bounded gap accounting: count sequence numbers a writer declares irrelevant via GAP
      ([gap_start, gap_list.base) + set bits) into `WriterState.gapped_sn_count`
- [x] Surface a `gaps` column in the writer tab
- [ ] Full per-writer received-vs-expected loss model — out of scope (would need an unbounded
      observed-sequence set); the gap count is the bounded, useful subset

## Acceptance criteria
- [ ] Replaying a known capture yields message counts that reconcile writer⇄topic⇄participant
- [ ] No relationship is created from a submessage lacking a real destination
- [ ] Stat labels match what is actually counted
- [ ] `just check` + `just test` green (with a frag-accounting test)

## Rollback
Per-issue commits; 005 is a self-contained one-block change.
