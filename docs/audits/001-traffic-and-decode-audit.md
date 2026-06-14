# Audit 001 — Traffic Monitor: Performance, Statistics, Decode

- **Date:** 2026-06-14
- **Scope:** the live data path of the passive RTPS sniffer — capture → decode →
  account → render. Three concern areas: (A) IO/CPU bottlenecks & contention,
  (B) statistical correctness, (C) DDS encode/decode robustness.
- **Method:** code read of ddshark + the RustDDS decode path it calls. Each finding
  below was verified against source (file:line). Severities reflect impact on a passive
  monitor whose value *is* accurate, complete observation under load.

Legend: 🔴 critical · 🟠 high · 🟡 medium · ⚪ note

---

## A. Performance, IO/CPU, contention

### A1 🔴 IP-defrag buffers never evicted — unbounded memory
`src/rtps/packet_decoder.rs:16-20, 81, 103-110`
`fragments`/`assemblers` maps only delete an entry when reassembly *completes*. Any
incomplete IP datagram (dropped/lost/hostile fragment, crashed peer) leaks forever —
keyed by (src, dst, ip-id), so a spoofed or churning id space grows without limit.
No timeout, no cap. The would-be cleanup lives in `state_cleanup.rs`, which is **not
compiled** (not a module in `main.rs`).

### A2 🟠 RTPS-defrag buffers also unbounded
`src/updater.rs:541-555`, `src/state.rs` `WriterState.frag_messages`
`frag_messages` entries are removed only on `defrag_buf.is_full()` completion. Never-
completed DataFrag sequences accumulate per writer. Same class as A1 at the RTPS layer.

### A3 🟠 Serial single-consumer updater is the throughput ceiling
`src/updater.rs` run loop; `src/rtps_watcher.rs:175` (`flat_map` serial decode)
Decode is a single serial stream and the updater processes one event per mutex
acquisition. There is no batching (the `batch_updater.rs` scaffolding is not compiled).
Under high submessage rates this is the bottleneck; see A4 for what happens when it
falls behind.

### A4 🟠 Channel-full ⇒ silent drop after 100 ms
`src/rtps_watcher.rs:67,109-119` (`SEND_TIMEOUT = 100ms`)
When the bounded channel (1024) fills, each event waits up to 100 ms then is **dropped**
(`warn!("congestion occurs")`, `message_dropped()`), no backpressure to capture, no
retry. For a monitor, dropped events = silently wrong statistics during exactly the peak
traffic you most want to observe. The drop is counted in metrics (good) but invisible in
the TUI stats.

### A5 🟡 TUI holds the State mutex for the whole frame
`src/ui.rs:230-` (`render` takes `state.lock()` then renders every tab)
The render path locks `State` and builds all tables (iterating every participant/topic/
writer/reader) while holding it. The sole writer (updater) blocks for the full render.
The longer the entity tables, the longer the writer stalls each tick — couples UI cost to
ingest latency. (Magnitude depends on entity count; not benchmarked here.)

### A6 🟡 Latency tracker is an `RwLock<Vec>` written on the hot path
`src/metrics.rs` (`record_processing_latency`, `lock_acquired` → `latencies.write()`)
Per-event writes into an `RwLock`-guarded `Vec`, read by the TUI. Adds a second
contended lock to every processed event and the vectors grow per sample. Other pipeline
counters are lock-free atomics (good); this one is not.

### A7 🟡 Per-event allocations on hot paths
`src/rtps_watcher.rs` (AckNack `missing_sn` collect; Gap `gap_list.clone()`; locator
list clones) and `src/updater.rs:729` (`to_vec()`).
Each AckNack/Gap/discovery event allocates one or more `Vec`s. Scales linearly with
traffic; bounded individually but constant pressure under load.

---

## B. Statistical correctness

### B1 🔴 Fragmented-message completion mis-accounts writer & topic counts
`src/updater.rs:559-560` then `572-573`
On DataFrag completion the writer message count is incremented **twice** (lines 559 and
again 572), and the *topic* block at 569-579 wrongly increments `writer.total_msg_count`
/ `writer.msg_rate_stat` instead of `topic.total_msg_count` / `topic.msg_rate_stat`.
Net effect: **writer msg count over-counts by 1 per fragmented sample; topic msg count
& msg-rate never count fragmented samples at all** (only topic bytes/bit-rate are
updated). Contrast the correct non-fragmented path at `updater.rs:403-404`. Clear
copy-paste bug.

