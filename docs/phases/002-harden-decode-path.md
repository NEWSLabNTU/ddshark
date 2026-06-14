# Phase 002 — Harden the decode path against hostile/malformed packets

- **Status:** planned
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
- [ ] In `RustDDS/` fork, cherry-pick `4fc80689` onto the `ddshark` patch branch (per CLAUDE.md fork workflow)
- [ ] `cargo build` + smoke-decode a sample capture
- [ ] Force-push fork branch; bump submodule pointer in superproject
### Issue 002 — representation id
- [ ] Read rep-id from serialized-payload header at `rtps_watcher.rs:559`
- [ ] Skip (don't guess) on unknown/unsupported rep-id; count as decode-skip
### Issue 003 — UNKNOWN prefix
- [ ] Replace `assert_ne!` at `rtps_watcher.rs:146,225` with graceful skip
### Issue 004 — IP reassembly
- [ ] Reject duplicate/overlapping fragments; complete only on gap-free contiguous range
- [ ] Cap per-reassembly size

## Acceptance criteria
- [ ] A fuzz/replay corpus of malformed RTPS packets causes no panic and no unbounded alloc
- [ ] Big-endian discovery sample decodes correctly (or is cleanly skipped)
- [ ] `just check` + `just test` green
- [ ] Decode-skip / malformed counters visible in metrics

## Rollback
Each issue is an isolated commit; submodule bump (001) revert restores prior RustDDS pin.
