# Phase 004 — Bound memory and stop silent drops

- **Status:** in progress (drop visibility done; eviction pending)
- **Goal:** the monitor must survive long runs and hostile traffic without unbounded
  memory, and must never lose data silently.
- **Issues:** [009](../issues/009-ip-defrag-buffer-unbounded.md),
  [010](../issues/010-rtps-fragmessages-unbounded.md),
  [011](../issues/011-silent-drop-on-congestion.md)

## Background
IP-defrag (009) and RTPS-defrag (010) buffers only free on completion → leak on partial
traffic. Cleanup scaffolding (`state_cleanup.rs`) is not even compiled. On congestion the
watcher drops events after a 100 ms timeout without surfacing it (011).

## Work items
### Eviction (009, 010)
- [x] Add first-seen timestamps to IP reassembly entries; evict stale partials past `REASSEMBLY_TTL` (30s)
- [x] Cap concurrent IP reassemblies (`MAX_REASSEMBLIES = 4096`, oldest evicted)
- [x] Bound per-writer `frag_messages` (`MAX_FRAG_MESSAGES_PER_WRITER = 1024`, oldest incomplete evicted)
- [x] Eviction done inline in `PacketDecoder`/updater rather than reviving the orphan `state_cleanup.rs`
- [ ] Also bound the append-only `abnormalities` Vec — still open
### Drop visibility (011)
- [x] Surface dropped-event count + queue depth in the Statistics tab
- [x] Persistent red "⚠ DROPPING N events" banner in the tab row (visible on every tab)
- [ ] Make `--buffer-size` guidance explicit; document the "exact unless drop gauge > 0" guarantee — deferred (docs)

## Acceptance criteria
- [ ] Soak test with continuous partial/incomplete fragments shows bounded RSS
- [ ] Drop counter is visible and non-silent under induced congestion
- [ ] `state_cleanup` runs on the live path
- [ ] `just check` + `just test` green

## Rollback
Eviction and UI changes are separable commits; revert individually.
