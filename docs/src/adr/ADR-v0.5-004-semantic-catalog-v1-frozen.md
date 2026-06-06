# ADR-v0.5-004 — Semantic Catalog v1 Is Frozen

**Status:** Accepted  
**Date:** 2026-05-19 (v0.5.0, RFC v0.5-004)

## Context

The semantic intent stream had grown organically across v0.2–v0.4 with no
versioning contract.  Consumers (proxy-text, diagnosticsd) could not rely on
stable tag values or field counts.

## Decision

`fjell-semantic-v1` ships a frozen `CATALOG_V1` const table.  Tags cannot be
reused; new entries may only be appended in `0.5.x` patch releases to unallocated
sub-ranges.  A `v2` catalog will be a separate file when needed.

The encoder/decoder use a `"FJSI-V1"` magic prefix and a `0xFF` sentinel; unknown
trailing bytes are rejected.  Round-trip tests are generated for every catalog entry.

Version negotiation uses `CatalogVersion { major, minor }` at stream handshake.

## Consequences

- `proxy-text` and `diagnosticsd` can parse any v1 envelope without branching on
  version at every receive site.
- Adding an intent requires a documented RFC, not an ad-hoc code change.
- Readers must handle `UnknownTag` gracefully for forward compatibility.
