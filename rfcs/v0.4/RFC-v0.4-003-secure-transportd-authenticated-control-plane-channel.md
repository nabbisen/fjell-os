# RFC-v0.4-003: secure-transportd Authenticated Control-Plane Channel

## Status

Draft (revised, supersedes pack v0.4-003 draft)

## Target Version

`v0.4.0`.

## Phase

Minimal Secure Control-Plane Networking — Epic C (Authenticated Channel).

## Related Work

- v0.3 RFC 001 (`HardwareTrustProvider`); v0.3 RFC 002 (Keyring).
- v0.4 RFC 002 (`netd` Session cap); v0.4 RFC 004 (update metadata fetch);
  v0.4 RFC 005 (remote attestation transport).
- v0.7 RFC 001 (node identity; consumed here as the local endpoint identity).

---

## 1. Summary

Introduce `secure-transportd`: a user-space service that converts a raw
`netd` `Session` into an **authenticated control-plane channel** suitable for:

- fetching update metadata (RFC v0.4-004);
- pushing remote diagnostics and remote attestation records (RFC v0.4-005);
- enrolling into a fleet identity (RFC v0.7-001, v0.8-001).

The channel is **server-authenticated TLS 1.3** with a hard-pinned trust
anchor set drawn from the keyring (`KeyPurpose::ReleaseVerification` reused
as the transport-trust purpose in v0.4.0 — see §14.2). It does **not** carry
arbitrary user data — only the four enumerated control-plane protocols are
allowed.

---

## 2. Motivation

Adding any network reachability without a per-byte trust check would expose
Fjell's update and attestation paths to MITM. Putting TLS in user space:

- keeps the kernel mechanism-only;
- restricts the cryptographic surface to one service;
- allows cap-broker to enforce "any Fjell service that wants to talk to the
  outside must go through secure-transportd."

Server-only authentication (no client-cert mTLS yet) is sufficient because:

1. Fjell's local attestation record already binds the device identity; the
   payload identifies the device.
2. Bidirectional certs require a per-device PKI which is v0.7's scope.

---

## 3. Goals

```text
- Implement TLS 1.3 client-side handshake using a single curated cipher
  suite (TLS_AES_128_GCM_SHA256) and a single key-exchange group (X25519).
- Trust-anchor selection via the keyring (RFC v0.3-002); rotation honoured.
- Strict server-name pinning per channel scope (e.g.
  "update.fjell.example").
- Channel exposes typed APIs:
    - update_metadata_fetch(url) -> bytes
    - diagnostics_push(record) -> ack
    - attestation_push(record) -> challenge_response
    - fleet_enroll_step(state) -> next_state
- Each typed API has its own ChannelCap with explicit rights.
- All bytes-on-the-wire flow through one `netd::Session` per active channel.
- Negotiation failure is observable and audited.
```

## 4. Non-Goals

```text
- No TLS 1.2 fallback.
- No HTTP/1.1 fallback in v0.4.0; HTTP/1.1 over TLS is the only application
  layer, but the parser is *strict* (no chunked encoding, no extensions
  beyond Content-Length, no compression).
- No client certificates (mTLS in v0.7 fleet phase).
- No session resumption tickets in v0.4.0 (each session does full handshake).
- No 0-RTT.
- No support for arbitrary CAs; the trust set is a hard-pinned anchor list.
```

---

## 5. External Design

### 5.1 Channel shape

```rust
pub struct ChannelDescriptor {
    pub channel_id:    ChannelId,
    pub kind:          ChannelKind,        // UpdateMetadata | Diagnostics | Attestation | FleetEnroll
    pub server_name:   [u8; 64],           // pinned SNI (zero-padded)
    pub anchor_epoch:  u32,                // keyring epoch used for cert verify
    pub state:         ChannelState,
    pub session_id:    SessionId,          // netd session backing this channel
    pub opened_tick:   u64,
}

#[repr(u8)]
pub enum ChannelKind {
    UpdateMetadata = 0x01,
    Diagnostics    = 0x02,
    Attestation    = 0x03,
    FleetEnroll    = 0x04,
}

#[repr(u8)]
pub enum ChannelState {
    Negotiating = 1,
    Established = 2,
    Draining    = 3,
    Closed      = 4,
    Faulted     = 5,
}
```

