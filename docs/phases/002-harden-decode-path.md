# Phase 002 — Harden the decode path against hostile/malformed packets

- **Status:** in progress
- **Goal:** ddshark must not crash, OOM, or silently mis-decode on untrusted sniffed
  bytes. Covers the decode-robustness issues.
- **Issues:** [001](../issues/001-unbounded-alloc-in-parameter-list-decode.md),
  [002](../issues/002-hardcoded-pl-cdr-le-representation-id.md),
  [003](../issues/003-panic-on-unknown-guid-prefix.md),
  [004](../issues/004-ip-defrag-no-dedup-overlap.md)

## Background
`packet_decoder.rs` feeds raw wire bytes into `Message::read_from_buffer`; discovery
payloads go through PL-CDR. Today a single crafted packet can OOM (001), big-endian
discovery mis-decodes silently (002), an all-zero GUID prefix panics (003), and IP
reassembly trusts fragment offsets (004).

## Work items
### Issue 001 — alloc bound (RustDDS #404)
- [x] In `RustDDS/` fork, cherry-pick `4fc80689` onto the `ddshark` patch branch → `a091aae2`
- [x] `cargo build` green
- [x] Push fork branch (fast-forward `9ccdc5a7..a091aae2`); bump submodule pointer in superproject
### Issue 002 — representation id
- [x] Read rep-id from serialized-payload encapsulation header in `deserialize_payload`
- [x] Skip on unknown/unsupported rep-id (logged)
### Issue 003 — UNKNOWN prefix
- [x] Replace `assert_ne!` at `rtps_watcher.rs:146,225` with graceful skip
### Issue 004 — IP reassembly
- [x] Reject duplicate/overlapping fragments; complete only on gap-free contiguous range; fix 8-octet offset units
- [ ] Cap per-reassembly size — deferred to Phase 004 (memory bounds)

## Acceptance criteria
- [ ] A fuzz/replay corpus of malformed RTPS packets causes no panic and no unbounded alloc — pending a corpus
- [ ] Big-endian discovery sample decodes correctly (or is cleanly skipped) — pending a BE sample
- [x] `just check` + `just test` green
- [ ] Decode-skip / malformed counters visible in metrics — deferred (overlaps Phase 004 drop visibility)

## Rollback
Each issue is an isolated commit; submodule bump (001) revert restores prior RustDDS pin.
