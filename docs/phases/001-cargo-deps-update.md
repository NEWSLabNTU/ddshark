# Phase 001 — Cargo Dependency Update

- **Status:** in progress
- **Started:** 2026-06-14
- **Goal:** Move every direct dependency to its current stable release, absorbing
  API changes, while keeping `just check` and `just test` green.

## Background

Caret-ranged deps (anyhow, clap, tokio, serde, chrono, bytes, …) already resolve to
their latest compatible release through `Cargo.lock`. The work here is the deps pinned
to an **old major**, whose requirement strings must be bumped and whose API breaks must
be fixed in code.

## Version targets

| Crate | Req now | Target | Risk | API shift |
|---|---|---|---|---|
| itertools | 0.11 | 0.14.0 | low | minor renames/deprecations |
| flume | 0.10.14 | 0.12.0 | low | minor |
| dashmap | 5.5 | 6.2.1 | low | minor (`Ref` API stable) |
| pcap | 1.1 | 2.4.0 | low | error/builder tweaks |
| gethostname | 0.4.3 | 1.0+ | low | return type |
| crossterm | 0.26.1 | 0.29.0 | med | must match ratatui re-export |
| etherparse | 0.13 | 0.20.1 | **high** | `PacketHeaders`/`Ipv4Header`/`TransportHeader` reworked into slice/header modules |
| ratatui | 0.22 | 0.30.1 | **high** | `Frame` no longer generic over `Backend`; `frame.size()` → `frame.area()`; widget/style changes |
| opentelemetry + opentelemetry-otlp + opentelemetry_sdk + semantic-conventions | 0.19 / 0.12 / 0.19 / 0.11 | 0.32.x | **high** | full builder/exporter rewrite; `opentelemetry_api` crate removed (folded into `opentelemetry`) |

## Work items

### Tier 1 — low-risk bumps
- [x] Bump `itertools` to 0.14; fix any deprecated combinators (no code change)
- [x] Bump `flume` to 0.12 (no code change)
- [x] Bump `dashmap` to 6.2 (no code change)
- [x] Bump `pcap` to 2.4 (no code change; `capture-stream` feature intact)
- [x] Bump `gethostname` to 1.1 (no code change)
- [x] `cargo build` + `just check` green after Tier 1

### Tier 2 — TUI stack (crossterm + ratatui together)
- [x] Bump `ratatui` to 0.30 and `crossterm` to 0.29 (ratatui's re-export)
- [x] `Frame<B>` → `Frame` across `src/ui.rs`; concretized `run_loop` to `CrosstermBackend<io::Stdout>`
- [x] `frame.size()` → `frame.area()`
- [x] Fix widget API: `Table::new(rows, widths)`, `highlight_style`→`row_highlight_style`, disambiguate `StatefulWidget::render`
- [x] `just check` green
- [ ] Manual TUI smoke run (`-f <pcap>`) — pending a sample capture

### Tier 3 — packet parsing (etherparse)
- [x] Bump `etherparse` to 0.20
- [x] Port `src/rtps/packet_decoder.rs`: `PacketHeaders{net,..}`, `NetHeaders::Ipv4`, `fragment_offset.value()`, `PayloadSlice::slice()`
- [x] Trim `RtpsPacketHeaders` to read fields only (dropped never-read `link`/`vlan`/`pcap_header`)
- [ ] Verify decode correctness against a known `.pcap` (counts match pre-bump) — pending sample capture

### Tier 4 — OpenTelemetry stack
- [x] Bump `opentelemetry`, `opentelemetry-otlp`, `opentelemetry_sdk`,
      `opentelemetry-semantic-conventions` to 0.32.x
- [x] Drop the obsolete `opentelemetry_api` dep; re-point imports at `opentelemetry`
- [x] Rewrite `src/otlp.rs`: `SpanExporter::builder().with_tonic()`, `SdkTracerProvider::builder().with_batch_exporter()`, `Resource::builder()`, `provider.shutdown()` on Drop
- [ ] Re-enable `src/otlp_metrics.rs` — deferred; needs its own metrics-API port (still commented in `main.rs`). `opentelemetry-semantic-conventions` is currently an unused dep kept for that revival.

## Acceptance criteria
- [x] No direct dependency more than one minor behind its latest stable
- [x] `just check` passes (fmt + clippy `-D warnings`)
- [x] `just test` passes
- [x] `cargo build --release` succeeds
- [ ] TUI renders and offline `.pcap` decode produces the same entity/packet counts as before — pending a sample capture
- [x] Version requirements pinned in `Cargo.toml` (`Cargo.lock` is gitignored in this repo)

## Rollback
Each tier is an isolated commit. Revert the tier's commit and `cargo update -p <crate> --precise <old>` to restore.