### 5.2 Channel-typed RPC

`secure-transportd` exposes one endpoint with four typed RPC paths. Each
path requires a `ChannelCap` whose rights enumerate that path only.

```text
Tag                            Direction               Payload
SXT_OPEN_CHANNEL               client → sxt            kind u8, server_name 64 B
SXT_OPENED                     sxt → client            channel_id u32, status u8
SXT_UPDATE_METADATA_FETCH      client → sxt            channel_id u32, url_page u16
SXT_UPDATE_METADATA_REPLY      sxt → client            channel_id u32, bytes_page u16, len u32, status u8
SXT_DIAG_PUSH                  client → sxt            channel_id u32, rec_page u16, len u32
SXT_DIAG_ACK                   sxt → client            channel_id u32, status u8
SXT_ATTEST_PUSH                client → sxt            channel_id u32, rec_page u16, len u32
SXT_ATTEST_CHALLENGE           sxt → client            channel_id u32, nonce_page u16
SXT_FLEET_ENROLL_STEP          client → sxt            channel_id u32, state_page u16, len u32
SXT_CLOSE                      client → sxt            channel_id u32
SXT_CLOSED                     sxt → client            channel_id u32, reason u8
SXT_FAULTED                    sxt → client            channel_id u32, reason u8
```

### 5.3 cap-broker policy

```text
service              allowed ChannelKinds       allowed ChannelCap rights
upgraded             UpdateMetadata             SXT_RPC_UPDATE
diagnostics          Diagnostics                SXT_RPC_DIAG
attestd              Attestation                SXT_RPC_ATTEST
fleet-agent (v0.7)   FleetEnroll                SXT_RPC_FLEET_ENROLL
```

A service with `SXT_RPC_UPDATE` cannot use the same cap for diagnostics; the
rights are kind-scoped.

---

## 6. Data Model

### 6.1 ChannelCap rights

```rust
pub const SXT_RPC_UPDATE:        CapRights = CapRights(1 <<  0); // re-used bit-space
pub const SXT_RPC_DIAG:          CapRights = CapRights(1 <<  1); //   within Channel cap kind
pub const SXT_RPC_ATTEST:        CapRights = CapRights(1 <<  2);
pub const SXT_RPC_FLEET_ENROLL:  CapRights = CapRights(1 <<  3);
pub const SXT_CLOSE_CHANNEL:     CapRights = CapRights(1 <<  4);
```

These are bits within a *new* cap kind `Channel`, so they don't collide with
the rights bit table used elsewhere.

### 6.2 TLS state machine (simplified)

```rust
pub enum TlsState {
    Closed,
    ClientHelloSent,
    ServerHelloReceived,
    HandshakeComplete,
    AppData,
    CloseNotifySent,
    Faulted(u16),    // last error code
}
```

### 6.3 Pinned anchor format

The transport anchor list is a keyring purpose with **algorithm = Ed25519**
(in v0.4.0). Server certificates are signed by anchors with
`AuthorityClass::Genesis` for that purpose.

Each anchor binds:

```rust
pub struct TransportAnchor {
    pub purpose:        KeyPurpose,        // = ReleaseVerification (reused)
    pub algorithm:      SignatureAlgorithm,// = Ed25519
    pub epoch:          KeyEpoch,
    pub key_bytes:      [u8; 32],          // Ed25519 pubkey
    pub server_name:    [u8; 64],          // associated SNI
    pub sig_over_cert:  Signature,         // signs the server's cert
}
```

(Note: this is *not* X.509. It is a single-issuer-direct pinning scheme. A
future v0.5 RFC may upgrade to X.509 if integration with public CAs becomes
required.)

---

## 7. Internal Design

### 7.1 Handshake flow

```text
1. open_channel(kind, sni):
     - resolve sni via cap-broker → permitted ChannelKind
     - request Session from netd: peer = sni:443 (TCP)
     - allocate ChannelDescriptor
     - send TLS ClientHello over Session
2. process ServerHello / EncryptedExtensions / Certificate / CertVerify / Finished:
     - verify cert chain (single-level: pinned anchor signs server cert)
     - verify CertVerify with anchor's pubkey
     - verify Finished MAC
     - transition to AppData
3. typed RPC sends/receives encrypted records
4. close_channel: send CloseNotify; transition to Closed
```

