# ADR-v0.7-001 — Node Identity and Snapshot Exchange Trust Model

**Status:** Accepted  
**Date:** 2026-05-19 (v0.7.0, RFC v0.7-001)

## Context

v0.6 proved the kernel invariants but all nodes were assumed to be identical.
v0.7 introduces multi-node snapshot sync; nodes must authenticate each other.

## Decision

Each node has a `NodeIdentity` (16-byte ID, 32-byte alias, Ed25519 pubkey,
platform/board digests) whose canonical SHA-256 `identity_digest` is stored
in the append-only log (`STORE_RECORD_KIND_IDENTITY = 0x0020`).

A `NodeIdentityPolicy` governs which remote nodes are accepted as snapshot
sources: `SameFamily` (same trust_profile_tag), `Fleet` (roster-pinned), or
`Open`. The default is `SameFamily`.

`identityd` manages the identity lifecycle; `attestd` signs it.

## Consequences

- Snapshot imports are rejected unless the source passes the local policy.
- The identity digest chains into the measurement record, making node
  substitution detectable.
- Fleet mode (roster-pinned) is reserved for v0.8 — the enum variant is
  defined but pinned_roster is not yet validated.
