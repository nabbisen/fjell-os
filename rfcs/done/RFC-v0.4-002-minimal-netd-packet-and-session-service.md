# RFC-v0.4-002: Minimal netd Packet and Session Service

**Status.** Implemented (v0.4.0)

## Status

Draft (revised, supersedes pack v0.4-002 draft)

## Target Version

`v0.4.0`.

## Phase

Minimal Secure Control-Plane Networking — Epic B (Packet / Session Service).

## Related Work

- v0.4 RFC 001 — `NetDevice` capability and packet protocol that `netd`
  consumes.
- v0.4 RFC 003 — `secure-transportd` (consumes netd's `Session` cap).
- v0.4 RFC 004 — remote-update fetch (consumes secure-transportd).
- v0.2 RFC 040 — `cap-broker` (mediates `Session` cap distribution).

---

## 1. Summary

Introduce `netd`, a user-space service that owns one `NetDevice` capability
and provides a **strictly bounded** L2-to-L4 protocol set required by Fjell's
control-plane use cases. `netd` exposes a `Session` capability whose rights
constrain a holder to a *single* outbound or inbound flow at a time. It does
not expose general-purpose sockets, raw packet sends, or arbitrary listening.

The supported protocol set in v0.4.0 is:

- ARP / NDP (resolution only; no proxy);
- IPv4 (only; IPv6 deferred to v0.5);
- UDP (used by DNS-over-secure transport in v0.4 RFC 003);
- TCP (single-stream client, no listen, no keepalive negotiation).

All higher-level protocols (HTTP, TLS, mTLS, attestation) live in
`secure-transportd`.

---

## 2. Motivation

The v0.4 phase plan promises "secure control-plane only, not general
networking." The hard part is to encode that promise as a *capability shape*
rather than a coding convention. The shape used here:

- `netd` owns the only `NetDevice` cap.
- `netd` mints `Session` caps whose rights enumerate *flow direction*,
  *protocol*, *peer scope*.
- `cap-broker` policy decides which service may obtain which session shape.

Without `netd`'s mediation any compromised service could just write to the
network ring directly. With it, the only path to bytes-on-the-wire is a
`Session` cap, and the only path to a `Session` cap is `cap-broker` policy.

---

## 3. Goals

```text
- One service binary `netd` that owns one NetDevice.
- Session capability with rights: SESSION_SEND, SESSION_RECV,
  SESSION_CLOSE; plus a *scope* that ties the session to a single peer
  (address + port) and a single protocol (UDP or TCP).
- ARP/NDP resolver internal to netd.
- IPv4-only in v0.4.0.
- UDP "datagram pair" sessions (one send/one recv per session).
- TCP "client connect" sessions (no listen).
- Hard caps on simultaneous sessions (MAX_SESSIONS=8) to avoid resource
  exhaustion.
- Default-deny: a Session cap without explicit peer scope is rejected.
- Audit every session open/close, every revoke.
```

## 4. Non-Goals

```text
- No listening sockets (a Fjell service cannot accept inbound connections at
  v0.4; that is v0.7+ scope).
- No multicast, no broadcast (except ARP/NDP internals).
- No raw IP or raw L2 access exposed to clients.
- No DNS resolver in netd (DNS lives in secure-transportd over a UDP session).
- No NAT / port forwarding.
- No connection pooling shared across services.
- No TLS in netd; TLS is in secure-transportd.
```

---

## 5. External Design

### 5.1 Session shape

A `Session` capability binds to a `SessionDescriptor`:

```rust
pub struct SessionDescriptor {
    pub session_id:    SessionId,         // local handle
    pub protocol:      L4Protocol,        // Udp | Tcp
    pub peer:          IpPort,            // IPv4 address + u16 port
    pub direction:     SessionDirection,  // Outbound (only in v0.4.0)
    pub mtu:           u16,
    pub state:         SessionState,
    pub created_tick:  u64,
}

#[repr(u8)]
pub enum SessionDirection { Outbound = 1 }   // Inbound deferred to v0.7

#[repr(u8)]
pub enum L4Protocol { Udp = 17, Tcp = 6 }

#[repr(u8)]
pub enum SessionState {
    Negotiating = 1,   // resolving ARP / handshaking TCP
    Established = 2,
    Draining    = 3,   // received close from peer, flushing
    Closed      = 4,
    Faulted     = 5,
}
```

### 5.2 netd IPC protocol

Endpoint tags:

```text
Tag                    Direction       Payload
NETD_SESSION_OPEN      client → netd   proto u8, peer_ip u32, peer_port u16, flags u16
NETD_SESSION_OPENED    netd → client   session_id u32, mtu u16, status u8
NETD_SESSION_SEND      client → netd   session_id u32, len u16, page_id u16 (shared)
NETD_SESSION_SENT      netd → client   session_id u32, bytes_sent u16, status u8
NETD_SESSION_RECV      client → netd   session_id u32, page_id u16
NETD_SESSION_RECEIVED  netd → client   session_id u32, len u16, status u8
NETD_SESSION_CLOSE     client → netd   session_id u32
NETD_SESSION_CLOSED    netd → client   session_id u32, reason u8
NETD_SESSION_REVOKED   netd → client   session_id u32, reason u8
NETD_LINK_DOWN         netd → all      reason u8
NETD_QUERY_STATE       client → netd   _
NETD_QUERY_REPLY       netd → client   link u8, session_count u8, mtu u16
```

### 5.3 cap-broker policy

`netd` mints `Session` caps with a *scope* equal to the `SessionDescriptor`
contents. cap-broker's policy table grows new rows:

```text
service             allowed scopes
secure-transportd   { proto=Udp, peer=*:53 }  (DNS-over-DTLS in v0.4)
                    { proto=Tcp, peer=*:443 } (update metadata fetch)
                    { proto=Tcp, peer=*:4443 } (attestation transport)
upgraded            indirect (only via secure-transportd)
attestd             indirect (only via secure-transportd)
diagnostics         indirect (only via secure-transportd)
```

The wildcard `peer=*` means the host portion is open but the port is fixed.
Tightening to specific hosts is deferred to a fleet-policy RFC in v0.8.

---

## 6. Data Model

### 6.1 Internal session table

```rust
pub const MAX_SESSIONS: usize = 8;

pub struct SessionEntry {
    pub descriptor:    SessionDescriptor,
    pub owner:         TaskId,
    pub lease:         LeaseId,
    pub tx_pending:    u8,        // pending TX descriptors
    pub rx_pending:    u8,        // pending RX descriptors
    pub last_active:   u64,
}
```

### 6.2 ARP / NDP cache

```rust
pub const ARP_CACHE_ENTRIES: usize = 16;

pub struct ArpEntry {
    pub ipv4:          [u8; 4],
    pub mac:           [u8; 6],
    pub last_seen:     u64,
    pub state:         ArpEntryState, // Resolving | Resolved | Stale | Failed
}
```

### 6.3 Routing (trivial in v0.4.0)

v0.4.0 supports exactly one IPv4 subnet:

```rust
pub struct NetdConfig {
    pub local_ipv4:   [u8; 4],
    pub local_mac:    [u8; 6],
    pub netmask:      [u8; 4],
    pub gateway:      [u8; 4],
    pub mtu:          u16,
}
```

`NetdConfig` is loaded from `configd` at boot. A future v0.5 RFC introduces
declarative network policy in TOML.

---

## 7. Internal Design

### 7.1 TCP client engine

A minimal TCB:

```rust
pub struct TcpControlBlock {
    pub state:        TcpState,         // SynSent | Established | FinWait | TimeWait | Closed
    pub local_port:   u16,
    pub peer_port:    u16,
    pub peer_ip:      [u8; 4],
    pub snd_iss:      u32,
    pub snd_una:      u32,
    pub snd_nxt:      u32,
    pub rcv_nxt:      u32,
    pub rcv_wnd:      u16,
    pub retransmit_deadline: u64,
}
```

Implementation principles:

- one in-flight segment per session at a time (no SACK, no window scaling);
- retransmit on a single deadline; exponential back-off bounded to 3 retries;
- no congestion control beyond the single-in-flight rule;
- payload size capped at MTU-40 (IP+TCP header).

The bar is "good enough to fetch update metadata"; production-quality TCP is
out of scope.

### 7.2 UDP engine

UDP is single-datagram per `SESSION_SEND`. There is no per-session datagram
buffering beyond one in-flight RX.

### 7.3 Revocation and clean-up

```text
on Session revoke:
  - if TCP and Established: send RST, transition to Closed
  - if UDP: drop pending RX
  - mark session Faulted; emit NETD_SESSION_REVOKED
  - reclaim shared DMA page
  - cap-broker informed; downstream services get LeaseRevoked
```

### 7.4 Link-down handling

```text
on NET_LINK_DOWN from driver:
  - all sessions transition to Faulted with reason=LinkDown
  - clients receive NETD_SESSION_REVOKED { reason=LinkDown }
  - sessions are *not* automatically restored when link comes back;
    clients must reopen
```

---

## 8. Security Design

### 8.1 Threat model deltas

```text
Threat T-70: A compromised service obtains a Session cap with peer=*:* and
             exfiltrates data to attacker-controlled host.
Mitigation:  cap-broker policy never mints a Session with peer=*:*. All
             session caps have a specific port scope.

Threat T-71: Compromised service holds a Session cap across a policy change.
Mitigation:  policy change triggers cap-broker revoke; lease epoch bumps;
             use-time check rejects on next operation.

Threat T-72: TCP RST injection.
Mitigation:  netd uses small randomised ISS; one in-flight segment limits
             the injection window. This is a known soft mitigation; full
             RST hardening is deferred to a v0.4.x RFC.

Threat T-73: ARP cache poisoning.
Mitigation:  ARP entries respected only if they came in response to a
             pending request; unsolicited replies are dropped. Cache entries
             have a state machine and a max age.

Threat T-74: Resource exhaustion via session-open spam.
Mitigation:  MAX_SESSIONS=8 hard cap; per-service quota enforced by
             cap-broker (typically 2/service).
```

### 8.2 Default-deny posture

- `cap-broker` rejects any session request whose scope is not enumerated in
  the policy table.
- `netd` itself rejects malformed `NETD_SESSION_OPEN` (e.g., proto outside
  {Udp, Tcp}, peer_port == 0).

### 8.3 Audit emission

```text
NetdSessionOpened          { session_id, proto, peer, owner }
NetdSessionOpenRejected    { reason_code, peer, owner }
NetdSessionRevoked         { session_id, reason_code }
NetdSessionFaulted         { session_id, reason_code }
NetdArpUnsolicitedReplyDropped { src_ipv4 }
NetdLinkDownObserved       { reason_code }
```

---

## 9. Memory / Resource Design

- Session table: 8 entries × ~80 B = 640 B.
- ARP cache: 16 × 20 B = 320 B.
- Per-session shared DMA page: 1 × 4 KiB allocated lazily from netd's
  per-task DMA region (v0.2 RFC 007).
- TCP TCBs: 8 × ~64 B = 512 B.

Total fixed footprint ≈ 1.5 KiB plus per-session DMA pages on demand.

---

## 10. Compatibility and Migration

### 10.1 ABI compatibility

- No syscall changes (consumes v0.4 RFC 001 surface).
- New cap kind `Session` and new rights `SESSION_SEND/RECV/CLOSE` defined in
  `fjell-cap`.

### 10.2 Migration plan

| Step | Action |
|------|--------|
| 1    | Add `Session` cap kind to `fjell-cap`. |
| 2    | Add `netd` crate with internal protocol crate (`fjell-net-format`). |
| 3    | Add cap-broker policy rows. |
| 4    | Smoke test: open UDP session to a QEMU-side echo server. |
| 5    | Smoke test: open TCP session to a QEMU-side static-content server. |
| 6    | RFC v0.4-003 (`secure-transportd`) consumes Session caps. |

---

## 11. Test Strategy

### 11.1 Host unit tests (`fjell-net-format`)

```text
- session_descriptor_serialise_then_parse_round_trip
- l4_protocol_tag_stability
- session_state_tag_stability
- tcp_iss_in_valid_range
- arp_entry_age_check
- ipv4_header_checksum
- udp_checksum
- mtu_packet_too_large_rejected
```

### 11.2 QEMU smoke tests

```text
- SMOKE:NETD:READY
- SMOKE:NETD:UDP_ECHO            — open UDP, echo round-trip < 100 ms
- SMOKE:NETD:TCP_FETCH           — open TCP, fetch 256 B
- SMOKE:NETD:ARP_RESOLVE         — successful ARP resolution to gateway
```

### 11.3 QEMU negative tests

| Marker                                                | Profile |
|-------------------------------------------------------|---------|
| `NEG:NETD:NO_SESSION_CAP_REJECTED`                    | netd    |
| `NEG:NETD:SCOPE_MISMATCH_REJECTED`                    | netd    |
| `NEG:NETD:STALE_SESSION_REJECTED`                     | netd    |
| `NEG:NETD:RAW_OPEN_PEER_WILDCARD_REJECTED`            | netd    |
| `NEG:NETD:SESSION_OVER_QUOTA_REJECTED`                | netd    |
| `NEG:NETD:LINK_DOWN_FORCES_REVOKE`                    | netd    |
| `NEG:NETD:UNSOLICITED_ARP_DROPPED`                    | netd    |
| `NEG:NETD:CLOSED_SESSION_REUSE_REJECTED`              | netd    |

### 11.4 Property tests (deferred to v0.6)

```text
- session_table_no_leak: after K random open/close cycles, table is empty.
- arp_cache_bounded: cache never exceeds ARP_CACHE_ENTRIES.
```

---

## 12. Acceptance Criteria

```text
- netd binary builds and runs in QEMU.
- All 4 SMOKE markers green.
- All 8 NEG markers green.
- cap-broker policy table extended with NetdSession scopes.
- ≥ 8 host unit tests for fjell-net-format.
- ADR-v0.4-002 filed.
```

---

## 13. Documentation Requirements

```text
docs/src/architecture/v0.4-002-netd.md
docs/src/development/v0.4-002-netd.md
docs/src/verification/v0.4-002-netd-invariants.md
docs/src/format/net-session-protocol.md
docs/src/adr/v0.4-002-net-session-shape.md
```

---

## 14. Open Questions

1. **IPv6** — deferred to v0.5. IPv6 introduces NDP (mostly compatible),
   stateful address autoconfig (out of scope), and link-local mac mapping
   (trivial). The fields in `SessionDescriptor` need extension; postponing
   keeps v0.4 narrow.
2. **TCP RST hardening** — current design is minimal. A v0.4.x RFC may add a
   per-session random sequence offset and a packet-validation table; or
   alternatively the design defers to a TLS layer that detects RST anyway.
3. **netd HA** — what if netd crashes? Sessions are revoked; clients must
   reopen. devmgr restarts netd. There is no in-memory state migration. The
   restart penalty is acceptable for a control-plane service.

---

## 15. Release Gate (RFC-local)

```text
- netd lands; smoke + negative markers green.
- cap-broker policy committed.
- 8 host tests green in fjell-net-format.
- ADR Accepted.
- CHANGELOG entry filed.
```