### 7.2 Strict HTTP/1.1 parser

For UpdateMetadata channel, the response parser accepts:

- exactly one `HTTP/1.1` status line;
- headers terminated by CRLFCRLF;
- `Content-Length` mandatory; chunked rejected;
- response body capped at `MAX_RESPONSE_BYTES = 64 KiB` (configurable in
  `NetdConfig`);
- text headers ASCII-only.

Everything else is rejected with `SxtError::HttpStrictReject`.

### 7.3 Error model

```rust
#[repr(u16)]
pub enum SxtError {
    UnknownKind            = 0x01,
    ServerNameNotPinned    = 0x02,
    HandshakeFailed        = 0x03,
    CertVerifyFailed       = 0x04,
    HttpStrictReject       = 0x05,
    ChannelClosed          = 0x06,
    ChannelFaulted         = 0x07,
    NoSessionCap           = 0x08,
    SessionRevoked         = 0x09,
    BodyTooLarge           = 0x0A,
    Internal               = 0xFFFF,
}
```

### 7.4 Crypto primitives

- AEAD: AES-128-GCM (constant-time table-free impl; reference build only,
  later swappable for SIMD if a target needs it).
- KEX: X25519 (per RFC 7748).
- KDF: HKDF-SHA256.

All primitives live in a single internal crate `fjell-sxt-crypto`. They are
unit-tested with RFC test vectors.

---

## 8. Security Design

### 8.1 Threat model deltas

```text
Threat T-80: MITM with a forged cert signed by a different CA.
Mitigation:  no CA store; cert must be signed by a pinned anchor for the
             exact SNI scope.

Threat T-81: Anchor rotation during in-flight connection.
Mitigation:  in-flight channel binds to anchor_epoch at open time; rotation
             does not invalidate established channels, but new opens use the
             current active epoch.

Threat T-82: Replay of attestation push.
Mitigation:  attestation_push carries a server challenge (nonce) returned
             before the push; the AttestationRecordV2 binds this nonce in
             freshness.

Threat T-83: HTTP smuggling via chunked transfer.
Mitigation:  parser rejects Transfer-Encoding.

Threat T-84: AEAD nonce reuse across channels.
Mitigation:  each channel has its own keyschedule; AEAD nonce within a
             channel is per-record counter.

Threat T-85: Compromise of secure-transportd extracts long-term key.
Mitigation:  no long-term keying material is held by secure-transportd; the
             pinned anchor pubkey is public; per-handshake ephemerals are
             discarded after Finished.
```

### 8.2 Default-deny posture

- A channel open with an unpinned SNI is rejected without contacting the
  network.
- A typed RPC missing the matching `SXT_RPC_*` right is rejected without
  contacting the network.

### 8.3 Audit emission

```text
SxtChannelOpened      { channel_id, kind, server_name, anchor_epoch }
SxtHandshakeFailed    { channel_id, error_code }
SxtCertVerifyFailed   { channel_id, anchor_epoch }
SxtChannelClosed      { channel_id, reason_code }
SxtChannelFaulted     { channel_id, error_code }
SxtAnchorRotation     { old_epoch, new_epoch }
```

---

## 9. Memory / Resource Design

- TLS record buffer: 2 × 16 KiB (one in, one out) per channel.
- Up to `MAX_CHANNELS = 4` simultaneous channels.
- Per-channel scratch: ~2 KiB for handshake state.
- Total ≈ 4 × 34 KiB ≈ 136 KiB peak.

No allocations on the hot path; all buffers are pre-reserved from a per-task
DMA region.

---

## 10. Compatibility and Migration

### 10.1 ABI compatibility

- New cap kind `Channel`; new rights bits in its private space.
- No syscall changes.
- v0.4 RFC 002 NetdSession protocol unchanged.

### 10.2 Migration plan

