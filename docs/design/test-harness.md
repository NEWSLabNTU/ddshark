# Design — Test Harness (unit → E2E with real DDS traffic)

- **Status:** design / exploration
- **Goal:** a layered test suite for ddshark, from pure unit tests up to end-to-end runs
  driven by **real DDS traffic**, with the bulk of coverage fast, deterministic, and
  runnable in CI without elevated privileges.

## Constraints that shape the design

- ddshark is **passive**: it observes RTPS off an interface or a `.pcap`. The natural test
  injection points are therefore (a) raw bytes into the decoder, (b) a `.pcap` into the
  File source, (c) live packets on an interface.
- **Live capture needs `CAP_NET_RAW`** (root/caps) → unfriendly to default CI. So live
  traffic must be opt-in, and its *output* (a `.pcap`) should be reusable by privilege-free
  tests.
- **Real traffic is nondeterministic**: GUIDs and timestamps are random, packet counts and
  ordering vary. Tests over live/recorded traffic assert **invariants**, not exact equality.
- RustDDS is already a dependency and emits **real UDP** (`network/udp_sender.rs`), with a
  **synchronous** `DataWriter::write` (`dds/with_key/datawriter.rs:331`) — so we can generate
  real DDS traffic in-process from a plain `#[test]` with no async runtime.
- RustDDS can also **serialize RTPS messages to bytes** directly (`rtps/message.rs`
  `Message`/`MessageBuilder` + `write_to_vec_with_ctx`) — so we can synthesize exact packets
  with no network at all.

## The test pyramid

### L1 — Unit (pure, no I/O, always in CI)
Target the pure logic, fastest feedback. Candidates already pure:
- `utils/timed_stat.rs` — windowed rate math (mean/variance, window eviction, the reversed
  `Entry` ordering). Highest value: the rate stats are core output.
- `utils/{guid,guid_prefix,entity_id,entity_kind,locator}.rs` — display formatting, incl. the
  `UNKNOWN` prefix path (relevant to issue 003/007).
- `rtps/packet_decoder.rs` — IP reassembly (`process_fragments`): offset-unit math, dedup,
  overlap rejection, contiguity, TTL/cap eviction (issues 004/009). Currently private →
  test via an in-module `#[cfg(test)]` block.
- rep-id parsing in `rtps_watcher.rs::deserialize_payload` (issue 002): LE/BE/unknown.

These need no new infrastructure beyond `#[cfg(test)]` modules.

### L2 — Component / decode (deterministic, no network)
Drive the real decode + accounting logic with **synthesized RTPS bytes** (no sockets), so
results are byte-exact and reproducible.

- **Decoder path:** build an RTPS `Message` via RustDDS `MessageBuilder`, serialize to bytes,
  wrap in UDP/IPv4/Ethernet (etherparse can *write* headers), feed `PacketDecoder` → assert the
  returned `RtpsPacket`/`PacketKind`. Covers framing, IP defrag, the `Message::read_from_buffer`
  hardening (issue 001), malformed-input robustness (feed truncated/garbage → assert no panic).
- **Submessage → event:** the `handle_submsg_*` functions are pure free fns taking
  `(&Interpreter, &Submessage-part)` → assert the emitted `RtpsSubmsgEventKind` (attribution,
  UNKNOWN-prefix handling — issues 003/007).
- **Event → state (the updater):** feed crafted `UpdateEvent`s and assert `State`/`Statistics`
  (fragmented-message accounting — issue 005; packet-vs-submsg count — 006; gap accounting — 008;
  per-writer frag cap — 010). This is where the correctness fixes get regression cover.

Needs test seams (below): a bytes→decode entry, and a way to push events through the updater.

### L3 — Integration (pcap replay, deterministic, always in CI)
Commit a small set of **golden `.pcap` fixtures** (recorded once from real DDS, see Fixtures).
Run the **whole headless pipeline** (`PacketSource::File` → watcher → updater) and assert on the
final `State`: e.g. "participant P, topic `Square`, ≥1 writer, ≥1 reader, data count > 0,
writer↔topic association present". No root, no flakiness, fast.

Needs: a public **headless run** helper returning the final `State`, and **clean EOF
termination** (today the File stream chains `stream::pending()` and hangs — see seams).

### L4 — E2E (live real DDS, opt-in, gated)
Spin real DDS participants **in-process with RustDDS**: a publisher (and optionally subscriber)
on a domain/topic, writing N samples; run ddshark capturing the interface live; assert it
discovers the participant/topic/endpoints and counts data. Doubles as the **fixture recorder**
for L3. Nondeterministic → assert invariants. Requires `CAP_NET_RAW` and a multicast-capable
interface → gated (see CI).

## Traffic sources — tradeoffs

| Source | Determinism | Privilege | Realism | Use |
|---|---|---|---|---|
| Synthesized bytes (RustDDS `MessageBuilder`) | exact | none | wire-accurate, hand-built | L2 |
| Golden `.pcap` fixtures | exact (replay) | none | real, captured once | L3 |
| Live RustDDS in-process | invariant-only | `CAP_NET_RAW` | fully real | L4 + recorder |

