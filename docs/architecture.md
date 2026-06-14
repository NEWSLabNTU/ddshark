# ddshark Architecture

ddshark is a **passive** RTPS/DDS monitor. It sniffs RTPS packets off a network
interface (or a `.pcap`) and infers participant / topic / reader / writer state and
traffic statistics **without joining** any DDS domain (it never sends, never ACKs,
never participates in discovery). Everything is reconstructed from observed traffic.

## Process model

Three concurrent units wired in `src/main.rs`:

```
 capture thread(s)            updater task                 TUI (main thread)
 ┌───────────────┐  flume   ┌──────────────┐  Arc<Mutex>  ┌──────────────┐
 │ rtps_watcher  │ bounded  │   Updater    │   <State>    │     Tui      │
 │  decode +     ├────────► │ single       ├────────────► │ render @     │
 │  emit events  │  (1024)  │ consumer,    │   (writer)   │ refresh_rate │
 └───────────────┘          │ sole writer  │              │  (reader)    │
        ▲                    └──────────────┘              └──────────────┘
   pcap/iface                       │ also writes
                                    ▼
                            LockFreeStatistics (atomics)
```

- **Watcher** (`src/rtps_watcher.rs`, `src/rtps/`): reads packets from a `PacketSource`
  (`File` / `Interface` / `Default`), decodes Ethernet→IPv4→UDP→RTPS, splits each RTPS
  message into submessages, and emits one `UpdateEvent` per submessage into a single
  bounded `flume` channel (`--buffer-size`, default 1024).
- **Updater** (`src/updater.rs`): the *only* writer of `State`. Drains the channel one
  event at a time, takes `Arc<Mutex<State>>`, mutates, releases. Also bumps
  `LockFreeStatistics` atomics in parallel.
- **TUI** (`src/ui.rs`, `src/ui/*`): on each refresh tick takes the same mutex (read
  side) and renders all tabs.

Watcher + updater run on a Tokio multi-thread runtime spawned on a dedicated OS thread;
the TUI runs on the main thread. A `CancellationToken` + Ctrl-C handler tears all three
down together. `--no-tui` drops the channel sender so the pipeline drains and exits.

**Observability guarantee.** Being passive, ddshark cannot apply backpressure to the wire,
so under sustained overload the bounded channel drops events rather than blocking capture.
Reported statistics are therefore **exact as long as the "dropped events" gauge is 0**; a
non-zero gauge (red banner) means counts during that window are undercounts. Raise
`--buffer-size` to absorb larger bursts. Memory is bounded everywhere it could grow from
hostile/lossy traffic: IP and RTPS reassembly buffers (TTL + count caps), per-writer
in-flight fragments, and the abnormality log.

## Decode path (what ddshark calls in RustDDS)

ddshark does **not** reimplement RTPS — it reuses the vendored RustDDS submodule
(`rustdds = { path = "RustDDS" }`, currently 0.11.8 + the "make mods public" patch).

| Step | ddshark site | RustDDS API |
|---|---|---|
| Parse one RTPS message | `src/rtps/packet_decoder.rs` (`Message::read_from_buffer`) | `rustdds::rtps::Message` |
| Iterate submessages | `src/rtps_watcher.rs` `handle_submsg` (`flat_map`) | `Submessage`, `SubmessageBody`, `WriterSubmessage`/`ReaderSubmessage`/`InterpreterSubmessage` |
| Decode discovery payloads | `src/rtps_watcher.rs:559` | `PlCdrDeserializerAdapter::from_bytes` → `SpdpDiscoveredParticipantData`, `DiscoveredReader/Writer/TopicData` |
| Identifiers | throughout | `GUID`, `GuidPrefix`, `EntityId`, `Locator`, `SequenceNumber` |

Submessage handling: Writer (Data, DataFrag, Gap, Heartbeat, HeartbeatFrag),
Reader (AckNack, NackFrag), Interpreter (InfoSource, InfoDestination, InfoReply,
InfoTimestamp). Unknown submessage kinds are silently ignored by RustDDS (safe).
Security (`SEC_*`) submessages are not compiled in (no `security` feature) — fine for
passive sniffing.

**Two-layer fragmentation.** IP-level fragmentation is reassembled by ddshark itself
in `packet_decoder.rs` (`fragments`/`assemblers` maps). RTPS-level fragmentation
(DataFrag) is tracked separately in `WriterState.frag_messages` with a defrag buffer.
These are independent layers.

## State model

- `src/state.rs` — `State` (the live, authoritative model): `participants`, `topics`,
  `abnormalities`, `stat: Statistics`. Per-entity rate stats via `utils::TimedStat`
  (windowed). This is what the TUI reads.
- `src/message.rs` — `UpdateEvent` (watcher→updater wire format) + per-submessage event
  structs.
- `src/lockfree_state.rs`, `src/state_adapter.rs` — half-finished lock-free migration.
  `StateAdapter.use_lockfree` defaults `false`; only `LockFreeStatistics` counters are
  live (updated alongside the mutex `State`). `state.rs` remains authoritative.

## Inference logic (passive reconstruction)

- **Discovery (SEDP/SPDP):** discovery samples are PL-CDR-decoded to learn
  participant locators and writer/reader→topic associations.
- **Attribution:** destination-addressed submessages (AckNack, NackFrag, Gap) use the
  `InfoDestination` GUID prefix to identify the peer; absent it, the code falls back to
  the source prefix (see audit — this can misattribute).
- **Reader/writer relationships** are inferred from observed SEDP endpoint data plus
  matching topic names; there is no active matching.

## Orphaned / not-compiled files

`src/batch_updater.rs`, `src/ring_buffer.rs`, `src/state_cleanup.rs` exist on disk but
are **not declared as modules** in `main.rs` — they are not part of the build. Their
intended roles (batched lock acquisition, bounded overflow buffer, periodic state
eviction) are therefore **not active**. `src/otlp_metrics.rs` and `src/qos.rs` are
likewise commented out.

See [audits/001-traffic-and-decode-audit.md](audits/001-traffic-and-decode-audit.md)
for the correctness/performance review.
