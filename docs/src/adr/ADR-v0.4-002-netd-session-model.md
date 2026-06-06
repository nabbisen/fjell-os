# ADR-v0.4-002 — netd: Capability-brokered Session Model for Network Access

**Status:** Accepted  
**Date:** 2026-05-19 (v0.4.0, RFC v0.4-002)

---

## Context

Multiple services (secure-transportd, future fleet management) need network
access, but direct `NetDevice` capability sharing would create uncontrolled
access patterns and make revocation difficult.

## Decision

`netd` acts as the single broker between the virtio-net driver and all
higher-level network consumers.  It models network access through `NetSession`
capabilities, each scoped to a `ChannelKind` (UpdateMetadata, Diagnostics,
Attestation, FleetEnroll).

Constants: `MAX_SESSIONS=8`, `MAX_CHANNELS=4`, `MAX_SXT_CHANNELS=4`,
`NET_RING_DESCRIPTORS=16`, `NET_DESCRIPTOR_PAYLOAD=240` bytes.

cap-broker enforces: only `netd` (ImageId 21) may acquire a brokered
`NetDevice` capability with `RIGHT_RECV | RIGHT_INSPECT | RIGHT_MINT`.
netd may then mint `Session` endpoint caps for `secure-transportd`.

## Consequences

- Session-level revocation is possible without touching the driver.
- The 19 host unit tests in `fjell-net-format` verify session/channel/ring
  state transitions without MMIO or IPC.
- Future services acquire sessions via cap-broker, never the raw device.
- `MAX_SESSIONS=8` is sufficient for v0.4; fleet scenarios (v0.7+) will
  require an increased limit and a more sophisticated allocation policy.