Strategy: **record once, replay forever.** L4 (or a dev `xtask`) records real DDS traffic to a
`.pcap`; that fixture drives the privilege-free L3 tests. L4 itself runs as a gated smoke check
and to regenerate fixtures when the protocol handling changes.

## Required test seams (small, enabling refactors)

1. **Headless pipeline entry point.** Extract the wiring currently inline in `main.rs` into a
   reusable `fn run_pipeline(source, opts, cancel) -> Arc<Mutex<State>>` (or a `Pipeline` struct)
   so integration tests can run it and read the resulting `State`. Today `main()` drops the Arc
   and has no seam.
2. **Terminate File source on EOF.** Replace the unconditional `stream.chain(stream::pending())`
   (`rtps_watcher.rs:81`) with EOF-terminating behavior for File input — e.g. an
   `--exit-on-eof` flag (default on for `-f` in `--no-tui`). Fixes a real UX wart *and* lets
   replay tests end deterministically without an external cancel.
3. **Decode-from-bytes entry.** Add `PacketDecoder::decode_bytes(&[u8], ts)` (or accept an
   Ethernet frame) so L2 tests don't have to construct a `pcap::Packet` (awkward, crate-internal).
4. **Reachable updater logic.** Make `Updater::handle_message` / handlers `pub(crate)`, or have
   the `Pipeline` expose "feed these events," so L2 can assert state transitions without a socket.
5. **State inspection helpers.** Small read-only accessors / a snapshot type so assertions read
   cleanly (counts per topic/writer/reader, relationships).

All are minor and align with work already done (the pipeline is already split into
`rtps_watcher`/`updater` functions; only `main` glue and the EOF chain need touching).

## Fixtures

- Location: `tests/fixtures/*.pcap` + a short README per fixture (what generated it: domain,
  topic, type, sample count, DDS impl/version).
- Keep them **tiny** (a few KB: SPDP + SEDP + a handful of DATA/HEARTBEAT/ACKNACK).
- Provide a **recorder** (`xtask record-fixture` or a `--ignored` test): start a RustDDS
  publisher, capture the interface to a savefile (`pcap` crate supports writing), stop, trim.
- Cover variety over time: vanilla pub/sub, fragmented (large sample → DataFrag), multi-topic,
  and a hand-crafted **malformed/hostile** pcap for the robustness assertions.

## CI tiers

1. **Default job (no privileges):** L1 + L2 + L3. `just test`. Must stay green on every push.
2. **Privileged job (caps/root):** L4 live E2E. Either a dedicated runner with `CAP_NET_RAW`,
   or `sudo setcap cap_net_raw+ep` on the test binary, or run inside a **network namespace**
   (`ip netns`) with a `dummy`/`veth` interface and multicast enabled to isolate from host DDS
   traffic. Marked `#[ignore]` so it only runs when explicitly invoked.
3. Fixture regeneration is manual/periodic via the recorder, reviewed in PR (binary diff).

## Determinism & isolation notes

- **Domain isolation:** pick a random/unique domain id per test run so concurrent tests and any
  host DDS don't cross-talk; assert on that domain only.
- **Multicast on loopback:** SPDP multicast (`239.255.0.1`, hardcoded in RustDDS) may need `lo`
  multicast enabled, or run participants over a `dummy`/`veth` iface in a netns. `only_networks`
  is an unimplemented placeholder upstream — can't restrict to localhost via API today.
- **Invariant assertions for live/recorded:** topic names, type names, presence of ≥1
  writer/reader, monotonic counters, discovered relationships — never exact packet counts.

## Crates / deps

- Likely sufficient with std assertions + existing deps (`pcap` for savefile read/write,
  `rustdds` for traffic + message synthesis).
- Optional niceties: `insta` (snapshot tests for State summaries), `assert_matches`,
  `serial_test` (serialize the privileged/live tests so they don't share a domain/interface).

## Proposed layout

```
tests/
  unit_*.rs            # or #[cfg(test)] in-module for private fns (L1)
  decode_synth.rs      # L2 synthesized-bytes decode + updater state
  replay_pcap.rs       # L3 golden-pcap → headless pipeline → assert State
  e2e_live.rs          # L4 #[ignore] real RustDDS traffic + live capture
  common/
    mod.rs             # builders: RTPS message synth, headless run, DDS publisher, captures
  fixtures/
    square_basic.pcap
    fragmented.pcap
    malformed.pcap
```

## Risks / open questions

- Live capture flakiness (timing, discovery races) → keep L4 small, invariant-only, retried.
- Multicast/loopback setup is environment-sensitive → prefer netns; document the setup.
- `pcap::Packet` synthesis vs a `decode_bytes` seam — the seam is cleaner; confirm timestamp
  handling (`timeval`) for replayed/synth packets.
- Whether to also support a non-pcap raw-frame source to avoid the `pcap` dependency in L2/L3
  decode tests (decode-from-bytes seam removes the need).

## Incremental rollout

1. L1 unit tests + the `process_fragments`/`TimedStat` cases (no refactor needed).
2. Seams #2 (`--exit-on-eof`) and #1 (`run_pipeline`) → enables L3.
3. One golden fixture + first L3 replay test.
4. Seams #3/#4 → L2 synthesized-bytes + updater-state tests for the issue-fix regressions.
5. L4 live + recorder, gated; backfill fixtures.
