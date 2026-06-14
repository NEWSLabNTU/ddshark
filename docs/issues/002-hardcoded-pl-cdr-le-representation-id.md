# Issue 002 — Discovery decoded with hardcoded PL_CDR_LE

- **Severity:** 🟠 high
- **Area:** DDS decode / correctness
- **Status:** open
- **Audit:** [001 / C2](../audits/001-traffic-and-decode-audit.md)
- **Phase:** [002](../phases/002-harden-decode-path.md)

## Location
`src/rtps_watcher.rs:559`
```rust
let result = PlCdrDeserializerAdapter::from_bytes(payload, RepresentationIdentifier::PL_CDR_LE);
```

## Problem
The representation identifier is hardcoded little-endian instead of read from the
SerializedPayload header (first 2 bytes). Big-endian (`PL_CDR_BE`) discovery payloads are
silently decoded as LE → corrupted participant/endpoint data, not an error.

## Impact
Silent mis-decode of discovery from big-endian peers; wrong inferred topology. Rare on
x86 but an interop/correctness gap with no signal.

## Proposed direction
Parse the rep-id from bytes 0–1 of the serialized payload and pass it through; on an
unknown/unsupported rep-id, skip the sample (don't guess).
