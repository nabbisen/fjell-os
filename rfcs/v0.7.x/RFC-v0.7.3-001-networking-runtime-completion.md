# RFC-v0.7.3-001: v0.4 Networking Runtime Completion

## Status

Draft (closes review findings **W-RB-04, C-M-07**)

## Target Version

`v0.7.3`

## Summary

Convert the v0.4 secure-networking stack from "alpha stub with
simulated paths" to a runtime-complete control plane: real virtio-net
descriptor processing in both directions, real packet demux in
`netd`, real peer verification in `secure-transportd`, and removal of
all simulated TLS handshake paths from release builds.

## Motivation

Whole-project review §4 RB-04 documents the gap:

```text
fjell-driver-virtio-net:
  full virtio register reads and ring ops land in v0.4.0-alpha.2
  feature negotiation uses simulated minimal offered set
  RX ring polling logic lands later

fjell-netd:
  session table and cap-broker integration land fully in alpha.2
  PacketRx demux deferred

fjell-secure-transportd:
  certificate verification lands in alpha.2
  perform_tls_handshake simulates ServerHello/Certificate/CertVerify
  metadata fetch stubbed
```

A v0.8 fleet milestone built on simulated transport is unacceptable.

Also addressed: **C-M-07** (debug UART writes in `sys_ipc_send` path)
which the architect noted is a production hygiene issue closely
related to the network path.

## Goals

```text
- virtio-net performs real RX/TX descriptor processing with the
  feature set negotiated against the host.
- netd demultiplexes PacketRx into per-session queues.
- secure-transportd performs real peer certificate verification or
  fails closed.
- diagnosticsd and upgraded use secure-transportd through service
  IPC (not direct simulated paths).
- All "simulated" code paths are gated behind a development-only
  feature flag.
- Debug UART writes are removed from sys_ipc_send.
```

## Non-Goals

```text
- No new wire protocols (the v0.4 ABI is unchanged).
- No replacement of fjell-sxt-crypto (that is RFC-v0.7.3-002).
- No multi-interface routing; single virtio-net device only.
- No IPv6 support (deferred to v0.8).
```

## External Design

### virtio-net feature negotiation

Replace the simulated offered set with real host negotiation:

```rust
// crates/fjell-driver-virtio-net/src/features.rs

pub fn negotiate_features(host_offered: u64) -> Result<u64, FeatureError> {
    let want = DRIVER_ACCEPTED_FEATURES;
    let intersection = host_offered & want;

    // Require MAC and STATUS at minimum.
    let required = (1u64 << VIRTIO_NET_F_MAC) | (1u64 << VIRTIO_NET_F_STATUS);
    if intersection & required != required {
        return Err(FeatureError::MissingRequired);
    }

    Ok(intersection)
}
```

Real `VIRTIO_PCI_COMMON_CFG.device_feature` reads replace the
hard-coded constant.  Real `driver_feature` writes commit the
negotiated set.

### RX ring polling

Real descriptor consumption from the RX ring:

```rust
pub struct RxRing { ... }

impl RxRing {
    /// Pop one packet from the ring. Returns None if ring empty.
    pub fn poll(&mut self) -> Option<RxPacket> {
        if self.used.idx == self.used.last_seen { return None; }
        let used_idx = self.used.last_seen as usize % RING_SIZE;
        let chain   = self.used.ring[used_idx];
        let len     = chain.len as usize;
        let desc_id = chain.id as usize;
        let buf     = &self.buffers[desc_id][..len];
        self.used.last_seen = self.used.last_seen.wrapping_add(1);
        Some(RxPacket { buf, descriptor: desc_id })
    }

    /// Return the descriptor to the ring for refill.
    pub fn release(&mut self, descriptor: usize) { ... }
}
```

Capability check at every `poll`: the caller must hold `NetDevice`
with `NET_RECV` right.

### netd packet demux

```rust
// crates/fjell-netd/src/demux.rs

impl Netd {
    pub fn ingest_packet(&mut self, p: RxPacket) -> Result<(), DemuxError> {
        let session_id = self.lookup_session(p.dst_mac, p.dst_port)?;
        self.sessions[session_id].queue.push(p)?;
        Ok(())
    }
}
```

Sessions are created via cap-broker policy.  `SessionId` is
capability-handle-shaped; the holder of a session cap can read its
queue.

### secure-transportd certificate verification

Replace the simulated handshake with real verification against the
keyring's `ReleaseVerification` anchors:

```rust
pub fn verify_peer_certificate(
    cert: &PeerCertificate,
    keyring: &Keyring,
) -> Result<VerifiedPeer, TlsError> {
    let anchor = keyring.find_anchor(KeyPurpose::ReleaseVerification)
        .ok_or(TlsError::NoTrustAnchor)?;
    let sig = cert.signature();
    anchor.verify(cert.tbs_bytes(), sig)
        .map_err(|_| TlsError::CertificateVerificationFailed)?;
    Ok(VerifiedPeer { ... })
}
```

`#[cfg(feature = "simulated-transport")]` gates the old simulated
path; default builds reject any attempt to use it.

### Debug UART removal from IPC path

`sys_ipc_send` no longer writes to UART.  Diagnostic emission goes
through:

1. The audit ring (kernel-side audit-format).
2. The semantic-stream intent path (user-side).

Both are existing mechanisms; the change is removing the direct UART
write.

## Data Model