### B2 🟠 `packet_count` actually counts submessages, not packets
`src/updater.rs:354,421,587,621,689,754,771`; surfaced as "packets" in the TUI
`stat.packet_count` is `+1` in every submessage handler, so one RTPS packet carrying N
submessages adds N (+1 more for the ParticipantInfo event). True packet count is never
tracked — the `UpdateEvent::RtpsMsg` variant that would carry it is never constructed and
its handler is `todo!()` (`updater.rs:166`). Displayed rate is a submessage rate
mislabeled as packets.

### B3 🟡 Destination fallback can fabricate reader/writer attribution
`src/rtps_watcher.rs:414-419 (Gap), 442-448 (NackFrag), 525-531 (AckNack)`
When `InfoDestination` is absent, the writer GUID is reconstructed from the **source**
prefix. AckNack/NackFrag are reader→writer; the source is the reader, so the inferred
writer GUID is wrong → potential phantom reader/writer relationships for peers that omit
InfoDestination or for truncated captures. Note line 242 elsewhere *does* guard
`UNKNOWN` — handling is inconsistent.

### B4 🟡 GAP is counted but not applied
`src/updater.rs:585-612` (body commented out)
GAP submessages bump counters but the gap/missing-sequence logic is disabled, so
writer-declared missing samples are not reflected in state. Counts ok; semantics
incomplete.

### B5 ⚪ Heartbeat `last_sn` fix verified correct
`src/updater.rs:~689` (commit `4fd123e`)
The earlier `first_sn`/`last_sn` mix-up is fixed; current init uses `last_sn`. No action.

---

## C. DDS encode/decode robustness (untrusted input)

ddshark feeds **raw sniffed bytes** straight into `Message::read_from_buffer`
(`packet_decoder.rs`). Robustness of the decoder against hostile/truncated packets is a
security property here, not just correctness.

### C1 🔴 Missing alloc-bound check in parameter-list decode (RustDDS #404 not in 0.11.8)
`RustDDS/src/messages/submessages/elements/parameter_list.rs` (`read_vec(length)`)
A malformed parameter length (e.g. `0xFFFFFFFF`) drives an allocation sized by attacker-
controlled input before validating bytes remain → OOM/DoS on a single crafted packet.
The upstream fix (`4fc80689`, GH #404) lands in 0.11.9-WIP and is **absent** from the
pinned stable 0.11.8. Reachable via SPDP/SEDP discovery and inline-QoS on the sniffed
path. (Tracked previously in the RustDDS rebase advisory.)

### C2 🟠 Discovery payloads decoded with hardcoded `PL_CDR_LE`
`src/rtps_watcher.rs:559`
The representation identifier is hardcoded little-endian instead of read from the
SerializedPayload header (first 2 bytes). Big-endian (`PL_CDR_BE`) discovery data is
silently mis-decoded → corrupted participant/endpoint info rather than an error. Rare on
x86 peers, but a correctness/interop gap and silent.

### C3 🟡 `assert_ne!` panics on `GuidPrefix::UNKNOWN`
`src/rtps_watcher.rs:146, 225`
A packet with an all-zero GUID prefix trips a debug/release `assert_ne!` → process
crash (DoS). The protocol permits UNKNOWN in places; line 242 handles it gracefully,
so the asserts are inconsistent and crash-prone on hostile input.

### C4 🟡 IP-defrag has no duplicate/overlap handling
`src/rtps/packet_decoder.rs:81-110`
`received_length` accumulates per fragment with no de-dup; a retransmitted/overlapping
fragment double-counts toward the completion total, so reassembly can complete on wrong
byte totals or mis-order. Last-write-wins per offset (BTreeMap) with no contiguity check.

### C5 ⚪ Submessage framing bounds-checked; unknown kinds ignored — OK
`RustDDS/src/rtps/submessage.rs` validates declared submessage length against the buffer
and skips unknown kinds. Endianness is taken from submessage flags correctly. No issue.

---

## Priority shortlist

1. **C1** alloc-bound on untrusted decode (DoS) — cherry-pick RustDDS #404 onto the fork patch.
2. **B1** fragmented-message stat bug (wrong writer/topic counts) — local one-block fix.
3. **A1/A2** unbounded defrag buffers — add TTL/cap eviction (wire up `state_cleanup`).
4. **A4** silent drop under congestion — at minimum surface drop count in the TUI; consider backpressure.
5. **B2** packet vs submessage count — relabel or implement true packet counting.
6. **C2/C3** rep-id handling + UNKNOWN-prefix asserts — read rep-id; replace asserts with graceful skip.

## Not changed
Audit only — no code modified. Items above are candidates for follow-up phase docs.
