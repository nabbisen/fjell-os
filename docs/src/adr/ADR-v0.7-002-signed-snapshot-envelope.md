# ADR-v0.7-002 — Signed Snapshot Export and Import Verification

**Status:** Accepted  
**Date:** 2026-05-19 (v0.7.0, RFC v0.7-002)

## Context

Snapshots were previously local-only. Exporting them across nodes requires
a signed, content-addressed envelope that resists replay and substitution.

## Decision

`SnapshotEnvelope` (schema v1/v2) wraps an ordered list of `SnapshotRecord`
values. The `snapshot_digest` is computed over the magic prefix, schema
version, source identity digest, issued tick, nonce, and all record payloads.
The signature domain-separates the digest with `"FJELL-SNAPSHOT-SIGN-V1"` to
prevent cross-protocol replay against attestation records.

`SnapshotImportOutcome` captures the result of import (Accepted / Refused)
with six typed error codes.

## Consequences

- Import verification is deterministic and testable purely in host code.
- The nonce prevents replay of old envelopes.
- `snapshotd` handles export; `syncd` handles import.
