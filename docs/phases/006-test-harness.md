# Phase 006 — Test harness (unit → E2E, nextest-driven)

- **Status:** in progress (runner + L1 done)
- **Goal:** build the layered test suite from [docs/design/test-harness.md](../design/test-harness.md),
  run by **cargo-nextest**. Most coverage fast, deterministic, and CI-runnable without
  privileges; live real-DDS traffic behind a gated profile.
- **Design:** [test-harness](../design/test-harness.md) (pyramid, seams, fixtures, nextest config).

## Background
No tests exist today (`qos.rs` disabled). The pipeline splits cleanly
(`rtps_watcher`/`updater`), but `main()` holds the wiring and drops the `State`, and the File
source hangs after EOF (`rtps_watcher.rs:81` chains `stream::pending()`). A few small seams
unlock the higher tiers. Runner is nextest so we get test-groups (serialize the live tier),
`default-filter` (exclude live by default), and per-test retries/timeouts.

## Work items

### Runner setup
- [x] Add `.config/nextest.toml` with `test-groups.e2e`, `profile.default`
      (`default-filter = 'not test(~e2e_)'`), the `test(~e2e_)` override
      (retries/slow-timeout/e2e group), `profile.ci` (junit), `profile.e2e` (`all()`).
      Note: filter uses `test(~e2e_)` not `binary(e2e_live)` — nextest validates `binary()`
      against existing binaries, which don't exist until L4 is written.
- [x] Point `just test` at `cargo nextest run`; add `just test-e2e` → `--profile e2e`
- [ ] Add `cargo-nextest` install note to the build docs (`cargo binstall cargo-nextest`)

### Seams (enabling refactors)
- [ ] **Add a `[lib]` target** (or split lib+bin): ddshark is a bin-only crate today, so
      `tests/` integration binaries can't `use ddshark::…`. L1 works as in-module `#[cfg(test)]`,
      but L2/L3 need a lib to import `run_pipeline`, `PacketDecoder`, `State`, etc.
- [ ] Extract a headless `run_pipeline(source, opts, cancel) -> Arc<Mutex<State>>` from `main()`
- [ ] Terminate the File source on EOF: `--exit-on-eof` (default on for `-f` + `--no-tui`),
      replacing the unconditional `stream::pending()` chain
- [ ] `PacketDecoder::decode_bytes(&[u8], ts)` so tests skip `pcap::Packet` construction
- [ ] Make updater logic reachable: `pub(crate)` handlers or a feed-events method on the pipeline
- [ ] Small read-only State snapshot/accessors for clean assertions

### L1 — unit (no refactor) — done, in-module `#[cfg(test)]`
- [x] `utils/timed_stat.rs`: window eviction, mean-over-window rate
- [x] `utils.rs`: displays incl. UNKNOWN prefix/GUID, builtin entity ids, locator, entity kind
- [x] `rtps/packet_decoder.rs::process_fragments`: contiguous reassembly, dedup, gap-never-completes
- [x] `rtps_watcher.rs`: rep-id LE/BE/unknown/short (extracted `representation_id_from_payload`)

### L2 — component (synthesized bytes, no network)
- [ ] Test helper: build an RTPS `Message` (RustDDS `MessageBuilder`) → bytes → wrap UDP/IP/Eth
- [ ] Decoder tests: well-formed decode; truncated/garbage → no panic (issue 001)
- [ ] `handle_submsg_*` → event assertions (attribution + UNKNOWN prefix, issues 003/007)
- [ ] `UpdateEvent` → `State` regressions for the fixes: frag accounting (005), submsg-count
      label (006), gap count (008), per-writer frag cap (010)

### L3 — integration (golden pcap replay)
- [ ] Record one small `square_basic.pcap` fixture (via L4 recorder) + a hand-made `malformed.pcap`
- [ ] `tests/replay_pcap.rs`: File source → `run_pipeline` → assert State invariants
      (topic present, ≥1 writer/reader, data count > 0, writer↔topic association)

### L4 — E2E live (gated)
- [ ] `tests/common`: in-process RustDDS publisher (sync `write`) on a unique domain/topic
- [ ] `tests/e2e_live.rs`: publish N samples, capture live, assert discovery + counts (invariants)
- [ ] Fixture recorder (`xtask` or a live test) capturing to `.pcap`
- [ ] Optional nextest setup-script: netns/`veth` + `DDSHARK_TEST_IFACE` via `$NEXTEST_ENV`

### CI
- [ ] Default job: `cargo nextest run --profile ci` (L1-L3, no caps)
- [ ] Privileged job: `cargo nextest run --profile e2e` on a `CAP_NET_RAW` runner / netns

## Acceptance criteria
- [ ] `cargo nextest run` (default profile) runs L1-L3 green with no elevated privileges
- [ ] Each of the 15 closed audit issues has at least one regression test (L1/L2/L3)
- [ ] `-f <file.pcap> --no-tui` exits on its own at EOF
- [ ] Live tier passes under the `e2e` profile on a privileged runner, serialized by the `e2e` group
- [ ] `just test` (nextest) wired; `just check` still green

## Rollout order
1. Runner setup + L1 (no refactor) — immediate value.
2. Seams `--exit-on-eof` + `run_pipeline` → L3 with one fixture.
3. `decode_bytes` + updater seam → L2 regressions for the issue fixes.
4. L4 live + recorder; backfill fixtures.

## Rollback
Config + tests are additive; seams are isolated commits (revert individually). The
`--exit-on-eof` default change is the only behavior change to the shipped binary.