| Step | Action |
|------|--------|
| 1    | Add `fjell-sxt-crypto` crate (host-testable, RFC test vectors). |
| 2    | Add `secure-transportd` crate with TLS state machine. |
| 3    | Wire `upgraded` to fetch metadata over UpdateMetadata channel. |
| 4    | Wire `attestd` push path. |
| 5    | Wire `diagnostics` push path (introduced by v0.4 RFC 005). |
| 6    | Update cap-broker policy table. |

---

## 11. Test Strategy

### 11.1 Host unit tests (crypto)

`fjell-sxt-crypto/src/tests.rs`:

```text
- aes128_gcm_rfc_test_vector_1
- aes128_gcm_rfc_test_vector_2
- x25519_rfc7748_section6_test_vector_1
- x25519_rfc7748_section6_test_vector_2
- hkdf_sha256_rfc5869_test_vector_a1
- hkdf_sha256_rfc5869_test_vector_a2
- aead_nonce_reuse_detected
```

### 11.2 Host unit tests (TLS state machine)

```text
- handshake_happy_path
- client_hello_serialise
- server_hello_parse
- cert_verify_with_pinned_anchor
- cert_verify_unknown_anchor_rejected
- finished_mac_check
- close_notify_round_trip
- http_strict_chunked_rejected
- http_strict_no_content_length_rejected
- http_body_too_large_rejected
```

### 11.3 QEMU smoke tests

```text
- SMOKE:SXT:READY
- SMOKE:SXT:UPDATE_FETCH         — fetch /meta.json from QEMU-side server
- SMOKE:SXT:ATTEST_PUSH          — push v2 record, receive nonce reply
```

### 11.4 QEMU negative tests

| Marker                                                       | Profile |
|--------------------------------------------------------------|---------|
| `NEG:SXT:UNPINNED_SNI_REJECTED`                              | sxt     |
| `NEG:SXT:WRONG_RPC_RIGHT_REJECTED`                           | sxt     |
| `NEG:SXT:CERT_NOT_SIGNED_BY_ANCHOR_REJECTED`                 | sxt     |
| `NEG:SXT:RETIRED_ANCHOR_EPOCH_REJECTED`                      | sxt     |
| `NEG:SXT:CHUNKED_TRANSFER_REJECTED`                          | sxt     |
| `NEG:SXT:BODY_OVER_LIMIT_REJECTED`                           | sxt     |
| `NEG:SXT:SESSION_REVOKE_FAULTS_CHANNEL`                      | sxt     |
| `NEG:SXT:HANDSHAKE_DOWNGRADE_REJECTED`                       | sxt     |

---

## 12. Acceptance Criteria

```text
- secure-transportd builds and runs in QEMU.
- 7+10 host crypto/state-machine tests pass.
- 3 SMOKE markers green.
- 8 NEG markers green.
- cap-broker policy table extended.
- upgraded / attestd / diagnostics use the channel APIs.
- ADR-v0.4-003 filed.
```

---

## 13. Documentation Requirements

```text
docs/src/architecture/v0.4-003-secure-transport.md
docs/src/development/v0.4-003-secure-transport.md
docs/src/verification/v0.4-003-secure-transport-invariants.md
docs/src/format/sxt-channel-protocol.md
docs/src/adr/v0.4-003-secure-transport-boundary.md
docs/src/adr/v0.4-003-tls-cipher-suite.md
```

---

## 14. Open Questions

1. **mTLS** — current design is server-only auth. Adding client certs
   requires per-device PKI which is v0.7 scope. Once v0.7 lands, this RFC's
   acceptance criteria will be revisited.
2. **KeyPurpose reuse** — using `KeyPurpose::ReleaseVerification` for both
   release manifests and transport anchors is convenient but couples
   rotation. A future v0.4.x or v0.5 RFC introduces
   `KeyPurpose::TransportAnchor` and migrates the binding.
3. **AES-GCM constant-time** — the reference impl is constant-time via
   table-free big-step. Performance on QEMU is acceptable; on a real board
   we'd want hardware-AES. Tracked under a v0.5 platform RFC.

---

## 15. Release Gate (RFC-local)

```text
- Code merged.
- Host crypto vectors pass.
- 3 SMOKE + 8 NEG markers green in CI.
- ADRs Accepted.
- CHANGELOG entries filed.
```
