# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What this is

`ddshark` is a live RTPS-protocol monitoring tool (a "DDS sniffer"). It captures RTPS
packets from a network interface or a `.pcap` file, decodes them, tracks per-participant /
per-topic / per-reader / per-writer state, and renders it in a terminal UI (ratatui). It is
independent of any DDS implementation and was tested against Cyclone DDS.

## Build / run / test

```sh
# Build (needs submodule). Clone with --recurse-submodules; binary -> target/release/ddshark
cargo build --release

cargo clippy --all-targets          # lint (CI-style, treat warnings as work)
cargo fmt                           # rustfmt.toml: imports_granularity=Crate, doc-comment formatting

# Live capture (needs CAP_NET_RAW; run with sudo or set caps on the binary)
sudo ./target/release/ddshark -i eno1
# Offline from a dump
./target/release/ddshark -f packets.pcap
# Headless (no TUI), prints tracing logs to stderr
./target/release/ddshark -f packets.pcap --no-tui
```

Tests: there is currently almost no test coverage. The only `#[test]`s live in `src/qos.rs`,
which is **not compiled** (`mod qos` is commented out in `main.rs`). Run a single test with
`cargo test <name>` only after re-enabling its module.

## Architecture

The program is a 3-thread pipeline wired together in `main.rs`:

1. **Capture/decode** — `rtps_watcher` (`src/rtps_watcher.rs`) pulls from a `PacketSource`
   (`src/rtps/`, variants `File` / `Interface` / `Default`), decodes Ethernet→IPv4→UDP→RTPS
   submessages, and emits `UpdateEvent`s.
2. **Updater** — `src/updater.rs` consumes those events and mutates the singleton state. This
   is the only writer of `State`.
3. **TUI** — `src/ui.rs` reads the state on a refresh tick and draws tabs.

Communication is a single bounded `flume` channel (`--buffer-size`, default 1024). The watcher
and updater run on a Tokio multi-thread runtime spawned on a dedicated OS thread; the TUI runs
on the main thread. A `CancellationToken` plus a Ctrl-C handler coordinates shutdown across all
three. When `--no-tui` is set, `main` drops the channel sender so the pipeline drains and exits.

### State (the central data model)
- `src/state.rs` — `State` is the legacy single source of truth: `Arc<Mutex<State>>` shared
  between updater (writer) and TUI (reader). Holds `participants`, `topics`, `abnormalities`,
  and `Statistics`. Per-entity stats use `utils::TimedStat` for rate-over-window tracking.
- `src/message.rs` — `UpdateEvent` enum is the wire format between watcher and updater
  (`RtpsMsg`, `RtpsSubmsg`, `ParticipantInfo`, `Tick`, `ToggleLogging`), plus the per-submessage
  event structs (`DataEvent`, `HeartbeatEvent`, `AckNackEvent`, `GapEvent`, frag variants, …).

### In-progress lock-free migration (read before touching state code)
There is a half-finished migration away from the global mutex. `src/lockfree_state.rs`
(`LockFreeState`, `LockFreeStatistics`) uses `DashMap` / `ArcSwap` / atomics, and
`src/state_adapter.rs` (`StateAdapter`) is a bridge that holds both old and new state with a
`use_lockfree` flag — **currently defaults to legacy (`false`)**. The updater already updates
`LockFreeStatistics` for packet counters in parallel with the mutex `State`. Treat
`state.rs` as authoritative for UI-visible data; `lockfree_state.rs`/`state_adapter.rs` are not
yet the primary path. Don't assume one is canonical without checking the call site.

### Metrics & observability
- `src/metrics.rs` — `MetricsCollector` (cloneable `Arc` of atomics + latency tracker), shared
  across all three stages. Pipeline-health counters (packets received/parsed, queue depth,
  drops, latencies), distinct from the DDS-domain `Statistics` in `state.rs`.
- `src/metrics_logger.rs` — optional CSV dump of `MetricsCollector` (`--metrics-log`,
  `--metrics-log-file`).
- `src/otlp.rs` — OpenTelemetry trace export (`--otlp`, `--otlp-endpoint`). **`otlp_metrics.rs`
  is disabled** (commented out in `main.rs`) due to an OTLP API version incompatibility; the
  OTLP-metrics task in `main.rs` is stubbed to a no-op. Don't wire it back in without resolving
  the `opentelemetry` 0.19 API mismatch.

### TUI
`src/ui.rs` is the controller; each tab is a `*Table` + `*TableState` pair in `src/ui/`
(`tab_participant`, `tab_topic`, `tab_writer`, `tab_reader`, `tab_stat`, `tab_abnormality`).
`xtable.rs` is the shared sortable-table widget. Keys: `q` quit / `h` help, `s` toggle sort,
`v` toggle show, `r` toggle logging, arrows/PageUp/Down/Home/End navigate, Tab/BackTab switch
tabs. `Focus` enum gates `q` (Dashboard vs Help).

### RTPS decoding depends on the vendored RustDDS
`RustDDS/` is a git submodule consumed as a path dependency (`rustdds = { path = "RustDDS" }`).
ddshark reuses RustDDS's wire types (GUID, EntityId, Locator, SequenceNumber, submessage
structs, discovery `Discovered*Data`, PL-CDR deserializers) rather than reimplementing RTPS.
If decode logic needs a type that doesn't exist, check RustDDS before adding it here.

### Utils worth knowing
`src/utils/` — extension traits and newtypes for RTPS identifiers (`EntityIdExt`, `GuidExt`,
`GuidPrefixExt`, `LocatorExt`, `EntityKind`) and `TimedStat` (windowed rate stats). Reach for
these instead of formatting GUIDs/locators by hand.

## Conventions
- `config::TICK_INTERVAL` (100 ms) drives the updater's internal tick; `--refresh-rate` (Hz)
  drives the TUI redraw. They are separate clocks — don't conflate them.
- Errors flow up via `anyhow::Result`; the `spawn` helper in `main.rs` cancels the token on a
  task join failure so the whole pipeline tears down together.
- The watcher is the single producer and the updater the single consumer of the `flume`
  channel; keep that invariant if adding stages.