### `RxPacket`

```rust
pub struct RxPacket<'a> {
    pub buf:        &'a [u8],
    pub descriptor: usize,    // ring slot to release after consumption
    pub src_mac:    [u8; 6],
    pub dst_mac:    [u8; 6],
    pub ethertype:  u16,
}
```

### `FeatureError`, `DemuxError`, `TlsError`

Typed error enums; details in the source.

### `simulated-transport` feature

```toml
# fjell-secure-transportd/Cargo.toml
[features]
default = []
# WARNING: Enables simulated handshake; for development only.
# Release builds MUST NOT enable this feature.
simulated-transport = []
```

CI gate: `cargo check --no-default-features --features ""` builds.
A second CI gate runs `cargo check --features simulated-transport`
to ensure the dev path still compiles.  A release verification gate
ensures no published binary has the feature enabled.

## Internal Design

### virtio-net RX/TX descriptor processing

Per the virtio 1.2 spec:

```text
- driver populates available ring with empty buffer descriptors
- device consumes available, writes received packets, posts used
- driver consumes used, processes packet, refills available
```

The current code path simulates this entire flow.  The replacement
implements it against the real MMIO region (which is now cap-gated
per RFC-v0.7.4-003).

### Capability gating

```text
NetDevice cap (RFC-v0.7.4-003 narrows the grant):
  - NET_RECV right required for RxRing::poll
  - NET_SEND right required for TxRing::push
  - NET_FEATURES_READ for device_features read
  - NET_FEATURES_WRITE for driver_features write
```

### Removal of simulated paths in release builds

CI gate (added by RFC-v0.7.1-002 §schema-gate plus a new gate):

```bash
ci-no-simulated-transport:
  ! grep -r "simulated-transport" target/release/
  cargo metadata --format-version=1 \
    | jq '.. | select(.features?) | .features' \
    | grep -v "simulated-transport"
```

## Security Design

### Removed attack surface

- Simulated TLS handshake removed in release builds.  An attacker
  cannot exploit the simulated path because it does not exist in the
  compiled binary.
- Real certificate verification means a malicious update server with
  no valid signature fails closed.

### Added attack surface

- Real virtio-net descriptor processing means real DMA usage.  This
  is safe ONLY if RFC-v0.7.4-001 (DMA lifetime safety) has landed.
  This RFC takes a hard dependency on RFC-v0.7.4-001.

### Audit events

```text
AUDIT_TLS_HANDSHAKE_STARTED         = 0x0301
AUDIT_TLS_HANDSHAKE_COMPLETED       = 0x0302
AUDIT_TLS_HANDSHAKE_FAILED          = 0x0303
AUDIT_TLS_PEER_CERT_REJECTED        = 0x0304
AUDIT_NET_FEATURE_NEGOTIATION_FAIL  = 0x0305
AUDIT_NETD_DEMUX_REJECTED           = 0x0306
```

## Memory / Resource Design

- RX ring buffers: 16 descriptors × 240 B (existing) = ~4 KiB.
- TX ring buffers: same.
- Session queue: 8 entries × packet size; per-session.
- secure-transportd: existing transcript buffer; no growth.

## Compatibility and Migration

- Existing v0.4 prebuilt service binaries are replaced.
- The simulated-transport feature must be enabled explicitly for
  development.  Anyone running a dev build today implicitly uses the
  simulated path; they must add the feature flag.
- IPC ABI unchanged.

## Test Strategy

```text
- virtio-net feature negotiation rejects host with no MAC support.
- RxRing::poll returns None on empty ring.
- RxRing::poll returns a packet then None when one packet is
  consumed.
- netd demux routes packet to correct session.
- secure-transportd rejects a certificate with no matching anchor.
- secure-transportd accepts a certificate signed by a registered
  anchor.
- ci-no-simulated-transport gate fails the build if the feature is
  accidentally enabled.

QEMU smoke v0.4-net:
  - virtio-net comes up
  - link-up intent emitted
  - secure-transportd does a real handshake with a test peer
  - TEST:V0.4-NET:PASS marker emitted
```

## Acceptance Criteria

```text
- TEST:V0.4-NET:PASS appears in QEMU smoke.
- No simulated TLS handshake in release builds.
- No direct UART writes in sys_ipc_send.
- ADR-v0.7.3-001 filed.
- This RFC depends on RFC-v0.7.4-001 (DMA safety); blocked until
  v0.7.4-001 is accepted.
```

## Documentation Requirements

```text
- docs/src/reference/net-runtime.md describes the real RX/TX flow.
- docs/src/reference/secure-transport.md updated to require a real
  cert chain.
- docs/src/reference/cargo-features.md describes simulated-transport
  development-only feature.
```

## Open Questions

```text
1. Where do we keep the test peer certificate for QEMU smoke? Proposal:
   crates/fjell-tools/testdata/peer-cert-v1.bin generated by a host
   tool that signs with a dev anchor.

2. Should netd implement RX backpressure? Proposal: yes — bounded
   per-session queues; overflow drops oldest with audit.

3. Should secure-transportd buffer the full handshake transcript?
   Proposal: yes — already does; document the bound.
```

## Release Gate

```text
- TEST:V0.4-NET:PASS in QEMU smoke
- ci-no-simulated-transport job green
- ADR-v0.7.3-001 accepted
- RFC-v0.7.4-001 accepted (hard dependency on DMA safety)
```
