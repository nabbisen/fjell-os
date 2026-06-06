# ADR-v0.7-004 — Conflict Domain Metadata and Snapshot v2

**Status:** Accepted  
**Date:** 2026-05-19 (v0.7.0, RFC v0.7-004)  
**BREAKING-SCHEMA:** fjell-snapshot-format::SnapshotEnvelope

## Context

When two nodes exchange snapshots, some records may be authoritative on one
side but contested on the other. v1 envelopes had no mechanism to convey
this.

## Decision

Snapshot schema version bumps to **v2**. Each record gains a leading
`domain u8` field (`ConflictDomain`: LocallyConfirmed=0x01,
ForeignAuthoritative=0x02, Pending=0x03, Contested=0x04).

The `snapshot_digest` formula includes the domain byte when `schema_version
>= 2`. v2 readers MUST accept v1 envelopes by defaulting the absent domain
to `ForeignAuthoritative`.

The `BREAKING-SCHEMA` constraint requires a schema version bump in the
frozen schema file, this ADR, and a matching changelog entry.

## Consequences

- `syncd` can mark locally-confirmed records before propagating.
- v1 readers on older nodes safely receive v2 snapshots (defaulting domain).
- The breaking change is isolated to `fjell-snapshot-format`; no service ABI
  changes are required.
