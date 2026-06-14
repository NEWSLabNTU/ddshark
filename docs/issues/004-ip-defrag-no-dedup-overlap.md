# Issue 004 — IP reassembly has no duplicate/overlap handling

- **Severity:** 🟡 medium
- **Area:** decode / correctness
- **Status:** open
- **Audit:** [001 / C4](../audits/001-traffic-and-decode-audit.md)
- **Phase:** [002](../phases/002-harden-decode-path.md)

## Location
`src/rtps/packet_decoder.rs:81-110` (`process_fragments`)

## Problem
`received_length` accumulates per fragment with no de-duplication; a retransmitted or
overlapping fragment double-counts toward the completion total. Fragments are stored
last-write-wins per offset (BTreeMap) with no contiguity validation.

## Impact
Reassembly can complete on wrong byte totals or mis-ordered data → corrupted RTPS bytes
handed to the decoder (which then feeds Issue 001's path).

## Proposed direction
Track expected total length from the last fragment (more_fragments=0) and validate each
fragment covers a fresh, contiguous, in-range offset window; reject duplicates/overlaps;
complete only when the covered range is gap-free.
